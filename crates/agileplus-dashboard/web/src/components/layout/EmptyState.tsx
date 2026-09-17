import React from 'react';
import { cn } from '../../lib/utils';
import type { EmptyStateProps } from '../../types';

// ============================================================================
// EmptyState Component
// Placeholder view with illustration, title, description, and optional action
// ============================================================================

/**
 * EmptyState Component
 * Displays a centered placeholder when no data is available
 *
 * @example
 * <EmptyState
 *   title="No work packages"
 *   description="Get started by creating your first work package."
 *   action={<Button onClick={handleCreate}>Create Work Package</Button>}
 * />
 */
export const EmptyState: React.FC<EmptyStateProps> = ({
  illustration,
  title,
  description,
  action,
  className,
}) => {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center px-6 py-16 text-center',
        className
      )}
      role="status"
    >
      {illustration && (
        <div className="mb-6 text-gray-400">{illustration}</div>
      )}
      <h3 className="text-lg font-semibold text-gray-900">{title}</h3>
      {description && (
        <p className="mt-2 max-w-md text-sm text-gray-500">{description}</p>
      )}
      {action && <div className="mt-6">{action}</div>}
    </div>
  );
};

EmptyState.displayName = 'EmptyState';

export default EmptyState;
