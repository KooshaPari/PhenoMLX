import React from 'react';
import { cn } from '../../lib/utils';
import type { SkeletonProps } from '../../types';

// ============================================================================
// Skeleton Component
// Loading placeholder that mimics content layout with animated pulse
// ============================================================================

const variantClasses: Record<NonNullable<SkeletonProps['variant']>, string> = {
  text: 'h-4 w-full rounded',
  circular: 'rounded-full',
  rectangular: 'rounded-md',
};

/**
 * Skeleton Component
 * Renders animated loading placeholders to indicate content is being fetched.
 * Supports multiple variants, custom dimensions, and count-based rendering.
 *
 * @example
 * <Skeleton variant="text" count={3} />
 * <Skeleton variant="circular" width={48} height={48} />
 * <Skeleton variant="rectangular" width="100%" height={200} animate={false} />
 */
export const Skeleton: React.FC<SkeletonProps> = ({
  width,
  height,
  variant = 'text',
  className,
  count = 1,
  animate = true,
}) => {
  const baseClasses = cn(
    'bg-gray-200',
    variantClasses[variant],
    animate && 'animate-pulse',
    className
  );

  const items = Array.from({ length: count }, (_, i) => i);

  return (
    <span
      className="inline-flex flex-col gap-2"
      aria-busy="true"
      aria-label="Loading"
      role="status"
    >
      {items.map((i) => (
        <span
          key={i}
          className={baseClasses}
          style={{
            width: width ?? (variant === 'circular' && !width ? height ?? 48 : undefined),
            height: height ?? (variant === 'text' ? undefined : variant === 'circular' && !height ? width ?? 48 : undefined),
          }}
        />
      ))}
    </span>
  );
};

export default Skeleton;
