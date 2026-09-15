import type { Context } from 'hono';
import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { bearerAuth } from 'hono/bearer-auth';
import { logger } from 'hono/logger';
import { prettyJSON } from 'hono/pretty-json';
import { zValidator } from '@hono/zod-validator';
import { HTTPException } from 'hono/http-exception';

import {
  modelAskRequestSchema,
  modelIdSchema,
  searchRequestSchema,
  useCaseSchema,
} from './schemas.js';
import { UpstreamClient } from './upstream.js';
import { swaggerUI } from '@hono/swagger-ui';
import type { HealthResponse } from './contract';
import { getOpenApiSpec } from './openapi';

/**
 * Hono app factory.
 *
 * The factory takes the upstream Rust URL + a `fetch` override so we can
 * fully stub the network in tests. Optionally accepts an API key to
 * protect read endpoints via Bearer auth.
 *
 * Every route validates with Zod, then forwards to the upstream client. On
 * invalid input we return a structured 400 with the Zod issue tree, so the
 * UI can render field-level errors when a search query is malformed.
 *
 * ## Why a `/v1/models/*` catch-all?
 *
 * Our canonical model ids look like `hf::meta-llama/Llama-3.1-8B-Instruct`.
 * Hono's `:id` segment matcher doesn't tolerate slash-containing id segments,
 * but the upstream itself speaks ids with slashes, so we register one
 * catch-all on `/v1/models/*` and dispatch internally based on the trailing
 * path component (`/quants`, `/similar`, or no suffix → detail).
 */
export interface AppDeps {
  /** Full base URL of the Rust `hwledger-server`, e.g. `http://127.0.0.1:8080`. */
  upstreamUrl: string;
  /** Per-request timeout (ms). */
  upstreamTimeoutMs?: number;
  /** Custom `fetch` for tests; defaults to the global `fetch`. */
  fetchImpl?: typeof fetch;
  /** Optional Bearer API key to protect read endpoints. */
  apiKey?: string;
}

/** Per-call envelope returned by the upstream client. */
interface UpstreamEnvelope<T> {
  payload: T;
  source: 'rust' | 'synthesized';
  status: number;
}

type AnyContext = Context;

function jsonError(
  c: AnyContext,
  status: number,
  body: Record<string, unknown>,
) {
  return c.json(body, status as 400);
}

export function createApp(deps: AppDeps) {
  const upstream = new UpstreamClient(
    {
      baseUrl: deps.upstreamUrl,
      timeoutMs: deps.upstreamTimeoutMs,
    },
    deps.fetchImpl ?? fetch.bind(globalThis),
  );

  const app = new Hono();

  // ---- middleware ----
  app.use('*', logger());
  app.use('*', prettyJSON());
  app.use(
    '*',
    cors({
      origin: ['http://localhost:5173', 'http://127.0.0.1:5173'],
      credentials: false,
      allowMethods: ['GET', 'POST', 'OPTIONS'],
    }),
  );

  // Bearer auth on read endpoints when API_KEY is configured.
  if (deps.apiKey) {
    app.use('/v1/*', bearerAuth({ verifyToken: (t) => t === deps.apiKey }));
  }

  // ---- OpenAPI / Swagger UI ----
  // Spec served as JSON at /docs/openapi.json; Swagger UI at /docs
  app.get('/docs/openapi.json', (c) => c.json(getOpenApiSpec()));
  app.get('/docs', swaggerUI({ url: '/docs/openapi.json' }));

  // ---- error handling ----
  app.onError((err, c) => {
    if (err instanceof HTTPException) {
      return err.getResponse();
    }
    return jsonError(c, 500, { error: 'internal_error', message: String(err) });
  });

  // ---- helpers ----
  async function jsonFromUpstream<T>(
    c: AnyContext,
    upstreamResult: UpstreamEnvelope<T>,
  ) {
    c.res.headers.set('x-upstream', upstreamResult.source);
    return c.json(upstreamResult.payload);
  }

  function parseId(raw: string) {
    return modelIdSchema.safeParse(decodeURIComponent(raw));
  }

  // ---- health (always public) ----
  app.get('/healthz', (c) => {
    const body: HealthResponse = {
      status: 'ok',
      upstream: 'synthesized',
      upstream_url: deps.upstreamUrl,
    };
    return c.json(body);
  });

  // ---- OpenAPI spec + Swagger UI (always public; auth is on /v1/* data routes) ----
  // Mounted via .route() so the OpenAPI helper preserves the spec prefix.
  app.get('/docs/openapi.json', (c) => c.json(getOpenApiSpec()));
  app.get('/docs', swaggerUI({ url: '/docs/openapi.json' }));

  // ---- search ----
  app.post(
    '/v1/search',
    zValidator('json', searchRequestSchema, (result, c) => {
      if (!result.success) {
        return jsonError(c, 400, {
          error: 'invalid_request',
          issues: result.error.issues,
        });
      }
      return undefined;
    }),
    async (c) => {
      const req = c.req.valid('json');
      const r = await upstream.search(req);
      return jsonFromUpstream(c, r);
    },
  );

  // ---- model catch-all ----
  // Routes by trailing path component.
  //   GET  /v1/models/{id}             → detail
  //   GET  /v1/models/{id}/quants      → quants
  //   GET  /v1/models/{id}/similar     → "more like this"
  //   POST /v1/models/{id}/ask         → model-scoped Q&A
  // Anything else → 404.
  //
  // We can't use Hono's `:id` parameter because ids contain `/` and
  // Hono's segment parser doesn't tolerate that. Instead we match the
  // rest of the path with `*` and split on the trailing action.
  app.get('/v1/models/*', async (c) => {
    // Strip `/v1/models/` prefix.
    const rest = c.req.path.replace(/^\/v1\/models\//, '');
    let id: string;
    let action: string;
    if (rest.endsWith('/quants')) {
      id = rest.slice(0, -'/quants'.length);
      action = 'quants';
    } else if (rest.endsWith('/similar')) {
      id = rest.slice(0, -'/similar'.length);
      action = 'similar';
    } else if (rest.endsWith('/ask')) {
      id = rest.slice(0, -'/ask'.length);
      action = 'ask';
    } else {
      id = rest;
      action = '';
    }

    const parsed = parseId(id);
    if (!parsed.success) {
      return jsonError(c, 400, {
        error: 'invalid_id',
        issues: parsed.error.issues,
      });
    }
    const validId = parsed.data;

    if (action === '') {
      const r = await upstream.detail(validId);
      return jsonFromUpstream(c, r);
    }
    if (action === 'quants') {
      const r = await upstream.quants(validId);
      return jsonFromUpstream(c, r);
    }
    if (action === 'similar') {
      const limit = Math.max(
        1,
        Math.min(Number(c.req.query('limit') ?? 10) || 10, 50),
      );
      const r = await upstream.similar(validId, limit);
      return jsonFromUpstream(c, r);
    }
    return jsonError(c, 404, { error: 'unknown_action', action });
  });

  // ---- use case ----
  app.get('/v1/use-case/:use_case', async (c) => {
    const parsed = useCaseSchema.safeParse(c.req.param('use_case'));
    if (!parsed.success) {
      return jsonError(c, 400, {
        error: 'invalid_use_case',
        issues: parsed.error.issues,
      });
    }
    const text = (c.req.query('text') ?? '').slice(0, 512);
    const limit = Math.max(
      1,
      Math.min(Number(c.req.query('limit') ?? 10) || 10, 50),
    );
    const r = await upstream.useCase(parsed.data, text, limit);
    return jsonFromUpstream(c, r);
  });

  // ---- model-scoped ask ----
  // POST /v1/models/{id}/ask — same dispatch trick as the GET catch-all:
  // ids contain `/` so we register a single route on `/v1/models/*` and
  // only handle requests whose path ends in `/ask`. Anything else (e.g.
  // a stray POST to `/v1/models/{id}/quants`) falls through to 404.
  app.post(
    '/v1/models/*',
    zValidator('json', modelAskRequestSchema, (result, c) => {
      if (!result.success) {
        return jsonError(c, 400, {
          error: 'invalid_request',
          issues: result.error.issues,
        });
      }
      return undefined;
    }),
    async (c) => {
      const rest = c.req.path.replace(/^\/v1\/models\//, '');
      if (!rest.endsWith('/ask')) {
        return jsonError(c, 404, { error: 'unknown_action', rest });
      }
      const id = rest.slice(0, -'/ask'.length);
      const parsed = parseId(id);
      if (!parsed.success) {
        return jsonError(c, 400, {
          error: 'invalid_id',
          issues: parsed.error.issues,
        });
      }
      const validId = parsed.data;
      const req = { ...c.req.valid('json'), id: validId };
      const r = await upstream.modelAsk(req);
      return jsonFromUpstream(c, r);
    },
  );

  return app;
}

export type App = ReturnType<typeof createApp>;
