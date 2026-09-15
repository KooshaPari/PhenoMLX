import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { createApp } from '../app.js';
import {
  synthesizeAsk,
  synthesizeDetail,
  synthesizeSearch,
  synthesizeSimilar,
  synthesizeUseCase,
  UpstreamClient,
} from '../upstream.js';
import type { HealthResponse, ModelAskResponse, SearchResponse } from '../contract.js';

/**
 * Vitest tests for the Hono proxy.
 *
 * The tests are organized in three layers so a failure immediately points
 * at the right code:
 *
 * 1. **app/http** — drives the Hono app via `app.request(...)` and asserts
 *    JSON shaping, status codes, Zod validation, and CORS headers.
 * 2. **synthesized fallback** — exercises the JS-only fallback path so we
 *    know the UI has data even when Rust is offline.
 * 3. **upstream client** — stubs `fetch` to simulate a healthy Rust server,
 *    a server returning 5xx, and a network-down scenario.
 */

const noFetch = vi.fn();

function makeApp(overrides: { fetchImpl?: typeof fetch } = {}) {
  return createApp({
    upstreamUrl: 'http://upstream.invalid:8080',
    upstreamTimeoutMs: 50,
    fetchImpl: overrides.fetchImpl,
  });
}

beforeEach(() => {
  noFetch.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('GET /healthz', () => {
  it('returns a structured liveness payload', async () => {
    const app = makeApp();
    const res = await app.request('/healthz');
    expect(res.status).toBe(200);
    const body = (await res.json()) as HealthResponse;
    expect(body.status).toBe('ok');
    expect(body.upstream).toBe('synthesized');
    expect(body.upstream_url).toMatch(/^http:\/\/upstream\.invalid/);
    expect(res.headers.get('x-upstream')).toBeNull();
  });

  it('exposes CORS headers for the Svelte dev origin', async () => {
    const app = makeApp();
    const res = await app.request('/healthz', {
      headers: { origin: 'http://localhost:5173' },
    });
    expect(res.headers.get('access-control-allow-origin')).toBe(
      'http://localhost:5173',
    );
  });
});

describe('POST /v1/search', () => {
  it('echoes the query and the effective limit in the envelope', async () => {
    const app = makeApp();
    const res = await app.request('/v1/search', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: 'llama', limit: 5 }),
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as SearchResponse;
    expect(body.query).toBe('llama');
    expect(body.limit).toBe(5);
    expect(Array.isArray(body.results)).toBe(true);
    expect(res.headers.get('x-upstream')).toBe('synthesized');
  });

  it('returns 400 with Zod issues on invalid payload', async () => {
    const app = makeApp();
    const res = await app.request('/v1/search', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ limit: 0 }), // limit must be >= 1
    });
    expect(res.status).toBe(400);
    const body = (await res.json()) as { error: string; issues: unknown };
    expect(body.error).toBe('invalid_request');
    expect(body.issues).toBeTruthy();
  });

  it('routes through the synthesized fallback when no fetch impl is provided', async () => {
    const app = makeApp();
    const res = await app.request('/v1/search', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: '', limit: 3 }),
    });
    const body = (await res.json()) as SearchResponse;
    expect(body.results.length).toBeLessThanOrEqual(3);
    // Hits the demo corpus.
    expect(body.results.every((r) => r.id.startsWith('hf::'))).toBe(true);
  });

  it('forwards a healthy upstream response and stamps x-upstream: rust', async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          query: 'rust',
          limit: 25,
          results: [
            {
              id: 'hf::x',
              score: 0.99,
              facets: { kinds: ['instruct'] },
              payload: { name: 'X' },
            },
          ],
        }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      ),
    );
    const app = makeApp({ fetchImpl });
    const res = await app.request('/v1/search', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: 'rust', limit: 25 }),
    });
    expect(fetchImpl).toHaveBeenCalledOnce();
    expect(res.headers.get('x-upstream')).toBe('rust');
    const body = (await res.json()) as SearchResponse;
    expect(body.results[0]?.id).toBe('hf::x');
    expect(body.results[0]?.score).toBeCloseTo(0.99);
  });

  it('falls back to synthesized when the upstream returns non-2xx', async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response('boom', { status: 502 }),
    );
    const app = makeApp({ fetchImpl });
    const res = await app.request('/v1/search', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: 'llama', limit: 2 }),
    });
    expect(res.status).toBe(200);
    expect(res.headers.get('x-upstream')).toBe('synthesized');
    const body = (await res.json()) as SearchResponse;
    expect(body.results.length).toBeGreaterThan(0);
  });
});

describe('GET /v1/models/:id', () => {
  it('returns detail envelope for an unknown id (found=false)', async () => {
    const app = makeApp();
    const res = await app.request('/v1/models/hf::no/such-model');
    expect(res.status).toBe(200);
    const body = (await res.json()) as { id: string; found: boolean };
    expect(body.id).toBe('hf::no/such-model');
    expect(body.found).toBe(false);
  });

  it('rejects malformed ids with a 400', async () => {
    const app = makeApp();
    const res = await app.request('/v1/models/has spaces and bad$chars');
    expect(res.status).toBe(400);
    const body = (await res.json()) as { error: string };
    expect(body.error).toBe('invalid_id');
  });

  it('honors CORS preflight', async () => {
    const app = makeApp();
    const res = await app.request('/v1/models/hf::any', {
      method: 'OPTIONS',
      headers: {
        origin: 'http://localhost:5173',
        'access-control-request-method': 'GET',
      },
    });
    expect([200, 204]).toContain(res.status);
  });
});

describe('GET /v1/use-case/:use_case', () => {
  it('rejects unknown use-case slugs', async () => {
    const app = makeApp();
    const res = await app.request('/v1/use-case/random');
    expect(res.status).toBe(400);
  });

  it('returns use-case results and stamps x-upstream', async () => {
    const app = makeApp();
    const res = await app.request('/v1/use-case/coding?limit=3');
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      use_case: string;
      results: unknown[];
    };
    expect(body.use_case).toBe('coding');
    expect(Array.isArray(body.results)).toBe(true);
    expect(res.headers.get('x-upstream')).toBe('synthesized');
  });
});

describe('POST /v1/models/:id/ask', () => {
  it('returns a stub answer + context list', async () => {
    const app = makeApp();
    const res = await app.request('/v1/models/test-model/ask', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ question: 'embedding models', limit: 2 }),
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as ModelAskResponse;
    expect(body.question).toBe('embedding models');
    expect(body.limit).toBe(2);
    expect(typeof body.answer).toBe('string');
  });

  it('returns 400 on empty question', async () => {
    const app = makeApp();
    const res = await app.request('/v1/models/test/ask', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ question: '' }),
    });
    expect(res.status).toBe(400);
  });
});

describe('synthesized fallback contract', () => {
  it('synthesizeSearch is deterministic and bounded by limit', () => {
    const a = synthesizeSearch({ text: '', limit: 2 });
    const b = synthesizeSearch({ text: '', limit: 2 });
    expect(a.results.map((r) => r.id)).toEqual(b.results.map((r) => r.id));
    expect(a.results.length).toBeLessThanOrEqual(2);
  });

  it('synthesizeDetail records known models', () => {
    const out = synthesizeDetail('hf::Qwen/Qwen2.5-Coder-32B-Instruct');
    expect(out.found).toBe(true);
    expect(out.kind).toBe('coding');
  });

  it('synthesizeSimilar drops the seed from results', () => {
    const out = synthesizeSimilar(
      'hf::meta-llama/Llama-3.1-8B-Instruct',
      5,
    );
    expect(out.results.every((r) => r.id !== 'hf::meta-llama/Llama-3.1-8B-Instruct'))
      .toBe(true);
    expect(out.limit).toBe(5);
  });

  it('synthesizeUseCase only emits rows that match the use case', () => {
    const out = synthesizeUseCase('embedding', '', 50);
    const ids = out.results.map((r) => r.id);
    // The BGE model is the only embedding model in the demo corpus.
    expect(ids).toContain('hf::BAAI/bge-large-en-v1.5');
  });

  it('synthesizeAsk returns no context for nonsense questions', () => {
    const out = synthesizeAsk({ question: 'zzzzqzqzqz', limit: 5 });
    expect(out.context.length).toBe(0);
    expect(out.answer.toLowerCase()).toContain('no results');
  });
});

describe('UpstreamClient fallback behaviour', () => {
  it('falls back when fetch rejects (network error)', async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error('ECONNREFUSED'));
    const client = new UpstreamClient(
      {
        baseUrl: 'http://upstream.invalid:8080',
        timeoutMs: 50,
      },
      fetchImpl,
    );
    const r = await client.search({ text: 'x', limit: 3 });
    expect(r.source).toBe('synthesized');
    expect(r.payload.results.length).toBeGreaterThan(0);
  });

  it('uses rust source when fetch resolves with a valid envelope', async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ results: [{ id: 'hf::a', score: 0.5 }] }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );
    const client = new UpstreamClient(
      {
        baseUrl: 'http://upstream.invalid:8080',
        timeoutMs: 50,
      },
      fetchImpl,
    );
    const r = await client.search({ text: 'x', limit: 3 });
    expect(r.source).toBe('rust');
    expect(r.payload.results[0]?.id).toBe('hf::a');
  });

  it('falls back when fetch resolves with a non-2xx status', async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response('nope', { status: 503 }),
    );
    const client = new UpstreamClient(
      {
        baseUrl: 'http://upstream.invalid:8080',
        timeoutMs: 50,
      },
      fetchImpl,
    );
    const r = await client.detail('hf::a');
    expect(r.source).toBe('synthesized');
    expect(r.payload.found).toBe(false);
  });
});
