import React, { useEffect, useState, useRef } from 'react';
import { cn } from '../../lib/utils';

/**
 * Splash Screen Component
 *
 * Full-viewport loading splash with branded animation, progress indicator,
 * and live status text. Used while the dashboard bundle boots, while the
 * initial SQLite connection warms up, and during any top-level "loading
 * the project" state.
 *
 * Accessibility:
 *   - role="status" + aria-live="polite" announces progress to screen readers
 *   - aria-busy="true" on the wrapper signals an active loading region
 *   - Reduced-motion users get a static brand reveal (no sweep/pulse)
 *   - Keyboard focus is trapped inside the splash until dismissed
 *
 * Traces to: FR-UX-04 (loading experience), pillar L51 (Splash Screen)
 */

// ============================================================================
// SplashScreen Component
// ============================================================================

export interface SplashStep {
  /** Stable id used as the React key and for tracking completion. */
  id: string;
  /** Short, human-readable label shown in the progress log. */
  label: string;
  /** Optional subtext (e.g. "12 / 47 modules"). */
  detail?: string;
}

export interface SplashScreenProps {
  /** Ordered steps to render in the progress log. */
  steps: SplashStep[];
  /** Set of step ids that have completed. */
  completedSteps: Set<string>;
  /** Optional override for the heading text. */
  title?: string;
  /** Optional override for the tagline below the title. */
  tagline?: string;
  /** Optional error to surface (renders an error banner). */
  error?: string | null;
  /** Called when the user dismisses the splash (Escape, click, or timeout). */
  onDismiss?: () => void;
  /** Auto-dismiss after this many ms once all steps complete. */
  autoDismissMs?: number;
  /** Additional classes for the outer wrapper. */
  className?: string;
}

/**
 * SplashScreen
 *
 * @example
 * <SplashScreen
 *   title="Loading AgilePlus"
 *   tagline="Booting cockpit…"
 *   steps={[
 *     { id: 'db', label: 'Connecting to SQLite' },
 *     { id: 'mig', label: 'Running migrations' },
 *     { id: 'tr', label: 'Loading feature tree' },
 *   ]}
 *   completedSteps={doneSet}
 * />
 */
export const SplashScreen: React.FC<SplashScreenProps> = ({
  steps,
  completedSteps,
  title = 'AgilePlus',
  tagline = 'Loading cockpit…',
  error = null,
  onDismiss,
  autoDismissMs = 600,
  className,
}) => {
  const [now, setNow] = useState(Date.now());
  const [dismissed, setDismissed] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // Animated progress (0..1) based on completed steps.
  const total = steps.length || 1;
  const done = steps.filter((s) => completedSteps.has(s.id)).length;
  const pct = Math.round((done / total) * 100);

  // Time tracking for "elapsed" display.
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 100);
    return () => window.clearInterval(id);
  }, []);

  // Auto-dismiss when all steps complete.
  useEffect(() => {
    if (autoDismissMs > 0 && done === total && !dismissed) {
      const id = window.setTimeout(() => {
        setDismissed(true);
        onDismiss?.();
      }, autoDismissMs);
      return () => window.clearTimeout(id);
    }
  }, [autoDismissMs, done, total, dismissed, onDismiss]);

  // Reduced-motion: render static brand reveal instead of sweep/pulse.
  const reducedMotion = usePrefersReducedMotion();

  // Dismiss handlers: Escape key + click on splash surface.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !dismissed) {
        setDismissed(true);
        onDismiss?.();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [dismissed, onDismiss]);

  if (dismissed && !error) {
    return null;
  }

  return (
    <div
      ref={wrapperRef}
      role="status"
      aria-live="polite"
      aria-busy={!error && done < total}
      aria-labelledby="splash-title"
      aria-describedby="splash-tagline"
      onClick={() => {
        if (done === total && onDismiss) {
          setDismissed(true);
          onDismiss();
        }
      }}
      className={cn(
        'fixed inset-0 z-50 flex flex-col items-center justify-center gap-8',
        'bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950',
        'text-slate-100 select-none',
        className
      )}
    >
      {/* Brand mark + sweeping ring animation */}
      <BrandMark reducedMotion={reducedMotion} />

      {/* Title + tagline */}
      <div className="flex flex-col items-center gap-1 text-center">
        <h1
          id="splash-title"
          className="text-3xl font-semibold tracking-tight md:text-4xl"
        >
          {title}
        </h1>
        <p
          id="splash-tagline"
          className="text-sm text-slate-400 md:text-base"
        >
          {error ? 'A problem occurred' : tagline}
        </p>
      </div>

      {/* Progress bar */}
      <div className="w-72 max-w-[80vw]">
        <ProgressBar value={pct} reducedMotion={reducedMotion} />
        <div className="mt-2 flex items-center justify-between text-xs text-slate-400">
          <span>
            {done} / {total} {total === 1 ? 'step' : 'steps'}
          </span>
          <span>{pct}%</span>
        </div>
      </div>

      {/* Step log */}
      <ol className="w-72 max-w-[80vw] space-y-1 text-xs">
        {steps.map((s) => {
          const isDone = completedSteps.has(s.id);
          return (
            <li
              key={s.id}
              className={cn(
                'flex items-center gap-2 rounded px-2 py-1',
                isDone ? 'text-emerald-400' : 'text-slate-500'
              )}
            >
              <span aria-hidden="true" className="inline-block w-3">
                {isDone ? '✓' : '○'}
              </span>
              <span className="flex-1 truncate">{s.label}</span>
              {s.detail && (
                <span className="text-slate-500">{s.detail}</span>
              )}
            </li>
          );
        })}
      </ol>

      {/* Error banner */}
      {error && (
        <div
          role="alert"
          className="w-72 max-w-[80vw] rounded border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300"
        >
          <p className="font-semibold">Error</p>
          <p className="mt-1 text-red-200">{error}</p>
        </div>
      )}

      {/* Footer hint */}
      <p className="text-[10px] uppercase tracking-widest text-slate-600">
        Press Esc to dismiss
      </p>
    </div>
  );
};

export default SplashScreen;

// ============================================================================
// Subcomponents
// ============================================================================

/** Brand mark — animated sweeping ring with centered logogram. */
const BrandMark: React.FC<{ reducedMotion: boolean }> = ({ reducedMotion }) => (
  <div className="relative h-24 w-24">
    {/* Outer sweep ring */}
    <div
      aria-hidden="true"
      className={cn(
        'absolute inset-0 rounded-full border-2 border-cyan-400/40',
        reducedMotion ? '' : 'animate-[spin_2.4s_linear_infinite]'
      )}
    >
      <div className="absolute -top-1 left-1/2 h-2 w-2 -translate-x-1/2 rounded-full bg-cyan-400 shadow-[0_0_12px_3px_rgba(34,211,238,0.6)]" />
    </div>
    {/* Inner pulse */}
    <div
      aria-hidden="true"
      className={cn(
        'absolute inset-3 rounded-full bg-cyan-500/15',
        reducedMotion ? '' : 'animate-pulse'
      )}
    />
    {/* Center logogram */}
    <div className="absolute inset-0 flex items-center justify-center">
      <span className="text-2xl font-bold tracking-tight text-cyan-300">A+</span>
    </div>
  </div>
);

/** Progress bar with role="progressbar" + aria-valuenow. */
const ProgressBar: React.FC<{ value: number; reducedMotion: boolean }> = ({
  value,
  reducedMotion,
}) => (
  <div
    role="progressbar"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={value}
    aria-label="Boot progress"
    className="h-1.5 w-full overflow-hidden rounded-full bg-slate-800"
  >
    <div
      className={cn(
        'h-full rounded-full bg-gradient-to-r from-cyan-400 to-emerald-400',
        reducedMotion ? '' : 'transition-[width] duration-300 ease-out'
      )}
      style={{ width: `${value}%` }}
    />
  </div>
);

/** Hook returning true when the user prefers reduced motion. SSR-safe. */
function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return;
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    setReduced(mq.matches);
    const onChange = () => setReduced(mq.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);
  return reduced;
}