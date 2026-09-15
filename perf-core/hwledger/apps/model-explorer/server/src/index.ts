import { serve } from '@hono/node-server';

import { createApp } from './app.js';

/**
 * Process-level bootstrap.
 *
 * Honors the same env vars the rest of the fleet uses (per ADR-035A env
 * contract): `HWLEDGER_UPSTREAM_URL`, `PORT`. `HOST` is also accepted so
 * containerized deployments can opt into `0.0.0.0`.
 */

const PORT = Number(process.env.PORT ?? 8787);
const HOST = process.env.HOST ?? '127.0.0.1';
const UPSTREAM_URL = process.env.HWLEDGER_UPSTREAM_URL ?? 'http://127.0.0.1:8080';
const UPSTREAM_TIMEOUT = Number(process.env.HWLEDGER_UPSTREAM_TIMEOUT_MS ?? 4_000);

const app = createApp({
  upstreamUrl: UPSTREAM_URL,
  upstreamTimeoutMs: UPSTREAM_TIMEOUT,
});

const server = serve({ fetch: app.fetch, port: PORT, hostname: HOST }, (info) => {
  // eslint-disable-next-line no-console
  console.log(
    `[model-explorer] Hono proxy listening on http://${info.address}:${info.port} → ${UPSTREAM_URL}`,
  );
});

const shutdown = (signal: NodeJS.Signals) => {
  // eslint-disable-next-line no-console
  console.log(`[model-explorer] received ${signal}, shutting down`);
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(1), 5_000).unref();
};

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
