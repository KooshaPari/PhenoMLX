/**
 * Shared REST contract for the hwledger model-explorer HTTP surface.
 *
 * These types intentionally mirror the wire shapes used by
 * `hwledger-cli --json` so the proxy is a drop-in front-end for the existing
 * `hwledger-server` binary. They are **the** contract the Svelte UI is built
 * against: changing a field name here requires updating `web/src/lib/types.ts`
 * in lockstep.
 *
 * The structural models loosely track:
 * - `hwledger_search_core::{Query, FusedResult, Facets}`
 * - `hwledger_search_index::IndexedModel`
 *
 * but with one intentional divergence: every collection field is made
 * mutable / null-safe on the JS side so we can render partial responses
 * cleanly while the Rust backend is still implementing endpoints.
 */

/** A single, post-fusion, post-reranking result row. */
export interface ResultRow {
  /** Stable primary key, e.g. `"hf::meta-llama/Llama-3.1-8B"`. */
  id: string;
  /** Final ranking score in `[0, +inf)`. */
  score: number;
  /** Optional facets resolved against the result. */
  facets?: Partial<Facets>;
  /** Optional raw payload — opaque JSON; useful for the preview pane. */
  payload?: Record<string, unknown> | null;
}

/** Facet object mirroring `hwledger_search_core::taxonomy::faceted::Facets`. */
export interface Facets {
  kinds?: string[];
  modalities?: string[];
  arch_kinds?: string[];
  attention_kinds?: Array<
    | 'mha'
    | 'gqa'
    | 'mqa'
    | 'mla'
    | 'sliding'
    | 'ssm'
    | 'hybrid'
    | 'sink'
  >;
  min_param_total?: number | null;
  max_param_total?: number | null;
  min_agentic_fit?: number | null;
  min_coding_fit?: number | null;
  license?: string | null;
  has_evals?: boolean | null;
  quants?: string[];
  provenance?: string | null;
}

/** Full search query, mirroring `hwledger_search_core::Query`. */
export interface SearchRequest {
  /** Free-text query. */
  text?: string;
  /** Structured filter. */
  facets?: Partial<Facets>;
  /** Optional sort hint (e.g. `"downloads"`, `"agentic_fit"`). */
  sort?: string | null;
  /** Hard cap on returned rows. Defaults to 25 in the proxy. */
  limit?: number;
}

/** Standard search response envelope. */
export interface SearchResponse {
  /** Echoed query text. */
  query: string;
  /** Effective limit used for the call. */
  limit: number;
  /** Result rows, descending by score. */
  results: ResultRow[];
}

/** Standard detail envelope (`hwledger-cli model detail --json`). */
export interface ModelDetail {
  id: string;
  found: boolean;
  score?: number | null;
  kind?: string | null;
  quants?: string[] | null;
}

/** Quantization tag list. */
export interface QuantsResponse {
  id: string;
  quants: string[];
}

/** "More like this" envelope. */
export interface SimilarResponse {
  seed: string;
  limit: number;
  results: ResultRow[];
}

/** Use-case scoring envelope. */
export interface UseCaseResponse {
  use_case: 'agentic' | 'coding' | 'reasoning' | 'embedding';
  text: string;
  limit: number;
  results: ResultRow[];
}

/** Natural-language Q&A envelope. */
export interface ModelAskRequest {
  question: string;
  limit?: number;
}

/**
 * A single ranked passage in a model's context bundle. The server returns
 * these in score-descending order so the UI can render them directly as an
 * ordered list. `section` is a stable label like `"card.introduction"` or
 * `"card.evals"` so the UI can show *where* each piece of evidence came from.
 */
export interface ModelAskContext {
  /** Source model id the passage was drawn from. */
  id: string;
  /** Ranking score in `[0, 1]` — higher is more relevant. */
  score: number;
  /** Short snippet shown to the user. */
  snippet: string;
  /** Section label inside the model's card / corpus. */
  section: string;
}

/**
 * Response envelope for `POST /v1/models/:id/ask`. The `context` array is the
 * ranked evidence bundle the UI renders below the answer.
 */
export interface ModelAskResponse {
  /** Echoed model id from the URL path. */
  id: string;
  /** Echoed question from the request body. */
  question: string;
  /** Free-text answer (may be empty when the corpus has no matches). */
  answer: string;
  /** Ranked context-bundle passages used as evidence. */
  context: ModelAskContext[];
}

/** Liveness probe payload. */
export interface HealthResponse {
  status: 'ok';
  upstream: 'rust' | 'synthesized';
  upstream_url: string;
}

/** Versioned use-case route slug → wire string mapping. */
export const USE_CASE_KINDS = {
  agentic: 'agentic',
  coding: 'coding',
  reasoning: 'reasoning',
  embedding: 'embedding',
} as const;

export type UseCaseSlug = keyof typeof USE_CASE_KINDS;
