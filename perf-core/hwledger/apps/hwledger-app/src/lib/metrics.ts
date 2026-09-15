import type { VariantSummary } from '../types';

const VERIFIED_EVIDENCE = new Set(['live_verified', 'verified']);

export function isVerifiedEvidence(
  cell: { metadata?: Record<string, string> },
): boolean {
  const label = cell.metadata?.evidence_label ?? '';
  return VERIFIED_EVIDENCE.has(label);
}

export function effectiveGenOk(cell: { gen_ok?: number; pass_at_1: number }): number {
  if (cell.gen_ok != null && !Number.isNaN(cell.gen_ok)) return cell.gen_ok;
  return cell.pass_at_1;
}

export function effectiveVerifiedPass(
  cell: { verified_pass_at_1?: number; metadata?: Record<string, string> },
): number | null {
  const v = cell.verified_pass_at_1;
  if (v == null || Number.isNaN(v)) return null;
  if (v > 0 || isVerifiedEvidence(cell)) return v;
  return null;
}

export function hasVerifiedPass(cell: { verified_pass_at_1?: number; metadata?: Record<string, string> }): boolean {
  return effectiveVerifiedPass(cell) != null;
}

export function qualityPass(
  cell: { gen_ok?: number; pass_at_1: number; verified_pass_at_1?: number; metadata?: Record<string, string> },
): number {
  const verified = effectiveVerifiedPass(cell);
  if (verified != null) return verified;
  return effectiveGenOk(cell);
}

export function summaryQualityPass(v: VariantSummary): number {
  const raw = v.verified_pass_at_1;
  if (raw != null && !Number.isNaN(raw) && raw > 0) return raw;
  return v.gen_ok ?? v.pass_at_1;
}

export function summaryQualityLabel(v: VariantSummary, untrusted = false): string {
  const raw = v.verified_pass_at_1;
  if (raw != null && !Number.isNaN(raw) && raw > 0) return 'Verified';
  return untrusted ? 'Gen ok' : 'Pass@1';
}

export function meanQualityPass(cells: { pass_at_1: number; gen_ok?: number; verified_pass_at_1?: number; metadata?: Record<string, string> }[]): number {
  if (!cells.length) return 0;
  return cells.reduce((s, c) => s + qualityPass(c), 0) / cells.length;
}
