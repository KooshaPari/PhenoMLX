import React, { useEffect, useRef, useState, useCallback } from 'react';
import { cn } from '../../lib/utils';
import type { OnboardingProps } from '../../types';
import { OnboardingOverlay } from './OnboardingOverlay';

// ============================================================================
// OnboardingTour Component
// Step-by-step walkthrough overlay with spotlight effect and keyboard nav
// ============================================================================

const STORAGE_KEY_DEFAULT = 'onboarding_complete';
const SPOTLIGHT_PADDING = 8;
const TOOLTIP_GAP = 14;
const VIEWPORT_MARGIN = 16;

/**
 * Returns the bounding client rect for a CSS selector, or null if no match.
 * Uses `querySelector` internally.
 */
function getTargetRect(selector?: string): DOMRect | null {
  if (!selector) return null;
  const el = document.querySelector(selector);
  if (!el) return null;
  return el.getBoundingClientRect();
}

/**
 * Calculates the optimal tooltip position relative to a target rect.
 * Clamps the result to stay within the viewport boundaries.
 */
function calcTooltipPosition(
  rect: DOMRect | null,
  tooltipWidth: number,
  tooltipHeight: number,
  placement: string,
): { top: number; left: number } {
  const gap = TOOLTIP_GAP;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const margin = VIEWPORT_MARGIN;

  let top: number;
  let left: number;

  if (!rect) {
    top = (vh - tooltipHeight) / 2;
    left = (vw - tooltipWidth) / 2;
  } else {
    switch (placement) {
      case 'top': {
        top = rect.top - tooltipHeight - gap;
        left = rect.left + rect.width / 2 - tooltipWidth / 2;
        break;
      }
      case 'bottom': {
        top = rect.bottom + gap;
        left = rect.left + rect.width / 2 - tooltipWidth / 2;
        break;
      }
      case 'left': {
        top = rect.top + rect.height / 2 - tooltipHeight / 2;
        left = rect.left - tooltipWidth - gap;
        break;
      }
      case 'right': {
        top = rect.top + rect.height / 2 - tooltipHeight / 2;
        left = rect.right + gap;
        break;
      }
      default: {
        top = (vh - tooltipHeight) / 2;
        left = (vw - tooltipWidth) / 2;
      }
    }
  }

  // Clamp within viewport
  top = Math.max(margin, Math.min(top, vh - tooltipHeight - margin));
  left = Math.max(margin, Math.min(left, vw - tooltipWidth - margin));

  return { top, left };
}

/**
 * OnboardingTour Component
 * Renders a guided step-by-step overlay tour with a spotlight cutout effect
 * around the targeted element. Supports keyboard navigation (Escape to skip,
 * arrow keys to navigate), reduced-motion preferences, and localStorage-based
 * completion tracking.
 *
 * @example
 * const steps = [
 *   { id: 'welcome', title: 'Welcome', description: '...', placement: 'center' },
 *   { id: 'nav', title: 'Navigation', description: '...', targetSelector: 'nav' },
 * ];
 * <OnboardingTour isOpen={showTour} onClose={() => setShowTour(false)} steps={steps} />
 */
export const OnboardingTour: React.FC<OnboardingProps> = ({
  isOpen,
  onClose,
  steps,
  onComplete,
  storageKey = STORAGE_KEY_DEFAULT,
}) => {
  const [currentStep, setCurrentStep] = useState(0);
  const [targetRect, setTargetRect] = useState<DOMRect | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ top: number; left: number }>({
    top: 0,
    left: 0,
  });
  const [tooltipVisible, setTooltipVisible] = useState(false);

  const tooltipRef = useRef<HTMLDivElement>(null);
  const prevPosKeyRef = useRef<string>('');
  const prefersReducedMotion = useRef(false);

  // ── Reduced motion detection ──────────────────────────────────────────────

  useEffect(() => {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    prefersReducedMotion.current = mq.matches;
    const handler = (e: MediaQueryListEvent) => {
      prefersReducedMotion.current = e.matches;
    };
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, []);

  // ── Reset step index when tour opens ──────────────────────────────────────

  useEffect(() => {
    if (isOpen) {
      setCurrentStep(0);
      setTooltipVisible(false);
      prevPosKeyRef.current = '';
    }
  }, [isOpen]);

  const step = steps[currentStep] as (typeof steps)[number] | undefined;

  // ── Recalculate target rect ───────────────────────────────────────────────

  const recalcTarget = useCallback(() => {
    if (!isOpen || !step) {
      setTargetRect(null);
      return;
    }
    setTargetRect(getTargetRect(step.targetSelector));
  }, [isOpen, step]);

  useEffect(() => {
    recalcTarget();
    window.addEventListener('resize', recalcTarget);
    window.addEventListener('scroll', recalcTarget, { passive: true });
    return () => {
      window.removeEventListener('resize', recalcTarget);
      window.removeEventListener('scroll', recalcTarget);
    };
  }, [recalcTarget]);

  // ── Measure tooltip and set position after render ─────────────────────────

  useEffect(() => {
    if (!isOpen || !tooltipRef.current) return;

    const el = tooltipRef.current;
    const tw = el.offsetWidth;
    const th = el.offsetHeight;
    const placement = step?.placement ?? 'center';
    const pos = calcTooltipPosition(targetRect, tw, th, placement);
    const posKey = `${pos.top},${pos.left}`;

    // Only update state if position actually changed to avoid render loops
    if (posKey !== prevPosKeyRef.current) {
      prevPosKeyRef.current = posKey;
      setTooltipPos(pos);
    }

    setTooltipVisible(true);
  }, [isOpen, currentStep, targetRect, step]);

  // ── Reset tooltip visibility on step change ───────────────────────────────

  useEffect(() => {
    setTooltipVisible(false);
    prevPosKeyRef.current = '';
  }, [currentStep]);

  // ── Keyboard navigation ──────────────────────────────────────────────────

  const handleSkip = useCallback(() => {
    try {
      localStorage.setItem(storageKey, 'true');
    } catch {
      // localStorage may be unavailable (private browsing, quota exceeded)
    }
    onClose();
  }, [storageKey, onClose]);

  const handleNext = useCallback(() => {
    if (currentStep < steps.length - 1) {
      setCurrentStep((p) => p + 1);
    } else {
      try {
        localStorage.setItem(storageKey, 'true');
      } catch {
        // opaque
      }
      onComplete?.();
      onClose();
    }
  }, [currentStep, steps.length, storageKey, onComplete, onClose]);

  const handleBack = useCallback(() => {
    if (currentStep > 0) {
      setCurrentStep((p) => p - 1);
    }
  }, [currentStep]);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if user is typing in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      switch (e.key) {
        case 'Escape':
          e.preventDefault();
          handleSkip();
          break;
        case 'ArrowLeft':
        case 'ArrowUp':
          e.preventDefault();
          handleBack();
          break;
        case 'ArrowRight':
        case 'ArrowDown':
          e.preventDefault();
          handleNext();
          break;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, handleSkip, handleBack, handleNext]);

  // ── Focus management ─────────────────────────────────────────────────────

  useEffect(() => {
    if (isOpen && tooltipRef.current) {
      tooltipRef.current.focus();
    }
  }, [isOpen, currentStep]);

  if (!isOpen || !step) return null;

  const isFirst = currentStep === 0;
  const isLast = currentStep === steps.length - 1;
  const progress = ((currentStep + 1) / steps.length) * 100;
  const reducedMotion = prefersReducedMotion.current;

  const spotlightStyle = targetRect
    ? {
        left: targetRect.left - SPOTLIGHT_PADDING,
        top: targetRect.top - SPOTLIGHT_PADDING,
        width: targetRect.width + SPOTLIGHT_PADDING * 2,
        height: targetRect.height + SPOTLIGHT_PADDING * 2,
      }
    : null;

  return (
    <>
      {/* Semi-transparent backdrop when no target (centered steps) */}
      {!targetRect && <OnboardingOverlay isOpen={isOpen} />}

      {/* Spotlight cutout — creates the "hole-punch" effect via box-shadow */}
      {targetRect && (
        <div
          className={cn(
            'fixed z-50 pointer-events-none',
            !reducedMotion && 'transition-all duration-300 ease-in-out',
          )}
          style={{
            left: spotlightStyle!.left,
            top: spotlightStyle!.top,
            width: spotlightStyle!.width,
            height: spotlightStyle!.height,
            boxShadow: '0 0 0 9999px rgba(0, 0, 0, 0.5)',
            borderRadius: '10px',
          }}
          aria-hidden="true"
        />
      )}

      {/* ── Tooltip card ──────────────────────────────────────────────────── */}
      <div
        ref={tooltipRef}
        role="dialog"
        aria-modal="true"
        aria-label={`Tour step ${currentStep + 1}: ${step.title}`}
        aria-describedby={`tour-desc-${step.id}`}
        tabIndex={-1}
        className={cn(
          'fixed z-[60] w-80 bg-white rounded-xl shadow-2xl border border-gray-200 overflow-hidden outline-none',
          'focus-visible:ring-2 focus-visible:ring-cyan-400 focus-visible:ring-offset-2',
          tooltipVisible ? 'opacity-100' : 'opacity-0',
          !reducedMotion && 'transition-all duration-300 ease-out',
        )}
        style={{
          top: tooltipPos.top,
          left: tooltipPos.left,
          transform: tooltipVisible ? 'translateY(0)' : 'translateY(8px)',
        }}
      >
        {/* Progress bar */}
        <div
          className="h-1 bg-gray-100"
          role="progressbar"
          aria-valuenow={progress}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={`Tour progress: step ${currentStep + 1} of ${steps.length}`}
        >
          <div
            className={cn(
              'h-full bg-cyan-500',
              !reducedMotion && 'transition-all duration-300',
            )}
            style={{ width: `${progress}%` }}
          />
        </div>

        {/* Body */}
        <div className="p-5">
          <span className="text-xs font-medium text-gray-400">
            Step {currentStep + 1} of {steps.length}
          </span>
          <h3 className="text-base font-semibold text-gray-900 mt-1 mb-1.5">
            {step.title}
          </h3>
          <p
            id={`tour-desc-${step.id}`}
            className="text-sm text-gray-600 leading-relaxed"
          >
            {step.description}
          </p>
        </div>

        {/* Footer actions */}
        <div className="flex items-center justify-between px-5 py-3 bg-gray-50 border-t border-gray-100">
          <button
            onClick={handleSkip}
            className={cn(
              'text-xs font-medium text-gray-400 hover:text-gray-600 rounded px-2 py-1',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400',
            )}
            aria-label="Skip all tour steps"
          >
            Skip all
          </button>

          <div className="flex items-center gap-2">
            {!isFirst && (
              <button
                onClick={handleBack}
                className={cn(
                  'text-sm font-medium text-gray-600 hover:text-gray-800 px-3 py-1.5 rounded-md',
                  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400',
                )}
                aria-label="Previous step"
              >
                Back
              </button>
            )}
            <button
              onClick={handleNext}
              className={cn(
                'text-sm font-medium px-4 py-1.5 rounded-md',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400',
                isLast
                  ? 'bg-green-600 text-white hover:bg-green-700'
                  : 'bg-cyan-500 text-white hover:bg-cyan-600',
              )}
              aria-label={isLast ? 'Finish tour' : 'Next step'}
            >
              {isLast ? "Done \u2713" : 'Next'}
            </button>
          </div>
        </div>
      </div>
    </>
  );
};

OnboardingTour.displayName = 'OnboardingTour';

export default OnboardingTour;
