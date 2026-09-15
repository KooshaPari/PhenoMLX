import { z } from 'zod';

/**
 * Zod request schemas for the Hono proxy.
 *
 * These match the structural shape of `hwledger_search_core::Query` /
 * `Facets`, but with safer coercion (`Number`-casting query-string params,
 * default limit, etc.) and stricter bounds so a malicious caller can't keep
 * the backend busy with `limit = 1_000_000`.
 */

/** String→number coercion for query params; rejects empty + non-finite. */
const coerceFiniteInt = (min: number, max: number, fallback?: number) =>
  z
    .preprocess((v) => {
      if (v === undefined || v === null || v === '') return fallback;
      const n = typeof v === 'string' ? Number(v) : (v as number);
      return Number.isFinite(n) ? n : NaN;
    }, z.number().int().min(min).max(max))
    .optional();

/** Facets schema — every collection is OR, every scalar is inclusive. */
export const facetsSchema = z
  .object({
    kinds: z.array(z.string()).optional(),
    modalities: z.array(z.string()).optional(),
    arch_kinds: z.array(z.string()).optional(),
    attention_kinds: z
      .array(
        z.enum([
          'mha',
          'gqa',
          'mqa',
          'mla',
          'sliding',
          'ssm',
          'hybrid',
          'sink',
        ]),
      )
      .optional(),
    min_param_total: coerceFiniteInt(0, 1e13),
    max_param_total: coerceFiniteInt(0, 1e13),
    min_agentic_fit: z.coerce.number().min(0).max(1).optional(),
    min_coding_fit: z.coerce.number().min(0).max(1).optional(),
    license: z.string().min(1).max(64).optional(),
    has_evals: z.coerce.boolean().optional(),
    quants: z.array(z.string()).optional(),
    provenance: z.string().min(1).max(32).optional(),
  })
  .strict();

/** Full search request body for `POST /v1/search`. */
export const searchRequestSchema = z
  .object({
    text: z.string().max(2048).optional().default(''),
    facets: facetsSchema.optional().default({}),
    sort: z.string().max(64).nullable().optional(),
    limit: coerceFiniteInt(1, 200, 25),
  })
  .strict();

/** `model-ask` body. */
export const modelAskRequestSchema = z
  .object({
    question: z.string().min(1).max(2048),
    limit: coerceFiniteInt(1, 50, 5),
  })
  .strict();

/** Use-case slug, route-param style. */
export const useCaseSchema = z.enum([
  'agentic',
  'coding',
  'reasoning',
  'embedding',
]);

/** API_KEY env-var schema. */
export const appEnvSchema = z.object({
  apiKey: z.string().min(1).optional(),
}).strict();

/** Model id path-param schema. */
export const modelIdSchema = z
  .string()
  .min(1)
  .max(512)
  .regex(/^[A-Za-z0-9._\-/:&@+]+$/, 'invalid id characters');
