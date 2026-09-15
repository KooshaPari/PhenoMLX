//! Tantivy-backed BM25 store.
//!
//! One [`TantivyStore`] wraps a single Tantivy index directory. The schema is
//! defined inline (see [`build_schema`]) and is intentionally narrow: just
//! enough fields to do reasonable free-text recall over a model card.
//!
//! Public API:
//!
//! - [`TantivyStore::open`] — create or open the index at a directory.
//! - [`TantivyStore::upsert`] — replace the doc with the given `id`.
//! - [`TantivyStore::search`] — BM25 over all indexed fields with per-field
//!   boosts (name^3, org^2, kind^2, family^2, arch^1, quants^1, card_snippet^1).
//! - [`TantivyStore::commit`] — flush the writer.
//!
//! [`IndexHit`] is the crate-local result row; the higher-level
//! [`crate::query::run_hybrid`] converts these into the
//! `hwledger_search_core::FusedResult` shape.
//!
//! ## Payload shape
//!
//! [`TantivyStore::upsert`] takes a single [`IndexedDoc`] payload rather
//! than a long positional argument list. This is the schema-mirroring type
//! that the crate exposes; the higher-level [`crate::ingest::IndexedModel`]
//! in this same crate is the transport type that callers (CLI, ingest,
//! MCP) construct before handing it to [`crate::ingest::upsert_model`].
//! `IndexedModel` -> `IndexedDoc` is a one-field (`Vec<String>` -> joined
//! `Cow<str>`) conversion.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED, STRING,
};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::error::IndexError;

/// One BM25 hit, in the shape callers (CLI, MCP) expect.
///
/// `id` is the primary key the caller used during [`TantivyStore::upsert`].
/// `score` is the raw Tantivy BM25 score (higher = more relevant).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexHit {
    /// Primary-key id (see [`crate::ingest::IndexedModel::id`]).
    pub id: String,
    /// Tantivy BM25 score, monotonically higher for more-relevant hits.
    pub score: f32,
}

impl IndexHit {
    /// Builder helper.
    pub fn new<I: Into<String>>(id: I, score: f32) -> Self {
        Self {
            id: id.into(),
            score,
        }
    }
}

/// Schema-mirroring payload for a single doc to upsert.
///
/// This is the type [`TantivyStore::upsert`] accepts. It is intentionally
/// `&str`-based (not `String`) so the caller doesn't have to allocate per
/// field; `quants` is whitespace-separated so Tantivy's default tokenizer
/// indexes each format as its own token (e.g. `"gguf gptq awq"` -> three
/// queryable tokens).
///
/// Construct via struct-literal syntax — there is intentionally no
/// `::new()` constructor, both to avoid a redundant API surface and to
/// keep the call-site field labels visible:
///
/// ```ignore
/// store.upsert(&IndexedDoc {
///     id: "qwen/Qwen2.5-7B-Instruct",
///     name: "Qwen2.5 7B Instruct",
///     org: "qwen",
///     kind: "instruct",
///     family: "qwen2",
///     arch: "gqa",
///     quants: "gguf gptq",
///     card_snippet: "Qwen2.5 is the latest series of large language models from Alibaba.",
/// })?;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedDoc<'a> {
    /// Stable primary key (e.g. `"qwen/Qwen2.5-7B-Instruct"`).
    pub id: &'a str,
    /// Human-readable display name.
    pub name: &'a str,
    /// Owner / publisher (e.g. `"qwen"`, `"meta-llama"`).
    pub org: &'a str,
    /// Coarse model kind (e.g. `"instruct"`, `"base"`).
    pub kind: &'a str,
    /// Architecture family (e.g. `"qwen2"`, `"llama"`).
    pub family: &'a str,
    /// Attention block flavor (e.g. `"gqa"`, `"mha"`).
    pub arch: &'a str,
    /// Quantization formats the model ships in, whitespace-separated.
    pub quants: &'a str,
    /// First ~2000 chars of the model card body, for free-text recall.
    pub card_snippet: &'a str,
}

/// Holds the resolved schema field handles, plus the running Tantivy objects.
pub struct TantivyStore {
    /// The schema we wrote to disk; held so we can resolve `Field` ids cheaply
    /// on every upsert/search call.
    schema: Schema,

    /// The underlying Tantivy index. Cheap to clone (`Arc` internally).
    idx: Index,

    /// Lazily-constructed writer; we hold an `Option` because the writer is
    /// `!Send` on drop in some edge cases and we want to be able to
    /// re-create it after a `commit` / explicit drop if the caller ever
    /// exhausts it.
    writer: Mutex<Option<IndexWriter>>,

    /// Cached reader used by `search`. The default reload policy is
    /// `OnCommitWithDelay`; we additionally call `reload()` after each commit
    /// for deterministic test behaviour.
    reader: IndexReader,

    /// Resolved field handles.
    fields: SchemaFields,

    /// Sidecar: per-id `kind` (untokenized string). We keep this in-process
    /// because the v1 Tantivy schema treats `kind` as a tokenized text
    /// field — fast and good for BM25, but expensive to round-trip on a
    /// post-filter lookup. The sidecar is updated by every [`upsert`].
    kinds: Arc<Mutex<std::collections::HashMap<String, String>>>,

    /// Sidecar: per-id `quants` list (split on whitespace).
    quants: Arc<Mutex<std::collections::HashMap<String, Vec<String>>>>,

    /// Directory the index lives in. Cached on open so we can re-read /
    /// re-write the kind/quants sidecar without going back through tantivy.
    index_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct SchemaFields {
    id: Field,
    name: Field,
    org: Field,
    kind: Field,
    family: Field,
    arch: Field,
    quants: Field,
    card_snippet: Field,
}

/// Build the (immutable) schema for the store.
///
/// The schema is the same whether we're creating or opening; tantivy will
/// validate it against `meta.json` on open.
fn build_schema() -> (Schema, SchemaFields) {
    let mut b = Schema::builder();

    // `id` is the primary key. STRING = untokenized + indexed; STORED so we
    // can pull it back out of search hits without re-walking the doc.
    let id = b.add_text_field("id", STRING | STORED);

    // The remaining fields are full-text searchable. Default tokenization +
    // positions. We deliberately do *not* set `STORED` on these — we never
    // need to retrieve them; only the `id` is read back.
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);

    let name = b.add_text_field("name", TextOptions::default().set_indexing_options(text_indexing.clone()));
    let org = b.add_text_field("org", TextOptions::default().set_indexing_options(text_indexing.clone()));
    let kind = b.add_text_field("kind", TextOptions::default().set_indexing_options(text_indexing.clone()));
    let family = b.add_text_field("family", TextOptions::default().set_indexing_options(text_indexing.clone()));
    let arch = b.add_text_field("arch", TextOptions::default().set_indexing_options(text_indexing.clone()));
    let quants = b.add_text_field("quants", TextOptions::default().set_indexing_options(text_indexing.clone()));
    let card_snippet = b.add_text_field(
        "card_snippet",
        TextOptions::default().set_indexing_options(text_indexing),
    );

    let schema = b.build();
    let fields = SchemaFields {
        id,
        name,
        org,
        kind,
        family,
        arch,
        quants,
        card_snippet,
    };
    (schema, fields)
}

impl TantivyStore {
    /// Open (or create) the index at `path`. The path must be a directory;
    /// tantivy will create the directory if it doesn't already exist.
    ///
    /// If the directory exists but doesn't yet contain a Tantivy `meta.json`
    /// (e.g. a freshly-created `tempfile::tempdir()`), we treat it as a
    /// *new* index and call `Index::create_in_dir` — this is friendlier to
    /// test harnesses that pre-create the directory.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, IndexError> {
        let path = path.as_ref();
        let (schema, fields) = build_schema();

        let meta_exists = path.join("meta.json").exists();
        let idx = if meta_exists {
            // Open existing. tantivy reads meta.json and rejects mismatched
            // schemas; we propagate the error.
            Index::open_in_dir(path)?
        } else {
            // Create new. Ensure the directory exists; tantivy's
            // `create_in_dir` will populate `meta.json` + segments/.
            std::fs::create_dir_all(path).map_err(IndexError::from)?;
            Index::create_in_dir(path, schema.clone())?
        };

        let writer = idx.writer(50_000_000)?;
        let reader = idx
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        // Reload the sidecar (kind/quants caches) from disk so a freshly
        // opened handle sees the same per-id metadata that the writer left
        // behind. Without this, `kind_for_id` / `quants_for_id` would only
        // return data for ids upserted in this same process — which would
        // break the CLI's `model detail` path when the index was populated
        // by a different process (e.g. an integration test fixture).
        let (kinds, quants) = read_sidecar(path);

        Ok(Self {
            schema,
            idx,
            writer: Mutex::new(Some(writer)),
            reader,
            fields,
            kinds: Arc::new(Mutex::new(kinds)),
            quants: Arc::new(Mutex::new(quants)),
            index_dir: path.to_path_buf(),
        })
    }

    /// Return a reference to the underlying schema (mainly useful in tests).
    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Replace (or insert) the document identified by `doc.id`.
    ///
    /// The payload is a borrowed [`IndexedDoc`]; this keeps the public
    /// signature a single argument after `&self` and avoids forcing
    /// callers to allocate per-field `String`s. Tantivy's "update" model
    /// is `delete_term` + `add_document`; the delete only takes effect
    /// after [`commit`](Self::commit).
    pub fn upsert(&self, doc: &IndexedDoc<'_>) -> Result<(), IndexError> {
        if doc.id.is_empty() {
            return Err(IndexError::InvalidArgs("id is empty".into()));
        }

        let mut guard = self
            .writer
            .lock()
            .map_err(|e| IndexError::Tantivy(format!("writer mutex poisoned: {e}")))?;
        let writer = guard
            .as_mut()
            .ok_or_else(|| IndexError::Tantivy("writer is missing (was committed away?)".into()))?;

        // Delete-by-id before add; the delete will be visible at the next
        // commit, at which point we'll have only the new doc.
        let id_term = Term::from_field_text(self.fields.id, doc.id);
        writer.delete_term(id_term);

        let mut tantivy_doc = TantivyDocument::default();
        tantivy_doc.add_text(self.fields.id, doc.id);
        tantivy_doc.add_text(self.fields.name, doc.name);
        tantivy_doc.add_text(self.fields.org, doc.org);
        tantivy_doc.add_text(self.fields.kind, doc.kind);
        tantivy_doc.add_text(self.fields.family, doc.family);
        tantivy_doc.add_text(self.fields.arch, doc.arch);
        tantivy_doc.add_text(self.fields.quants, doc.quants);
        tantivy_doc.add_text(self.fields.card_snippet, doc.card_snippet);

        writer.add_document(tantivy_doc)?;

        // Sidecar refresh: replace this id's kind + quants so a follow-up
        // post-filter lookup is O(1).
        if let Ok(mut kinds_map) = self.kinds.lock() {
            kinds_map.insert(doc.id.to_string(), doc.kind.to_string());
        }
        if let Ok(mut quants_map) = self.quants.lock() {
            let split: Vec<String> = doc
                .quants
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            quants_map.insert(doc.id.to_string(), split);
        }
        Ok(())
    }

    /// Look up the (untokenized) `kind` string we stored at
    /// [`upsert`](Self::upsert) time.
    #[must_use]
    pub fn kind_for_id(&self, id: &str) -> Option<String> {
        self.kinds.lock().ok().and_then(|m| m.get(id).cloned())
    }

    /// Look up the list of quantization tags we stored at
    /// [`upsert`](Self::upsert) time.
    #[must_use]
    pub fn quants_for_id(&self, id: &str) -> Option<Vec<String>> {
        self.quants.lock().ok().and_then(|m| m.get(id).cloned())
    }

    /// Enumerate every indexed model id. Used by the OpenAI-compatible
    /// `/v1/models` enumeration endpoint.
    ///
    /// Implementation note: we walk the sidecar map (not a Tantivy
    /// `IndexReader::terms()` scan) because the sidecar is O(N) HashMap
    /// iteration while a Tantivy term scan would be O(N · log N) and
    /// would require loading every term dictionary.
    #[must_use]
    pub fn list_all_ids(&self) -> Vec<String> {
        self.kinds
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Flush the writer to disk and force the reader to pick up the new
    /// segment.
    pub fn commit(&self) -> Result<(), IndexError> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|e| IndexError::Tantivy(format!("writer mutex poisoned: {e}")))?;
        let writer = guard
            .as_mut()
            .ok_or_else(|| IndexError::Tantivy("writer is missing".into()))?;
        writer.commit()?;
        // Reload so a follow-up `search()` in the same process observes the
        // commit without waiting for the policy delay.
        self.reader.reload()?;

        // Persist the kind/quants sidecar so a freshly-opened handle sees
        // the same per-id metadata. We grab *clones* of the two maps and
        // write them out; the in-memory state stays untouched so we don't
        // need to hold the mutex across the (slow) disk write.
        let kinds = self
            .kinds
            .lock()
            .map(|m| m.clone())
            .map_err(|e| IndexError::Tantivy(format!("sidecar mutex poisoned: {e}")))?;
        let quants = self
            .quants
            .lock()
            .map(|m| m.clone())
            .map_err(|e| IndexError::Tantivy(format!("sidecar mutex poisoned: {e}")))?;
        write_sidecar(&self.index_dir, &kinds, &quants)?;
        Ok(())
    }

    /// Run a BM25 query against the indexed fields and return up to `k`
    /// hits, sorted by score descending.
    ///
    /// The query is parsed with Tantivy's [`QueryParser`] over the
    /// default-fields list `[name^3, org^2, kind^2, family^2, arch^1,
    /// quants^1, card_snippet^1]`; per-field boosts multiply the user's
    /// own `^N` syntax if they use it.
    pub fn search(&self, q: &str, k: usize) -> Result<Vec<IndexHit>, IndexError> {
        if k == 0 {
            return Ok(Vec::new());
        }

        let mut parser = QueryParser::for_index(
            &self.idx,
            vec![
                self.fields.name,
                self.fields.org,
                self.fields.kind,
                self.fields.family,
                self.fields.arch,
                self.fields.quants,
                self.fields.card_snippet,
            ],
        );
        // Per-field BM25 boost multipliers. The QueryParser multiplies these
        // with any user-supplied ^N.
        parser.set_field_boost(self.fields.name, 3.0);
        parser.set_field_boost(self.fields.org, 2.0);
        parser.set_field_boost(self.fields.kind, 2.0);
        parser.set_field_boost(self.fields.family, 2.0);
        parser.set_field_boost(self.fields.arch, 1.0);
        parser.set_field_boost(self.fields.quants, 1.0);
        parser.set_field_boost(self.fields.card_snippet, 1.0);

        // `parse_query_lenient` returns a "match nothing" subquery for
        // unparseable input instead of erroring out — we still produce
        // *some* BM25 hits if any term is valid.
        let (query, _parse_errors) = parser.parse_query_lenient(q);

        let searcher = self.reader.searcher();
        let top: Vec<(f32, tantivy::DocAddress)> = searcher.search(&*query, &TopDocs::with_limit(k))?;

        if top.is_empty() {
            return Ok(Vec::new());
        }

        let mut hits = Vec::with_capacity(top.len());
        for (score, addr) in top {
            let stored: TantivyDocument = searcher.doc(addr)?;
            // `id` is STRING | STORED; pull the first value.
            let id = stored
                .get_first(self.fields.id)
                .and_then(|v| match v {
                    tantivy::schema::OwnedValue::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            hits.push(IndexHit::new(id, score));
        }
        Ok(hits)
    }
}

// ---------------------------------------------------------------------------
// Sidecar persistence
// ---------------------------------------------------------------------------
//
// The kind/quants lookup tables live in `HashMap`s on the TantivyStore
// handle. They are deliberately kept in-process (per `open`) for speed.
// But that means a CLI invocation cannot see the sidecar that was written
// by a prior `seed build` run, unless we persist it on disk.
//
// We pick a tiny JSON file at `<index>/sidecar.json` (next to Tantivy's
// `meta.json`). The format is intentionally explicit and forward-compatible:
// adding a new field requires a versioned schema bump, not a migration.

/// Sidecar on-disk layout, versioned for forward-compatibility.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Sidecar {
    /// Format version. Bump on backwards-incompatible schema changes.
    #[serde(default = "default_sidecar_version")]
    version: u32,
    /// `id -> kind` map.
    kinds: std::collections::HashMap<String, String>,
    /// `id -> quants` map.
    quants: std::collections::HashMap<String, Vec<String>>,
}

fn default_sidecar_version() -> u32 {
    1
}

fn sidecar_path(index_dir: &Path) -> std::path::PathBuf {
    index_dir.join("sidecar.json")
}

/// Load the sidecar from `<index_dir>/sidecar.json`. A missing file (or any
/// read/parse error) yields empty maps — the tantivy index is the source
/// of truth for the *documents*; the sidecar is only an accelerator.
fn read_sidecar(
    index_dir: &Path,
) -> (
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, Vec<String>>,
) {
    let path = sidecar_path(index_dir);
    let Ok(bytes) = std::fs::read(&path) else {
        return (Default::default(), Default::default());
    };
    let parsed: Result<Sidecar, _> = serde_json::from_slice(&bytes);
    match parsed {
        Ok(s) => (s.kinds, s.quants),
        Err(e) => {
            // Don't fail the open because of a stale sidecar; just start
            // with empty maps and let the next commit overwrite.
            eprintln!(
                "hwledger-search-index: ignoring unreadable sidecar {}: {e}",
                path.display()
            );
            (Default::default(), Default::default())
        }
    }
}

/// Atomically write the sidecar to `<index_dir>/sidecar.json`. We write
/// to a sibling `.tmp` file and rename so a reader never observes a
/// half-written JSON blob.
fn write_sidecar(
    index_dir: &Path,
    kinds: &std::collections::HashMap<String, String>,
    quants: &std::collections::HashMap<String, Vec<String>>,
) -> Result<(), IndexError> {
    let final_path = sidecar_path(index_dir);
    let tmp_path = final_path.with_extension("json.tmp");
    let sidecar = Sidecar {
        version: default_sidecar_version(),
        kinds: kinds.clone(),
        quants: quants.clone(),
    };
    let bytes = serde_json::to_vec(&sidecar)
        .map_err(|e| IndexError::Tantivy(format!("sidecar serialize: {e}")))?;
    std::fs::write(&tmp_path, &bytes)
        .map_err(|e| IndexError::Tantivy(format!("sidecar write {}: {e}", tmp_path.display())))?;
    std::fs::rename(&tmp_path, &final_path).map_err(|e| {
        IndexError::Tantivy(format!(
            "sidecar rename {} -> {}: {e}",
            tmp_path.display(),
            final_path.display()
        ))
    })?;
    Ok(())
}