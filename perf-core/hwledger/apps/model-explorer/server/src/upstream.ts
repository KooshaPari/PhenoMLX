import type {
  ModelAskRequest,
  ModelAskResponse,
  ModelDetail,
  QuantsResponse,
  ResultRow,
  SearchRequest,
  SearchResponse,
  SimilarResponse,
  UseCaseResponse,
} from './contract.js';

/**
 * Thin HTTP client for the underlying Rust `hwledger-server`.
 *
 * Two transports are supported:
 *
 * 1. **Real** — `fetch(upstream + path)` when the server is reachable. We
 *    fail soft on transport errors (connection refused, timeout) and let the
 *    caller fall back to a synthesized response.
 * 2. **Synthesized** — pure-JS fallback so the proxy can be developed,
 *    tested, and demoed without a running Rust server. The synthesized
 *    shape mirrors the Rust wire format exactly, so consumers (CLI, UI,
 *    tests) never need to know which backend served them.
 */

export interface UpstreamConfig {
  /** Full base URL, e.g. `"http://127.0.0.1:8080"`. */
  baseUrl: string;
  /** Per-request timeout (ms). */
  timeoutMs?: number;
}

/**
 * Mark a request "would normally talk to Rust but didn't" so callers can
 * distinguish real responses from the synthesized fallback in observability
 * + tests. Behaviour identical otherwise.
 */
export interface UpstreamResult<T> {
  payload: T;
  source: 'rust' | 'synthesized';
}

const DEFAULT_TIMEOUT_MS = 4_000;

/** Coerce known numeric / null fields in a response. */
function coerceResultRow(r: unknown): ResultRow {
  if (r === null || typeof r !== 'object') {
    return { id: '', score: 0 };
  }
  const obj = r as Record<string, unknown>;
  const id = typeof obj.id === 'string' ? obj.id : '';
  const score = typeof obj.score === 'number' && Number.isFinite(obj.score)
    ? obj.score
    : 0;
  const facets = (obj.facets && typeof obj.facets === 'object')
    ? (obj.facets as ResultRow['facets'])
    : undefined;
  const payload = (obj.payload && typeof obj.payload === 'object')
    ? (obj.payload as Record<string, unknown>)
    : null;
  return { id, score, ...(facets ? { facets } : {}), payload };
}

/**
 * Internal helper: run an HTTP request with a timeout + try/catch. Returns
 * `null` on any failure (network down, non-2xx, timeout, malformed JSON).
 */
async function tryFetch<T>(
  url: string,
  init: RequestInit,
  timeoutMs: number,
  fetchImpl: typeof fetch,
): Promise<T | null> {
  const ac = new AbortController();
  const timer = setTimeout(() => ac.abort(), timeoutMs);
  try {
    const res = await fetchImpl(url, { ...init, signal: ac.signal });
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/**
 * The actual proxy logic. Each method tries the Rust endpoint first; if it
 * fails, it returns a synthesized response. The synthesized responses are
 * deterministic so the UI renders the same way in dev / CI / preview.
 */
export class UpstreamClient {
  readonly baseUrl: string;
  readonly timeoutMs: number;
  private readonly fetchImpl: typeof fetch;

  constructor(
    cfg: UpstreamConfig,
    fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {
    this.baseUrl = cfg.baseUrl.replace(/\/+$/, '');
    this.timeoutMs = cfg.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    this.fetchImpl = fetchImpl;
  }

  // ------------------------- search -------------------------

  async search(req: SearchRequest): Promise<UpstreamResult<SearchResponse>> {
    const url = `${this.baseUrl}/v1/search`;
    const r = await tryFetch<unknown>(
      url,
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(req),
      },
      this.timeoutMs,
      this.fetchImpl,
    );
    if (r && typeof r === 'object') {
      const obj = r as Partial<SearchResponse> & { results?: unknown };
      if (Array.isArray(obj.results)) {
        return {
          source: 'rust',
          payload: {
            query: typeof obj.query === 'string' ? obj.query : req.text ?? '',
            limit: typeof obj.limit === 'number' ? obj.limit : (req.limit ?? 25),
            results: obj.results.map(coerceResultRow),
          },
        };
      }
    }
    return {
      source: 'synthesized',
      payload: synthesizeSearch(req),
    };
  }

  // ------------------------- detail -------------------------

  async detail(id: string): Promise<UpstreamResult<ModelDetail>> {
    const url = `${this.baseUrl}/v1/models/${encodeURIComponent(id)}`;
    const r = await tryFetch<ModelDetail>(
      url,
      { method: 'GET' },
      this.timeoutMs,
      this.fetchImpl,
    );
    if (r) return { source: 'rust', payload: r };
    return { source: 'synthesized', payload: synthesizeDetail(id) };
  }

  // ------------------------- quants -------------------------

  async quants(id: string): Promise<UpstreamResult<QuantsResponse>> {
    const url = `${this.baseUrl}/v1/models/${encodeURIComponent(id)}/quants`;
    const r = await tryFetch<QuantsResponse>(
      url,
      { method: 'GET' },
      this.timeoutMs,
      this.fetchImpl,
    );
    if (r) return { source: 'rust', payload: r };
    return { source: 'synthesized', payload: synthesizeQuants(id) };
  }

  // ------------------------- similar -------------------------

  async similar(
    id: string,
    limit: number,
  ): Promise<UpstreamResult<SimilarResponse>> {
    const url = `${this.baseUrl}/v1/models/${encodeURIComponent(id)}/similar?limit=${limit}`;
    const r = await tryFetch<SimilarResponse>(
      url,
      { method: 'GET' },
      this.timeoutMs,
      this.fetchImpl,
    );
    if (r) return { source: 'rust', payload: r };
    return { source: 'synthesized', payload: synthesizeSimilar(id, limit) };
  }

  // ------------------------- use case -------------------------

  async useCase(
    useCase: string,
    text: string,
    limit: number,
  ): Promise<UpstreamResult<UseCaseResponse>> {
    const qs = new URLSearchParams({ text, limit: String(limit) });
    const url = `${this.baseUrl}/v1/use-case/${encodeURIComponent(useCase)}?${qs}`;
    const r = await tryFetch<UseCaseResponse>(
      url,
      { method: 'GET' },
      this.timeoutMs,
      this.fetchImpl,
    );
    if (r) return { source: 'rust', payload: r };
    return {
      source: 'synthesized',
      payload: synthesizeUseCase(useCase, text, limit),
    };
  }

  // ------------------------- model-ask -------------------------

  async modelAsk(
    req: ModelAskRequest & { id: string },
  ): Promise<UpstreamResult<ModelAskResponse>> {
    const url = `${this.baseUrl}/v1/models/${encodeURIComponent(req.id)}/ask`;
    const { id: _id, ...body } = req;
    const r = await tryFetch<ModelAskResponse>(
      url,
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      },
      this.timeoutMs,
      this.fetchImpl,
    );
    if (r) return { source: 'rust', payload: r };
    return {
      source: 'synthesized',
      payload: synthesizeAsk(req),
    };
  }
}

// ---------------------------------------------------------------------------
// Synthesized fallbacks — deterministic, testable, JS-only.
// ---------------------------------------------------------------------------

interface DemoRecord {
  id: string;
  name: string;
  org: string;
  kind: string;
  family: string;
  arch: 'gqa' | 'mha' | 'moe' | 'mqa' | 'mla' | 'sliding' | 'ssm' | 'hybrid' | 'sink';
  quants: string[];
  card_snippet: string;
  use_cases: Array<'agentic' | 'coding' | 'reasoning' | 'embedding'>;
}

/**
 * Small built-in demo corpus. Sourced from public Hugging Face
 * `meta-llama`, `Qwen`, `mistralai`, and `BAAI` organizations — only used as
 * fallback data so the UI is navigable when the Rust server is offline.
 */
const DEMO_CORPUS: DemoRecord[] = [
  {
    id: 'hf::meta-llama/Llama-3.1-8B-Instruct',
    name: 'Llama-3.1-8B-Instruct',
    org: 'meta-llama',
    kind: 'instruct',
    family: 'llama',
    arch: 'gqa',
    quants: ['gguf', 'gptq', 'awq'],
    card_snippet:
      'Llama 3.1 is a multilingual large language model. The 8B variant is instruct-tuned.',
    use_cases: ['agentic', 'coding', 'reasoning'],
  },
  {
    id: 'hf::Qwen/Qwen2.5-Coder-32B-Instruct',
    name: 'Qwen2.5-Coder-32B-Instruct',
    org: 'Qwen',
    kind: 'coding',
    family: 'qwen2',
    arch: 'gqa',
    quants: ['gguf', 'awq'],
    card_snippet:
      'Code-focused variant of Qwen 2.5. Strong on multi-file edits and tool use.',
    use_cases: ['coding', 'agentic'],
  },
  {
    id: 'hf::deepseek-ai/DeepSeek-R1-Distill-Qwen-32B',
    name: 'DeepSeek-R1-Distill-Qwen-32B',
    org: 'deepseek-ai',
    kind: 'reasoning',
    family: 'qwen2',
    arch: 'gqa',
    quants: ['gguf', 'gptq'],
    card_snippet:
      'A reasoning-tuned variant distilled from DeepSeek R1 into a Qwen 2.5 32B base.',
    use_cases: ['reasoning'],
  },
  {
    id: 'hf::mistralai/Mistral-7B-Instruct-v0.3',
    name: 'Mistral-7B-Instruct-v0.3',
    org: 'mistralai',
    kind: 'instruct',
    family: 'mistral',
    arch: 'sliding',
    quants: ['gguf', 'gptq', 'awq'],
    card_snippet:
      'A 7B-parameter instruct-tuned model with sliding-window attention.',
    use_cases: ['agentic'],
  },
  {
    id: 'hf::BAAI/bge-large-en-v1.5',
    name: 'BGE-large-en-v1.5',
    org: 'BAAI',
    kind: 'embedding',
    family: 'bert',
    arch: 'mha',
    quants: ['gguf'],
    card_snippet:
      'BGE embedding model — dense vector representations for English text.',
    use_cases: ['embedding'],
  },
  {
    id: 'hf::meta-llama/Llama-3.2-3B-Instruct',
    name: 'Llama-3.2-3B-Instruct',
    org: 'meta-llama',
    kind: 'instruct',
    family: 'llama',
    arch: 'gqa',
    quants: ['gguf', 'gptq'],
    card_snippet:
      'Compact 3B instruct model from the Llama 3.2 release wave.',
    use_cases: ['agentic', 'coding'],
  },
];

/** Stable id lookup. */
function findRecord(id: string): DemoRecord | undefined {
  const target = id.replace(/^hf::/, '');
  return DEMO_CORPUS.find(
    (r) => r.id === id || r.id.endsWith(`::${target}`),
  );
}

/** Tiny seeded PRNG so synthesized scores are stable across runs. */
function scoreFor(seed: string, salt = ''): number {
  let h = 2166136261;
  const input = `${seed}::${salt}`;
  for (let i = 0; i < input.length; i++) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  // Map onto (0, 1).
  const v = (h >>> 0) / 0xffffffff;
  return Math.round(v * 1000) / 1000;
}

export function synthesizeSearch(req: SearchRequest): SearchResponse {
  const text = (req.text ?? '').trim().toLowerCase();
  const kinds = req.facets?.kinds?.map((k) => k.toLowerCase()) ?? [];
  const matches = DEMO_CORPUS.filter((r) => {
    if (kinds.length && !kinds.includes(r.kind)) return false;
    if (!text) return true;
    return (
      r.name.toLowerCase().includes(text) ||
      r.card_snippet.toLowerCase().includes(text) ||
      r.org.toLowerCase().includes(text)
    );
  });
  const limit = Math.max(1, Math.min(req.limit ?? 25, 200));
  const results: ResultRow[] = matches
    .slice(0, limit)
    .map((r, i) => ({
      id: r.id,
      score: scoreFor(r.id, `text:${text}:idx:${i}`),
      facets: { kinds: [r.kind] },
      payload: {
        name: r.name,
        org: r.org,
        family: r.family,
        arch: r.arch,
        quants: r.quants,
        card_snippet: r.card_snippet,
      },
    }));
  return { query: req.text ?? '', limit, results };
}

export function synthesizeDetail(id: string): ModelDetail {
  const r = findRecord(id);
  if (!r) {
    return { id, found: false };
  }
  return {
    id: r.id,
    found: true,
    score: scoreFor(r.id, 'detail'),
    kind: r.kind,
    quants: r.quants,
  };
}

export function synthesizeQuants(id: string): QuantsResponse {
  const r = findRecord(id);
  return { id, quants: r?.quants ?? [] };
}

export function synthesizeSimilar(id: string, limit: number): SimilarResponse {
  const seed = findRecord(id);
  const candidates = DEMO_CORPUS.filter((r) => r.id !== (seed?.id ?? id));
  const cap = Math.max(1, Math.min(limit, 50));
  const results: ResultRow[] = candidates
    .sort((a, b) =>
      seed
        ? a.use_cases.filter((u) => seed.use_cases.includes(u)).length -
          b.use_cases.filter((u) => seed.use_cases.includes(u)).length
        : a.id.localeCompare(b.id),
    )
    .slice(0, cap)
    .map((r, i) => ({
      id: r.id,
      score: scoreFor(r.id, `sim:${id}:idx:${i}`),
      facets: { kinds: [r.kind] },
      payload: { name: r.name, kind: r.kind, arch: r.arch },
    }));
  return { seed: id, limit: cap, results };
}

export function synthesizeUseCase(
  useCase: string,
  text: string,
  limit: number,
): UseCaseResponse {
  const cap = Math.max(1, Math.min(limit, 50));
  const uc = useCase as DemoRecord['use_cases'][number];
  const matches = DEMO_CORPUS.filter((r) => r.use_cases.includes(uc));
  const t = text.trim().toLowerCase();
  const filtered = t
    ? matches.filter((r) =>
        (r.name + r.card_snippet + r.org).toLowerCase().includes(t),
      )
    : matches;
  const results: ResultRow[] = filtered.slice(0, cap).map((r, i) => ({
    id: r.id,
    score: scoreFor(r.id, `uc:${uc}:${t}:idx:${i}`),
    facets: { kinds: [r.kind] },
    payload: { name: r.name, kind: r.kind },
  }));
  return { use_case: uc, text, limit: cap, results };
}

export function synthesizeAsk(req: ModelAskRequest): ModelAskResponse {
  const limit = Math.max(1, Math.min(req.limit ?? 5, 50));
  const t = req.question.toLowerCase();
  const tokens = t.split(/\s+/).filter(Boolean);
  const ranked = DEMO_CORPUS
    .map((r) => {
      const hay = `${r.name} ${r.card_snippet} ${r.org}`.toLowerCase();
      const hits = tokens.reduce(
        (acc, tok) => acc + (hay.includes(tok) ? 1 : 0),
        0,
      );
      return { r, hits };
    })
    .sort((a, b) => b.hits - a.hits);
  const top = ranked.slice(0, limit).filter((x) => x.hits > 0);
  const context = top.map(({ r }) => ({
    id: r.id,
    score: scoreFor(r.id, `ask:${t}`),
    snippet: r.card_snippet.slice(0, 240),
  }));
  return {
    question: req.question,
    limit,
    answer: context.length
      ? `Synthesized stub: matched ${context.length} result(s) for "${req.question}".`
      : `Synthesized stub: no results for "${req.question}".`,
    context,
  };
}
