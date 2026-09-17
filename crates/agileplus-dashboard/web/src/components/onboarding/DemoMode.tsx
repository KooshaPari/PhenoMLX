import React from 'react';
import { useDemoMode } from './DemoContext';
import { cn } from '../../lib/utils';

// ============================================================================
// DemoMode Component
// Compact toggle / badge for demo (sample-data) mode worn in the nav bar
// ============================================================================

export interface DemoModeProps {
  /** Additional class names applied to the root element */
  className?: string;
}

/**
 * DemoMode Component
 *
 * When **inactive** renders a small "Demo" toggle button.
 * When **active** renders a pill badge "Demo Mode ●" with an exit button.
 *
 * State is persisted in localStorage via DemoContext.
 *
 * @example
 * <DemoMode />
 */
export const DemoMode: React.FC<DemoModeProps> = ({ className }) => {
  const { isDemoMode, setDemoMode } = useDemoMode();

  // ── Inactive state — toggle button ────────────────────────────────────────
  if (!isDemoMode) {
    return (
      <button
        type="button"
        onClick={() => setDemoMode(true)}
        className={cn(
          'text-xs font-medium px-2 py-1 rounded transition-colors',
          'bg-amber-500/10 text-amber-400 hover:bg-amber-500/20',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400',
          className,
        )}
        aria-label="Enable demo mode"
        title="Show sample epics and stories"
      >
        Demo
      </button>
    );
  }

  // ── Active state — pill badge ─────────────────────────────────────────────
  return (
    <div
      role="status"
      className={cn(
        'inline-flex items-center gap-1.5 px-2 py-1 rounded-full',
        'bg-amber-400/15 text-amber-300 text-xs font-medium',
        'border border-amber-400/25',
        className,
      )}
    >
      <span
        className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"
        aria-hidden="true"
      />
      Demo Mode
      <button
        type="button"
        onClick={() => setDemoMode(false)}
        className={cn(
          'ml-0.5 text-amber-300/60 hover:text-amber-200 transition-colors',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 rounded-sm',
        )}
        aria-label="Exit demo mode"
      >
        ✕
      </button>
    </div>
  );
};

DemoMode.displayName = 'DemoMode';

export default DemoMode;
