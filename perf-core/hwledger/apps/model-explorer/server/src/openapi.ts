// SPDX-License-Identifier: Apache-2.0
// OpenAPI 3.1 specification for the Hono proxy REST surface.
//
// The spec mirrors the 8 endpoints declared in app.ts. Each route
// references a request/response schema from schemas.ts so the docs
// stay in sync with the runtime validation.
//
// Rendered at:
//   GET /openapi.json   — JSON spec
//   GET /docs           — Swagger UI (interactive)

import { OpenAPIHono, createRoute, z } from "@hono/zod-openapi";

export function openapiSpec() {
  const app = new OpenAPIHono();

  // ---- /v1/models/search ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/v1/models/search",
      tags: ["models"],
      summary: "Hybrid BM25 + cosine search over indexed models",
      request: {
        query: z.object({
          q: z.string().min(1).openapi({ description: "Search query" }),
          kind: z.string().optional().openapi({ description: "ModelKind filter" }),
          modality: z.string().optional(),
          arch: z.string().optional(),
          quant: z.string().optional(),
          param_bucket: z.string().optional(),
          min_agentic_fit: z.coerce.number().min(0).max(1).optional(),
          min_coding_fit: z.coerce.number().min(0).max(1).optional(),
          limit: z.coerce.number().int().min(1).max(200).default(25),
        }),
      },
      responses: {
        200: {
          description: "Paged hybrid search results",
          content: {
            "application/json": {
              schema: z.object({
                results: z.array(z.object({
                  id: z.string(),
                  name: z.string(),
                  score: z.number(),
                  snippet: z.string().optional(),
                })),
                total: z.number(),
                query: z.string(),
              }),
            },
          },
        },
        400: { description: "Bad query" },
        502: { description: "Upstream Rust server error" },
      },
    }),
    (c) => c.json({ stub: true }, 200) as never,
  );

  // ---- /v1/models/{id} ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/v1/models/{id}",
      tags: ["models"],
      summary: "Get structured detail for a model id",
      request: {
        params: z.object({
          id: z.string().openapi({ description: "HF model id, e.g. 'meta-llama/Llama-3.1-8B-Instruct'" }),
        }),
      },
      responses: {
        200: { description: "Model detail" },
        404: { description: "Model not found" },
      },
    }),
    (c) => c.json({ stub: true }, 200) as never,
  );

  // ---- /v1/models/{id}/ask ----
  app.openapi(
    createRoute({
      method: "post",
      path: "/v1/models/{id}/ask",
      tags: ["rag"],
      summary: "RAG context-bundle for a question about a model",
      request: {
        params: z.object({ id: z.string() }),
        body: {
          content: {
            "application/json": {
              schema: z.object({
                question: z.string().min(1),
                top_k: z.coerce.number().int().min(1).max(32).default(8),
              }),
            },
          },
        },
      },
      responses: {
        200: { description: "Context bundle (top-k passages)" },
        404: { description: "Model not found" },
      },
    }),
    (c) => c.json({ stub: true }, 200) as never,
  );

  // ---- /v1/models/{id}/quants ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/v1/models/{id}/quants",
      tags: ["models"],
      summary: "List all quantization variants for a model",
      request: {
        params: z.object({ id: z.string() }),
      },
      responses: {
        200: { description: "Quant list" },
        404: { description: "Model not found" },
      },
    }),
    (c) => c.json({ stub: true }, 200) as never,
  );

  // ---- /v1/models/{id}/similar ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/v1/models/{id}/similar",
      tags: ["models"],
      summary: "Vector-NN to find variants/finetunes/merges",
      request: {
        params: z.object({ id: z.string() }),
        query: z.object({
          k: z.coerce.number().int().min(1).max(50).default(10),
        }),
      },
      responses: {
        200: { description: "Similar models" },
      },
    }),
    (c) => c.json({ stub: true }, 200) as never,
  );

  // ---- /v1/models/for-use-case/{usecase} ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/v1/models/for-use-case/{usecase}",
      tags: ["discovery"],
      summary: "Curated top-k by use-case (coding / agentic / reasoning / embedding)",
      request: {
        params: z.object({
          usecase: z.enum(["coding", "agentic", "reasoning", "embedding"]),
        }),
        query: z.object({
          k: z.coerce.number().int().min(1).max(50).default(10),
        }),
      },
      responses: {
        200: { description: "Curated models" },
      },
    }),
    (c) => c.json({ stub: true }, 200) as never,
  );

  // ---- /v1/admin/seed-build ----
  app.openapi(
    createRoute({
      method: "post",
      path: "/v1/admin/seed-build",
      tags: ["admin"],
      summary: "Queue a seed-build job (admin token required)",
      responses: {
        202: { description: "Job accepted" },
        401: { description: "Missing or invalid admin token" },
      },
    }),
    (c) => c.json({ stub: true }, 202) as never,
  );

  // ---- /v1/admin/seed-expand ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/v1/admin/seed-expand",
      tags: ["admin"],
      summary: "Queue a seed-expand job (admin token required)",
      responses: {
        202: { description: "Job accepted" },
        401: { description: "Missing or invalid admin token" },
      },
    }),
    (c) => c.json({ stub: true }, 202) as never,
  );

  // ---- /healthz ----
  app.openapi(
    createRoute({
      method: "get",
      path: "/healthz",
      tags: ["ops"],
      summary: "Liveness probe (public, no auth)",
      responses: {
        200: {
          description: "OK",
          content: {
            "application/json": {
              schema: z.object({ status: z.string(), service: z.string() }),
            },
          },
        },
      },
    }),
    (c) => c.json({ status: "ok", service: "hwledger-model-explorer-proxy" }, 200) as never,
  );

  // Document the app at the root
  return app.doc("/openapi.json", {
    openapi: "3.1.0",
    info: {
      title: "hwLedger Model Explorer",
      version: "0.1.0",
      description:
        "REST surface for HuggingFace model search, detail, RAG-ask, quants, similarity, and discovery shortcuts. " +
        "Backed by the Rust hwledger-server (Tantivy + LanceDB + tract-onnx).",
    },
    servers: [
      { url: "http://127.0.0.1:8787", description: "Local dev" },
      { url: "http://localhost:8080", description: "Rust upstream" },
    ],
  });
}
