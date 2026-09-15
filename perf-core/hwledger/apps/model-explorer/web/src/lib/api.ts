import type {
  HealthResponse,
  ModelAskResponse,
  ModelDetail,
  QuantsResponse,
  SearchRequest,
  SearchResponse,
  SimilarResponse,
  UseCaseSlug,
} from './types.js';

/**
 * API client for the Hono proxy.
 *
 * `baseUrl` is left empty when the page is rendered on the SvelteKit
 * origin — Vite's dev proxy (and the production reverse-proxy / same-
 * origin deployment) forwards `/v1/*` calls directly. In unit tests we
 * pin `baseUrl` to an absolute URL.
 *
 * Every method throws `ApiError` on non-2xx responses; consumers can
 * `try { … } catch (e: ApiError) { … }` and surface a sensible message.
 */

export class ApiError extends Error {
  readonly status: number;
  readonly body: unknown;
  readonly url: string;

  constructor(status: number, url: string, body: unknown, message?: string) {
    super(message ?? `API ${status} on ${url}`);
    this.name = 'ApiError';
    this.status = status;
    this.url = url;
    this.body = body;
  }
}

export interface ApiClientOptions {
  /** Override the base URL — defaults to same-origin (`""`). */
  baseUrl?: string;
  /** Per-request fetch override (tests + SvelteKit load hooks). */
  fetchImpl?: typeof fetch;
}

export class ApiClient {
  readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(opts: ApiClientOptions = {}) {
    this.baseUrl = (opts.baseUrl ?? '').replace(/\/+$/, '');
    this.fetchImpl = opts.fetchImpl ?? fetch.bind(globalThis);
  }

  /** Build a fully-qualified URL from a path. */
  url(path: string): string {
    if (path.startsWith('http://') || path.startsWith('https://')) {
      return path;
    }
    if (!path.startsWith('/')) path = `/${path}`;
    return `${this.baseUrl}${path}`;
  }

  async health(): Promise<HealthResponse> {
    return this.#request<HealthResponse>('GET', '/healthz');
  }

  async search(req: SearchRequest): Promise<SearchResponse> {
    return this.#request<SearchResponse>('POST', '/v1/search', req);
  }

  async detail(id: string): Promise<ModelDetail> {
    return this.#request<ModelDetail>('GET', `/v1/models/${encodeURIComponent(id)}`);
  }

  async quants(id: string): Promise<QuantsResponse> {
    return this.#request<QuantsResponse>(
      'GET',
      `/v1/models/${encodeURIComponent(id)}/quants`,
    );
  }

  async similar(id: string, limit = 10): Promise<SimilarResponse> {
    return this.#request<SimilarResponse>(
      'GET',
      `/v1/models/${encodeURIComponent(id)}/similar?limit=${limit}`,
    );
  }

  async useCase(
    useCase: UseCaseSlug,
    text = '',
    limit = 10,
  ): Promise<SearchResponse & { use_case: UseCaseSlug }> {
    const qs = new URLSearchParams({ text, limit: String(limit) });
    const r = await this.#request<{
      use_case: UseCaseSlug;
      text: string;
      limit: number;
      results: SearchResponse['results'];
    }>('GET', `/v1/use-case/${useCase}?${qs.toString()}`);
    return {
      query: r.text,
      limit: r.limit,
      results: r.results,
      use_case: r.use_case,
    };
  }

  /**
   * Ask a natural-language question about a specific model.
   *
   * POSTs to `/v1/models/:id/ask` (the id is encoded into the path so the
   * upstream can scope retrieval to that model's card + corpus) with a
   * `{ question }` body. The server returns the synthesized answer plus a
   * ranked context bundle of passages used as evidence.
   *
   * @param id      Stable model id (e.g. `hf::meta-llama/Llama-3.1-8B-Instruct`).
   * @param question  Free-text question about the model.
   */
  async modelAsk(id: string, question: string): Promise<ModelAskResponse> {
    const raw = await this.#request<ModelAskResponse>(
      'POST',
      `/v1/models/${encodeURIComponent(id)}/ask`,
      { question },
    );
    // Enforce score-descending sort contract so consumers always get
    // passages in ranked order regardless of upstream ordering.
    const sorted = (raw.context ?? [])
      .slice()
      .sort((a, b) => (b.score ?? 0) - (a.score ?? 0));
    return { ...raw, context: sorted };
  }

  /** Internal request helper. Throws ApiError on non-2xx, returns parsed JSON on 2xx. */
  async #request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const url = this.url(path);
    const init: RequestInit = {
      method,
      headers: {
        accept: 'application/json',
        ...(body === undefined ? {} : { 'content-type': 'application/json' }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    };
    const res = await this.fetchImpl(url, init);
    let parsed: unknown = null;
    const ct = res.headers.get('content-type') ?? '';
    if (ct.includes('application/json')) {
      try {
        parsed = await res.json();
      } catch {
        parsed = null;
      }
    } else {
      parsed = await res.text();
    }
    if (!res.ok) {
      throw new ApiError(res.status, url, parsed);
    }
    return parsed as T;
  }
}

/** Default module-scoped client used by components. */
export const api = new ApiClient({
  baseUrl: import.meta.env?.PUBLIC_MODEL_EXPLORER_API ?? '',
});
