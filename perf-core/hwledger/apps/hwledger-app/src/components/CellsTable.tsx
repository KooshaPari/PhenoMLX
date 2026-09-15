import React from 'react';
import { Cell, GroupBy } from '../types';
import { qualityPass } from '../lib/metrics';

interface Props {
  cells: Cell[];
  sortKey: string;
  sortDir: 1 | -1;
  groupBy: GroupBy;
  onSelect: (cell: Cell | null) => void;
  onSort: (key: string) => void;
  onGroup: (g: GroupBy) => void;
}

const COLS: Array<{ key: string; label: string; w: number; fmt: (v: any, c?: Cell) => string }> = [
  { key: 'task_id', label: 'Task', w: 130, fmt: (v) => v },
  { key: 'variant', label: 'Var', w: 52, fmt: (v) => v },
  { key: 'suite', label: 'Suite', w: 100, fmt: (v) => v },
  { key: 'difficulty', label: 'Diff', w: 60, fmt: (v) => v },
  { key: 'ok', label: 'Status', w: 72, fmt: (v) => (v ? 'ok' : 'fail') },
  { key: 'wall_clock_s', label: 'Wall', w: 70, fmt: (v) => v.toFixed(2) + 's' },
  {
    key: 'pass_at_1',
    label: 'P@1',
    w: 56,
    fmt: (_v, c) => {
      if (!c) return '—';
      return (qualityPass(c) * 100).toFixed(0) + '%';
    },
  },
  { key: 'partial_credit', label: 'PC', w: 56, fmt: (v) => v.toFixed(3) },
  { key: 'format_compliance_rate', label: 'Fmt', w: 56, fmt: (v) => (v * 100).toFixed(0) + '%' },
  { key: 'judge_score', label: 'Judge', w: 60, fmt: (v) => (v ? v.toFixed(2) : '—') },
  { key: 'tokens_per_second', label: 'Tok/s', w: 60, fmt: (v) => (v ? v.toFixed(1) : '—') },
  { key: 'hallucination_count', label: 'Hal', w: 44, fmt: (v) => v },
  { key: 'cost_usd', label: '$', w: 64, fmt: (v) => '$' + v.toFixed(4) },
];

const ROW_WIDTH = COLS.reduce((s, c) => s + c.w, 0);

function cellSortValue(c: Cell, key: string): number | string | boolean {
  if (key === 'pass_at_1') return qualityPass(c);
  return c[key];
}

export default function CellsTable({ cells, sortKey, sortDir, groupBy, onSelect, onSort, onGroup }: Props) {
  const sorted = [...cells].sort((a, b) => {
    const av = cellSortValue(a, sortKey);
    const bv = cellSortValue(b, sortKey);
    if (sortKey === 'ok') return sortDir * (Number(bv) - Number(av));
    if (typeof av === 'string' || typeof bv === 'string') return sortDir * String(av).localeCompare(String(bv));
    return sortDir * ((Number(av) || 0) - (Number(bv) || 0));
  });

  const rows: Array<{ kind: 'group' | 'cell'; label?: string; cell?: Cell }> = [];
  if (groupBy === 'none') {
    for (const c of sorted) rows.push({ kind: 'cell', cell: c });
  } else {
    const groups = new Map<string, Cell[]>();
    for (const c of sorted) {
      let gk: string;
      if (groupBy === 'status') gk = c.ok ? 'ok' : c.wall_clock_s >= 59 && !c.tokens_per_second ? 'timeout' : 'fail';
      else if (groupBy === 'variant') gk = c.variant;
      else gk = c[groupBy] || 'other';
      if (!groups.has(gk)) groups.set(gk, []);
      groups.get(gk)!.push(c);
    }
    for (const [gk, arr] of groups) {
      rows.push({ kind: 'group', label: `${gk} (${arr.length})` });
      for (const c of arr) rows.push({ kind: 'cell', cell: c });
    }
  }

  const col = COLS.find((c) => c.key === sortKey);
  const colLabel = col?.label || sortKey;

  return (
    <div className="view-content">
      <div className="cells-toolbar">
        <span className="cells-count">{cells.length} cells</span>
        <span className="cells-sort">sorted by {colLabel} {sortDir < 0 ? '▼' : '▲'}</span>
        <div className="cells-group">
          {(['none', 'suite', 'difficulty', 'variant', 'status'] as const).map((g) => (
            <button key={g} type="button" className={`gt-btn ${groupBy === g ? 'on' : ''}`} onClick={() => onGroup(g)}>{g}</button>
          ))}
        </div>
      </div>
      <div className="cells-wrap">
        <div className="cells-header" style={{ width: '100%' }}>
          <div className="cells-header-inner" style={{ width: ROW_WIDTH }}>
            {COLS.map((c) => (
              <div key={c.key} className="ch" style={{ width: c.w, minWidth: c.w }}
                role="button" tabIndex={0}
                onClick={() => onSort(c.key)}
                onKeyDown={(e) => { if (e.key === 'Enter') onSort(c.key); }}>
                {c.label}
                {sortKey === c.key && <span className="csort">{sortDir < 0 ? '▼' : '▲'}</span>}
              </div>
            ))}
          </div>
        </div>
        <div className="cells-scroll">
          <div style={{ width: ROW_WIDTH }}>
            {rows.map((row, idx) => {
              if (row.kind === 'group') {
                return (
                  <div key={idx} className="cg-row" style={{ height: 28, width: ROW_WIDTH }}>
                    <span className="cg-label">{row.label}</span>
                  </div>
                );
              }
              const c = row.cell!;
              return (
                <div
                  key={idx}
                  className="cd-row"
                  style={{ height: 28, width: ROW_WIDTH }}
                  role="button" tabIndex={0}
                  onClick={() => onSelect(c)}
                  onKeyDown={(e) => { if (e.key === 'Enter') onSelect(c); }}
                >
                  {COLS.map((col) => (
                    <div key={col.key} className="cd" style={{ width: col.w, minWidth: col.w }}>
                      {col.key === 'ok' ? (
                        <span className={`sp ${c.ok ? 'ok' : 'fail'}`}>{c.ok ? 'ok' : 'fail'}</span>
                      ) : col.key === 'variant' ? (
                        <span className={`vp ${c.variant}`}>{c.variant}</span>
                      ) : col.key === 'difficulty' ? (
                        <span className={`dp ${c.difficulty}`}>{c.difficulty}</span>
                      ) : col.key === 'wall_clock_s' ? (
                        <>
                          <span
                            className={`mini-bar ${c.wall_clock_s > 30 ? 'warn' : 'ours'}`}
                            title={`${c.wall_clock_s.toFixed(2)}s`}
                          >
                            <div style={{ width: `${Math.min((c.wall_clock_s / 60) * 100, 100)}%` }} />
                          </span>
                          {col.fmt(c[col.key], c)}
                        </>
                      ) : (
                        col.fmt(c[col.key], c)
                      )}
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
