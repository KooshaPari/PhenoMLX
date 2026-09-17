import React from 'react';
import { cn } from '../../lib/utils';

// ============================================================================
// OnboardingOverlay Component
// Semi-transparent backdrop used behind modals, tours, and focus overlays
// ============================================================================

export interface OnboardingOverlayProps {
  /** Whether the overlay is visible */
  isOpen: boolean;
  /** Optional click handler for dismissing the overlay */
  onClose?: () => void;
  /** Additional class names */
  className?: string;
}

/**
 * OnboardingOverlay Component
 * A full-screen semi-transparent backdrop that dims the page behind a foreground
 * dialog or spotlight. Supports optional click-to-dismiss.
 *
 * @example
 * <OnboardingOverlay isOpen={show} onClose={() => setShow(false)} />
 */
export const OnboardingOverlay: React.FC<OnboardingOverlayProps> = ({
  isOpen,
  onClose,
  className,
}) => {
  if (!isOpen) return null;

  return (
    <div
      className={cn(
        'fixed inset-0 z-40 bg-black/50',
        'data-[reduced-motion=false]:transition-opacity',
        'data-[reduced-motion=false]:duration-300',
        className,
      )}
      onClick={onClose}
      role="presentation"
      aria-hidden="true"
    />
  );
};

OnboardingOverlay.displayName = 'OnboardingOverlay';

export default OnboardingOverlay;
