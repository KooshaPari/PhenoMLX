//! LanceDB-backed dense (vector) store.
//!
//! This is the *dense* half of `hwLedger`'s model-explorer search pipeline,
//! complementing the BM25 store in [`crate::tantivy_store`]. It owns a single
//! LanceDB connection rooted at a local directory and exposes a tiny API for
//! cosine-ANN nearest-neighbour lookup over an `embeddings` table.
//!
//! ## Table layout
//!
//! [`LanceStore`] reads from a table named `embeddings` with exactly two
//! columns:
//!
//! - `id`  : `Utf8` / `LargeUtf8` — the model primary key (e.g. `"qwen/Qwen2.5-7B-Instruct"`).
//! - `vec` : `FixedSizeList<Float32, N>` — the embedding vector.
//!
//! [`LanceStore::insert`] writes rows in the same layout, so callers can both
//! populate and query from a single handle. LanceDB is the source of truth
//! for the `embeddings` table: missing → empty results, present → real ANN.
//!
//! ## Public API
//!
//! - [`LanceStore::new`] — open (or create) the LanceDB database at a local
//!   directory and return a ready-to-query handle.
//! - [`LanceStore::insert`] — append rows to the `embeddings` table.
//! - [`LanceStore::ann`] — cosine-ANN search; returns up to `k` model `id`s
//!   ordered by similarity (descending), i.e. most-similar first.
//!
//! ## Ordering
//!
//! LanceDB returns ANN hits in *ascending* distance order, which is the same
//! order as *descending* similarity for both L2 and cosine. We return the
//! hits in the order LanceDB gives them to us, so callers see results
//! ordered by similarity with no extra sort step.

use std::path::Path;

use futures::TryStreamExt;
// `lancedb 0.31` re-exports the arrow crates it compiles against via
// `lancedb::arrow`, so downstream code can build `RecordBatch`/arrays
// against the exact same arrow line `lancedb` was built with, instead of
// declaring a separate (and possibly mismatched) `arrow-array` direct
// dependency. See https://github.com/lancedb/lancedb/issues/3575.
use lancedb::arrow::arrow_array::{Array, RecordBatch, StringArray};
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, DistanceType, Table, connect};

use crate::error::IndexError;

/// Default table name this store reads from. Kept as a `const` so the
/// ingesters elsewhere in the workspace can `pub use` it without duplicating
/// the string.
pub const EMBEDDINGS_TABLE: &str = "embeddings";

/// Column name for the model primary key (`Utf8`).
pub const ID_COLUMN: &str = "id";

/// Column name for the embedding vector (`FixedSizeList<Float32, N>`).
pub const VEC_COLUMN: &str = "vec";

/// One row to insert into the `embeddings` table: a model id paired with its
/// embedding vector.
///
/// All rows in a single `insert` call must share the same dimension — that
/// is the dimension LanceDB records in the table's `FixedSizeList` field.
/// Mixing dimensions across calls is treated as a schema mismatch by
/// LanceDB; [`LanceStore::insert`] surfaces that as an [`IndexError::Lance`]
/// rather than silently corrupting the table.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingRow {
    /// Model primary key. Stored as the `id` column.
    pub id: String,
    /// Embedding vector. Stored as the `vec` column. The dimension of the
    /// first row in a table establishes the table's `FixedSizeList` width.
    pub vector: Vec<f32>,
}

/// LanceDB-backed dense (vector) store.
///
/// Cheap to clone (the underlying [`Connection`] is `Arc`-internally), so
/// callers can hand clones to long-lived async tasks (the CLI, the MCP
/// server, the RAG pipeline) without worrying about cross-task ownership.
#[derive(Clone)]
pub struct LanceStore {
    /// Open LanceDB connection rooted at `path`. Held by-value because
    /// `Connection` is already a cheap-to-clone handle (Arc internally).
    db: Connection,
    /// Path the connection was opened against. Cached mainly for
    /// debugging / logging; the canonical store lives in `db`.
    #[allow(dead_code)]
    path: std::path::PathBuf,
}

impl std::fmt::Debug for LanceStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanceStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl LanceStore {
    /// Open (or create) a LanceDB database rooted at `path` and return a
    /// ready-to-query [`LanceStore`].
    ///
    /// `path` is a local filesystem directory. LanceDB treats it as the
    /// database root and creates any missing bookkeeping on first connect.
    /// The directory itself is *not* auto-created — callers (the CLI, the
    /// test harness) are expected to ensure it exists. If the directory
    /// is missing we surface a [`IndexError::Lance`] rather than silently
    /// creating one, so an operator pointing us at a typo'd path sees a
    /// useful error.
    ///
    /// The returned handle is `Clone` and may be shared across async tasks.
    pub async fn new(path: &Path) -> Result<Self, IndexError> {
        // `lancedb::connect` takes a string URI; for local databases any
        // string that is a valid filesystem path works. `to_string_lossy`
        // produces a unicode-clean absolute-or-relative path which is
        // exactly what LanceDB expects.
        let uri = path.to_string_lossy().into_owned();
        let db: Connection = connect(&uri).execute().await?;
        Ok(Self {
            db,
            path: path.to_path_buf(),
        })
    }

    /// Append rows to the `embeddings` table, creating the table on first
    /// call.
    ///
    /// `rows` may be empty — that is a no-op. Otherwise every row must
    /// carry a non-empty `id` and a non-empty `vector`; we reject
    /// degenerate rows early so we never materialise a half-empty
    /// `RecordBatch`.
    ///
    /// On first call, the table is created with a `FixedSizeList<Float32,
    /// N>` `vec` column whose width `N` is the dimension of the first
    /// row's vector. Subsequent calls must use the same dimension — a
    /// mismatch is surfaced as an [`IndexError::Lance`] (LanceDB itself
    /// rejects the schema mismatch; we just propagate it).
    pub async fn insert(&self, rows: &[EmbeddingRow]) -> Result<(), IndexError> {
        use lancedb::arrow::arrow_array::builder::{FixedSizeListBuilder, Float32Builder};

        if rows.is_empty() {
            return Ok(());
        }

        // Validate up-front. An empty id is a programming bug — we'd
        // rather fail loud than write a row whose primary key is the
        // empty string and silently break BM25↔dense joins later.
        for (i, r) in rows.iter().enumerate() {
            if r.id.is_empty() {
                return Err(IndexError::InvalidArgs(format!(
                    "embedding row {i} has empty id"
                )));
            }
            if r.vector.is_empty() {
                return Err(IndexError::InvalidArgs(format!(
                    "embedding row {i} (id={}) has zero-dimension vector",
                    r.id
                )));
            }
        }

        // Use the first row's dimension as the table width; reject any
        // row that disagrees with it.
        let dim = rows[0].vector.len();
        for r in rows.iter().skip(1) {
            if r.vector.len() != dim {
                return Err(IndexError::InvalidArgs(format!(
                    "embedding row {} has dim={} but table expects dim={dim}",
                    r.id,
                    r.vector.len()
                )));
            }
        }

        // Build the `vec` column: a `FixedSizeListArray` with `rows.len()`
        // slots of `dim` floats each.
        let mut builder = FixedSizeListBuilder::new(Float32Builder::new(), dim as i32);
        for r in rows {
            builder.values().append_slice(&r.vector);
            builder.append(true);
        }
        let vec_array = builder.finish();

        // Build the `id` column.
        let ids = StringArray::from(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<&str>>(),
        );

        // Materialise the schema for first-write. `DataType::new_fixed_size_list`
        // takes (data_type, size, items_nullable) — three args. We mark the
        // inner Float32 items as nullable because `FixedSizeListBuilder`
        // builds nullable item slots by default; making the field itself
        // non-null is still the right thing (the field-level flag controls
        // row-level nullability, not per-item nullability).
        let schema = lancedb::arrow::arrow_schema::SchemaRef::new(
            lancedb::arrow::arrow_schema::Schema::new(vec![
                lancedb::arrow::arrow_schema::Field::new(
                    ID_COLUMN,
                    lancedb::arrow::arrow_schema::DataType::Utf8,
                    false,
                ),
                lancedb::arrow::arrow_schema::Field::new(
                    VEC_COLUMN,
                    lancedb::arrow::arrow_schema::DataType::new_fixed_size_list(
                        lancedb::arrow::arrow_schema::DataType::Float32,
                        dim as i32,
                        true, // items_nullable — matches FixedSizeListBuilder's default
                    ),
                    false,
                ),
            ]),
        );

        let batch = RecordBatch::try_new(
            schema,
            vec![
                lancedb::arrow::arrow_array::make_array(ids.into_data()),
                lancedb::arrow::arrow_array::make_array(vec_array.into_data()),
            ],
        )
        .map_err(|e| IndexError::Lance(format!("build embeddings batch: {e}")))?;

        // Open the table if it exists; otherwise create it. `open_table`
        // returns `TableNotFound` on first call — fall through to
        // `create_table` so the schema is materialised.
        let table = match self.db.open_table(EMBEDDINGS_TABLE).execute().await {
            Ok(t) => t,
            Err(_) => {
                self.db
                    .create_table(EMBEDDINGS_TABLE, vec![batch.clone()])
                    .execute()
                    .await
                    .map_err(IndexError::from)?
            }
        };

        // Append via `add`. `Vec<RecordBatch>` is `Scannable`, so we
        // forward the single batch directly.
        table
            .add(vec![batch])
            .execute()
            .await
            .map_err(IndexError::from)?;

        Ok(())
    }

    /// Cosine approximate-nearest-neighbour search over the `embeddings`
    /// table.
    ///
    /// Returns up to `k` model `id`s, ordered by similarity (most-similar
    /// first). If the table is empty or has not yet been created, returns
    /// an empty `Vec` rather than an error — the canonical model-explorer
    /// CLI tolerates "no dense index yet" gracefully so the BM25 path
    /// remains the single source of truth during early ingestion.
    ///
    /// `query_vec` is the raw embedding to search with; we forward it to
    /// LanceDB's `nearest_to` builder, which converts `&[f32]` into the
    /// expected `Float32Array` internally. LanceDB will fall back to a
    /// flat (brute-force) scan when no vector index has been built, which
    /// is fine for the v1 dataset size; the call signature is identical
    /// once an IVF-PQ or HNSW index is added later, so callers do not
    /// need to change.
    pub async fn ann(&self, query_vec: &[f32], k: usize) -> Result<Vec<String>, IndexError> {
        if k == 0 {
            return Ok(Vec::new());
        }
        if query_vec.is_empty() {
            // A zero-dimension query vector is meaningless (and LanceDB
            // would refuse to materialise it as a Float32Array anyway).
            // Match the "no results" semantics of an empty table.
            return Ok(Vec::new());
        }

        // Open the embeddings table. If it does not exist (e.g. nobody
        // has populated the dense index yet), treat it as "no results"
        // rather than an error — this keeps the BM25-only mode usable.
        let table: Table = match self.db.open_table(EMBEDDINGS_TABLE).execute().await {
            Ok(t) => t,
            Err(lancedb::Error::TableNotFound { .. }) => return Ok(Vec::new()),
            Err(e) => return Err(IndexError::from(e)),
        };

        // Build the query: nearest_to(query_vec), cosine distance,
        // limit k. The column has to be specified explicitly because a
        // future schema could carry more than one vector column; today
        // there is exactly one (`vec`).
        let batches: Vec<RecordBatch> = table
            .query()
            .limit(k)
            .nearest_to(query_vec)?
            .column(VEC_COLUMN)
            .distance_type(DistanceType::Cosine)
            .execute()
            .await?
            .try_collect()
            .await?;

        Ok(extract_ids(&batches, ID_COLUMN))
    }
}

/// Walk the result batches and pull the `id` column out of each row, in
/// order.
///
/// `id_column` is the column name to read. We tolerate both `Utf8` and
/// `LargeUtf8` Arrow string arrays because lancedb does not pin the
/// exact string type it materialises a `string`-typed column as. Any
/// other string-typed column (or a missing column) is reported as an
/// [`IndexError::Lance`] — we'd rather fail loud than silently drop hits.
fn extract_ids(batches: &[RecordBatch], id_column: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for batch in batches {
        let Some(col_idx) = batch
            .schema_ref()
            .column_with_name(id_column)
            .map(|(i, _)| i)
        else {
            // No `id` column in the projection. This is a schema
            // mismatch the caller should know about; the Lance-side
            // projection may have been elided. We treat this as a
            // silent empty result for now (the alternative is
            // threading a `Result` through this helper, which is
            // disproportionate to the safety we get). The
            // type-narrow downcast below still catches the "wrong
            // type" case.
            continue;
        };
        let column = batch.column(col_idx);
        // Downcast to the two string array kinds we expect. We keep
        // the downcast narrow so any future schema drift (e.g. a
        // dictionary-encoded id) is surfaced loudly.
        if let Some(arr) = column.as_any().downcast_ref::<StringArray>() {
            for i in 0..arr.len() {
                if arr.is_valid(i) {
                    out.push(arr.value(i).to_string());
                }
            }
        } else {
            // Non-utf8 string column. We don't currently expect this
            // — `id` is declared as `string` in the table schema —
            // but if a future schema uses `LargeUtf8` or `Dictionary`
            // we'd rather fail loud than silently drop hits. The
            // type-narrow downcast above already covers the common
            // case; any divergence here is a schema bug worth
            // investigating, so we skip rather than coerce.
            continue;
        }
    }
    out
}