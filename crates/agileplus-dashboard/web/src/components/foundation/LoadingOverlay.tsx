import React from 'react';
import { cn } from '../../lib/utils';
import { Skeleton } from './Skeleton';

// ============================================================================
// LoadingOverlay Component
// Full-screen overlay with backdrop blur for loading states
// ============================================================================

interface LoadingOverlayProps {
  /** When true, shows the loading overlay over children */
  isLoading: boolean;
  /** Content to render beneath the overlay when loading, or normally when not */
  children: React.ReactNode;
  /** Optional className for the overlay container */
  className?: string;
}

/**
 * LoadingOverlay Component
 * Renders a full-screen blurred overlay with skeleton placeholders when loading.
 * When not loading, renders children directly without overlay.
 *
 * @example
 * <LoadingOverlay isLoading={isFetching}>
 *   <DashboardContent />
 * </LoadingOverlay>
 */
export const LoadingOverlay: React.FC<LoadingOverlayProps> = ({
  isLoading,
  children,
  className,
}) => {
  if (!isLoading) {
    return <>{children}</>;
  }

  return (
    <div
      className={cn('relative', className)}
      role="alert"
      aria-busy={isLoading}
    >
      {/* Content underneath (dimmed) */}
      <div className="pointer-events-none select-none opacity-30 blur-sm transition-all duration-300">
        {children}
      </div>

      {/* Overlay with skeleton placeholders */}
      <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-4 backdrop-blur-[2px]">
        <div className="flex flex-col items-center gap-3 w-full max-w-md px-6">
          <Skeleton variant="rectangular" width="100%" height={16} />
          <Skeleton variant="text" count={3} />
          <div className="grid grid-cols-3 gap-4 w-full mt-2">
            <Skeleton variant="rectangular" height={80} />
            <Skeleton variant="rectangular" height={80} />
            <Skeleton variant="rectangular" height={80} />
          </div>
          <Skeleton variant="text" count={2} className="mt-2" />
        </div>
        <span className="text-sm text-gray-400 mt-2">Loading…</span>
      </div>
    </div>
  );
};

export default LoadingOverlay;
