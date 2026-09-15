/**
 * Tests for the ApiClient. We stub `fetch` per-test so we can drive the
 * client through success / failure / JSON-parse / query-string code paths
 * without touching the network.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

import { ApiClient, ApiError } from './api.js';
import type {
  HealthResponse,
  ModelAskContext,
  ModelAskResponse,
  SearchRequest,
  SearchResponse,
  UseCaseSlug,
} from './types.js';

// ---- helpers --------------------------------------------------------------------------

interface FetchCall {
  url: string;
  init: RequestInit | undefined;
}

/**
 * Build a `fetch` stub that records the most recent call and returns
 * whatever response the test queued. The queued responses are consumed
 * FIFO so a test can stage `search() → detail()` in one block.
 */
function stagedFetch(
  responses: Array<{ status?: number; body?: unknown; contentType?: string }>,
) {
  const calls: FetchCall[] = [];
  const fn = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
    const u = typeof url === 'string' ? url : url.toString();
    calls.push({ url: u, init });
    const r = responses.shift() ?? { status: 200, body: {} };
    const status = r.status ?? 200;
    const ct = r.contentType ?? 'application/json';
    const body =
      ct.includes('json')
        ? JSON.stringify(r.body ?? {})
        : String(r.body ?? '');
    return new Response(body, {
      status,
      headers: { 'content-type': ct },
    });
  });
  return Object.assign(fn, { calls }) as typeof fn & { calls: FetchCall[] };
}

// ---- tests ----------------------------------------------------------------------------

describe('ApiClient', () => {
  let fetchStub: ReturnType<typeof stagedFetch>;
  let client: ApiClient;

  beforeEach(() => {
    fetchStub = stagedFetch([]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });
  });

  it('health() GETs /healthz and parses the JSON envelope', async () => {
    fetchStub = stagedFetch([{ body: { status: 'ok', upstream: 'rust', upstream_url: 'http://x' } }]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    const out: HealthResponse = await client.health();
    expect(fetchStub.calls).toHaveLength(1);
    expect(fetchStub.calls[0].url).toBe('http://proxy.test/healthz');
    expect(fetchStub.calls[0].init?.method).toBe('GET');
    expect(out.status).toBe('ok');
    expect(out.upstream).toBe('rust');
  });

  it('search() POSTs the SearchRequest as JSON', async () => {
    const req: SearchRequest = {
      text: 'llama coder',
      limit: 10,
      facets: { kinds: ['instruct'], license: 'apache-2.0' },
    };
    fetchStub = stagedFetch([
      {
        body: {
          query: 'llama coder',
          limit: 10,
          results: [
            { id: 'hf::meta-llama/CodeLlama', score: 0.91, payload: { x: 1 } },
          ],
        },
      },
    ]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    const r: SearchResponse = await client.search(req);
    const call = fetchStub.calls[0];
    expect(call.url).toBe('http://proxy.test/v1/search');
    expect(call.init?.method).toBe('POST');
    expect(call.init?.headers).toMatchObject({
      accept: 'application/json',
      'content-type': 'application/json',
    });
    expect(JSON.parse(String(call.init?.body))).toEqual(req);
    expect(r.results[0].id).toBe('hf::meta-llama/CodeLlama');
    expect(r.results[0].score).toBeCloseTo(0.91);
  });

  it('detail() URL-encodes ids containing slashes', async () => {
    fetchStub = stagedFetch([{ body: { id: 'hf::meta-llama/Llama', found: true, kind: 'instruct' } }]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    const r = await client.detail('hf::meta-llama/Llama');
    expect(fetchStub.calls[0].url).toBe('http://proxy.test/v1/models/hf%3A%3Ameta-llama%2FLlama');
    expect(r.found).toBe(true);
    expect(r.kind).toBe('instruct');
  });

  it('useCase() forwards slug, text, and limit via query string', async () => {
    fetchStub = stagedFetch([
      {
        body: {
          use_case: 'coding',
          text: 'code completion',
          limit: 5,
          results: [{ id: 'hf::x', score: 0.5 }],
        },
      },
    ]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    const slug: UseCaseSlug = 'coding';
    const r = await client.useCase(slug, 'code completion', 5);
    const url = new URL(fetchStub.calls[0].url);
    expect(url.pathname).toBe('/v1/use-case/coding');
    expect(url.searchParams.get('text')).toBe('code completion');
    expect(url.searchParams.get('limit')).toBe('5');
    expect(r.use_case).toBe('coding');
    expect(r.results[0].id).toBe('hf::x');
  });

  it('throws ApiError on non-2xx responses with parsed JSON body', async () => {
    // Persistent stub — every call gets the same 400 envelope.
    const fn = vi.fn(async () =>
      new Response(JSON.stringify({ error: 'invalid facets', field: 'min_param_total' }), {
        status: 400,
        headers: { 'content-type': 'application/json' },
      }),
    );
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fn as unknown as typeof fetch });

    await expect(client.health()).rejects.toBeInstanceOf(ApiError);
    try {
      await client.health();
    } catch (e) {
      expect(e).toBeInstanceOf(ApiError);
      const apiErr = e as ApiError;
      expect(apiErr.status).toBe(400);
      expect(apiErr.body).toMatchObject({ error: 'invalid facets' });
      expect(apiErr.url).toBe('http://proxy.test/healthz');
    }
  });

  it('modelAsk(id, question) POSTs the question to /v1/models/:id/ask', async () => {
    const id = 'hf::Qwen/Qwen2.5-Coder-32B-Instruct';
    const question = 'best coder model?';
    const passages: ModelAskContext[] = [
      {
        id: 'hf::Qwen/Qwen2.5-Coder-32B-Instruct',
        score: 0.91,
        snippet: 'Code-focused variant of Qwen 2.5. Strong on multi-file edits and tool use.',
        section: 'card.introduction',
      },
      {
        id: 'hf::meta-llama/CodeLlama-34B',
        score: 0.74,
        snippet: 'Code Llama is a family of large language models for code generation.',
        section: 'card.introduction',
      },
    ];
    const body: ModelAskResponse = {
      id,
      question,
      answer: 'Try Qwen2.5-Coder-32B-Instruct.',
      context: passages,
    };
    fetchStub = stagedFetch([{ body }]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    const r = await client.modelAsk(id, question);
    const call = fetchStub.calls[0];
    expect(call.init?.method).toBe('POST');
    expect(call.url).toBe(
      `http://proxy.test/v1/models/${encodeURIComponent(id)}/ask`,
    );
    expect(call.init?.headers).toMatchObject({
      accept: 'application/json',
      'content-type': 'application/json',
    });
    expect(JSON.parse(String(call.init?.body))).toEqual({ question });
    expect(r.id).toBe(id);
    expect(r.question).toBe(question);
    expect(r.answer).toContain('Qwen2.5-Coder-32B-Instruct');
    expect(r.context).toHaveLength(2);
    expect(r.context[0].score).toBeCloseTo(0.91);
    expect(r.context[0].section).toBe('card.introduction');
  });

  it('modelAsk() URL-encodes ids containing slashes and colons', async () => {
    fetchStub = stagedFetch([
      {
        body: {
          id: 'hf::meta-llama/Llama-3.1-8B-Instruct',
          question: 'q',
          answer: 'a',
          context: [],
        },
      },
    ]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    await client.modelAsk('hf::meta-llama/Llama-3.1-8B-Instruct', 'q');
    const url = new URL(fetchStub.calls[0].url);
    expect(url.pathname).toBe(
      `/v1/models/${encodeURIComponent('hf::meta-llama/Llama-3.1-8B-Instruct')}/ask`,
    );
  });

  it('modelAsk() context passages are returned in score-ranked order', async () => {
    const id = 'hf::x/y';
    fetchStub = stagedFetch([
      {
        body: {
          id,
          question: 'rank?',
          answer: 'top-1',
          context: [
            { id: 'hf::a', score: 0.62, snippet: 'mid', section: 'card.usage' },
            { id: 'hf::b', score: 0.88, snippet: 'top', section: 'card.introduction' },
            { id: 'hf::c', score: 0.41, snippet: 'low', section: 'card.evals' },
          ],
        },
      },
    ]);
    client = new ApiClient({ baseUrl: 'http://proxy.test', fetchImpl: fetchStub });

    const r = await client.modelAsk(id, 'rank?');
    const scores = r.context.map((c) => c.score);
    // Server contract: passages arrive ranked, so the scores must be
    // monotonically non-increasing as the UI iterates over them.
    for (let i = 1; i < scores.length; i++) {
      expect(scores[i]).toBeLessThanOrEqual(scores[i - 1]);
    }
    expect(r.context[0].id).toBe('hf::b');
    expect(r.context[0].section).toBe('card.introduction');
  });

  it('modelAsk() throws ApiError on upstream 4xx with parsed body', async () => {
    const fn = vi.fn(async () =>
      new Response(
        JSON.stringify({ error: 'invalid_request', field: 'question' }),
        { status: 400, headers: { 'content-type': 'application/json' } },
      ),
    );
    client = new ApiClient({
      baseUrl: 'http://proxy.test',
      fetchImpl: fn as unknown as typeof fetch,
    });

    await expect(client.modelAsk('hf::x/y', 'q')).rejects.toBeInstanceOf(ApiError);
    try {
      await client.modelAsk('hf::x/y', 'q');
    } catch (e) {
      const apiErr = e as ApiError;
      expect(apiErr.status).toBe(400);
      expect(apiErr.body).toMatchObject({ error: 'invalid_request' });
      expect(apiErr.url).toMatch(/\/v1\/models\/hf%3A%3Ax%2Fy\/ask$/);
    }
  });
});
