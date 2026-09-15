/* ── View & filter enums ───────────────────────────────────────────── */

export type ViewType =
  | 'overview'
  | 'suites'
  | 'cells'
  | 'comparison'
  | 'langfuse'
  | 'settings';

export type SortDir = 1 | -1;

export type GroupBy = 'none' | 'suite' | 'difficulty' | 'variant' | 'status';

/* ── Variant summary metrics ──────────────────────────────────────── */

export interface VariantSummary {
  pass_at_1: number;
  gen_ok: number;
  verified_pass_at_1: number;
  mean_wall_clock_s: number;
  mean_partial_credit: number;
  mean_format_compliance: number;
  n_hallucinations: number;
  mean_tokens_read: number;
  mean_cost_usd: number;
  mean_peak_rss_mb: number;
  mean_energy_joules: number;
  mean_first_token_ms: number;
  mean_retry_count: number;
  success_rate: number;
  timeout_rate: number;
  [k: string]: number;
}

/* ── Summary data ─────────────────────────────────────────────────── */

export interface SummaryData {
  meta: { model: string; n_cells: number; n_suites: number; variants?: string[] };
  by_variant: Record<string, VariantSummary> & {
    stock?: VariantSummary;
    ours?: VariantSummary;
  };
}

export interface SuiteCoverageRow {
  suite: string;
  present: boolean;
  variants: Record<string, number>;
  n_cells: number;
  has_stock: boolean;
  has_ours: boolean;
  experiment_arms: string[];
}

export type Summary = SummaryData;

/* ── Individual benchmark cell ────────────────────────────────────── */

export interface Cell {
  task_id: string;
  variant: string;
  suite: string;
  difficulty: string;
  task_type: string;
  ok: boolean;
  wall_clock_s: number;
  tokens_per_second: number;
  first_token_latency_ms: number;
  pass_at_1: number;
  gen_ok?: number;
  verified_pass_at_1?: number;
  partial_credit: number;
  format_compliance_rate: number;
  judge_score: number;
  hallucination_count: number;
  retry_count: number;
  total_tokens_in: number;
  total_tokens_out: number;
  cost_usd: number;
  peak_rss_mb: number;
  peak_gpu_mem_mb: number;
  energy_proxy_joules: number;
  created_at: string;
  completed_at?: string;
  semantic?: Record<string, number>;
  failure_analysis?: {
    primary_factor?: string;
    confidence?: number;
    [k: string]: any;
  };
  reply?: string;
  prompt?: string;
  error_message?: string;
  model_name?: string;
  metadata?: Record<string, string>;
  [k: string]: any;
}

/* ── Payload envelope ─────────────────────────────────────────────── */

export interface LintWarning {
  code: string;
  severity: 'error' | 'warning' | 'info';
  message: string;
  cells?: string[];
}

export interface BenchPayload {
  serverTs: string;
  lintRunTs?: string;
  warnings?: LintWarning[];
  jsonPath?: string;
  suite_coverage?: SuiteCoverageRow[];
  data: { summary: SummaryData; cells: Cell[] };
}

export interface HistoryEntry {
  receivedAt: string;
  summary: SummaryData;
  cellCount: number;
}

export interface Insight {
  kind: string;
  level: 'good' | 'warn' | 'bad';
  text: string;
  jumpTo?: string;
}

export interface CellFilters {
  suite: Set<string>;
  difficulty: Set<string>;
  variant: Set<string>;
}
