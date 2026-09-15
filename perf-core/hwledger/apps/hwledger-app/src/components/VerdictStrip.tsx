import React from 'react';
import type { VariantSummary } from '../types';
import { summaryQualityPass, summaryQualityLabel } from '../lib/metrics';

export const EMPTY_VARIANT_SUMMARY: VariantSummary = {
  pass_at_1: 0,
  gen_ok: 0,
  verified_pass_at_1: 0,
  mean_wall_clock_s: 0,
  mean_partial_credit: 0,
  mean_format_compliance: 0,
  n_hallucinations: 0,
  mean_tokens_read: 0,
  mean_cost_usd: 0,
  mean_peak_rss_mb: 0,
  mean_energy_joules: 0,
  mean_first_token_ms: 0,
  mean_retry_count: 0,
  success_rate: 0,
  timeout_rate: 0,
};

interface Props {
  summary: { stock: VariantSummary; ours: VariantSummary };
  statusText: string;
  statusLevel: 'connected' | 'error';
  passAt1Untrusted?: boolean;
}

function deltaFmt(a: number, b: number, digits = 3): { text: string; cls: string } {
  const d = b - a;
  const text = `${d >= 0 ? '+' : ''}${d.toFixed(digits)}`;
  const cls = d > 0.005 ? 'positive' : d < -0.005 ? 'negative' : 'neutral';
  return { text, cls };
}

export default function VerdictStrip({ summary, statusText, statusLevel, passAt1Untrusted }: Props) {
  const s = summary.stock;
  const o = summary.ours;

  const pcDelta = deltaFmt(s.mean_partial_credit, o.mean_partial_credit);
  const wallDelta = deltaFmt(s.mean_wall_clock_s, o.mean_wall_clock_s, 2);
  const sP = summaryQualityPass(s);
  const oP = summaryQualityPass(o);
  const passDelta = deltaFmt(sP, oP, 3);
  const passLabel = summaryQualityLabel(s, passAt1Untrusted);

  return (
    <div className="verdict-strip">
      <span className={`vs-status ${statusLevel}`}>{statusText}</span>
      <span className="vs-metric">PC <span className="vs-s">{s.mean_partial_credit.toFixed(3)}</span> → <span className="vs-o">{o.mean_partial_credit.toFixed(3)}</span> <span className={`vs-delta ${pcDelta.cls}`}>{pcDelta.text}</span></span>
      <span className="vs-metric">Wall <span className="vs-s">{s.mean_wall_clock_s.toFixed(2)}s</span> → <span className="vs-o">{o.mean_wall_clock_s.toFixed(2)}s</span> <span className={`vs-delta ${wallDelta.cls}`}>{wallDelta.text}</span></span>
      <span className="vs-metric">{passLabel} <span className="vs-s">{(sP * 100).toFixed(1)}%</span> → <span className="vs-o">{(oP * 100).toFixed(1)}%</span> <span className={`vs-delta ${passDelta.cls}`}>{passDelta.text}</span></span>
    </div>
  );
}
