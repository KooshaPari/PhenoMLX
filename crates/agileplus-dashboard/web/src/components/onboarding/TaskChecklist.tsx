import React, { useRef, useEffect } from 'react';
import { useDemoMode, DEFAULT_DEMO_TASKS } from './DemoContext';
import { cn } from '../../lib/utils';

// ============================================================================
// TaskChecklist Component
// Collapsible floating panel with first-time-user tasks and progress bar.
// ============================================================================

export interface TaskChecklistProps {
  /** Called when the user clicks a task that has an actionView */
  onNavigate?: (view: string) => void;
  /** Whether to auto-expand this panel (used right after onboarding tour) */
  autoExpand?: boolean;
}

/**
 * TaskChecklist Component
 *
 * A floating bottom-right panel showing onboarding task progress.
 * Tasks are stored in localStorage via DemoContext.
 * Collapse/expand is local component state.
 *
 * @example
 * <TaskChecklist onNavigate={(v) => setView(v)} autoExpand={onboardingJustDone} />
 */
export const TaskChecklist: React.FC<TaskChecklistProps> = ({
  onNavigate,
  autoExpand = false,
}) => {
  const {
    completedTasks,
    completeTask,
    taskProgress,
    isChecklistDismissed,
    dismissChecklist,
  } = useDemoMode();

  const [isExpanded, setIsExpanded] = React.useState(false);
  const autoExpandHandled = useRef(false);

  // Auto-expand once when the onboarding tour completes (autoExpand flips true)
  useEffect(() => {
    if (autoExpand && !autoExpandHandled.current) {
      setIsExpanded(true);
      autoExpandHandled.current = true;
    }
  }, [autoExpand]);

  if (isChecklistDismissed) return null;

  const allDone = taskProgress === 100;

  return (
    <div
      role="region"
      aria-label="Getting started checklist"
      className={cn(
        'fixed bottom-4 right-4 z-30 max-w-xs w-72',
        'bg-white rounded-xl shadow-xl border border-gray-200',
        'transition-all duration-200',
        isExpanded ? 'opacity-100' : 'opacity-90 hover:opacity-100',
      )}
    >
      {/* ── Header — collapse / expand toggle ──────────────────────────────── */}
      <button
        type="button"
        onClick={() => setIsExpanded((p) => !p)}
        className={cn(
          'w-full flex items-center justify-between px-4 py-3',
          'text-sm font-semibold text-gray-800',
          'hover:bg-gray-50 rounded-t-xl transition-colors',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400',
        )}
        aria-expanded={isExpanded}
        aria-controls="checklist-body"
      >
        <span>Getting Started</span>
        <span className="text-xs text-gray-400" aria-hidden="true">
          {isExpanded ? '\u25BC' : '\u25B2'}
        </span>
      </button>

      {isExpanded && (
        <div id="checklist-body" className="px-4 pb-4 space-y-3">
          {/* ── Progress bar ──────────────────────────────────────────────── */}
          <div
            role="progressbar"
            aria-valuenow={taskProgress}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`${completedTasks.length} of ${DEFAULT_DEMO_TASKS.length} tasks complete`}
            className="w-full h-1.5 bg-gray-100 rounded-full overflow-hidden"
          >
            <div
              className={cn(
                'h-full rounded-full transition-all duration-500',
                allDone ? 'bg-green-500' : 'bg-cyan-500',
              )}
              style={{ width: `${taskProgress}%` }}
            />
          </div>
          <p className="text-xs text-gray-500">
            {completedTasks.length} of {DEFAULT_DEMO_TASKS.length} tasks complete
          </p>

          {/* ── Task list ──────────────────────────────────────────────────── */}
          <ul role="list" className="space-y-1">
            {DEFAULT_DEMO_TASKS.map((task) => {
              const isDone = completedTasks.includes(task.id);
              return (
                <li key={task.id} role="listitem">
                  <button
                    type="button"
                    onClick={() => {
                      if (!isDone) {
                        completeTask(task.id);
                      }
                      if (task.actionView && onNavigate) {
                        onNavigate(task.actionView);
                      }
                    }}
                    disabled={isDone && !task.actionView}
                    className={cn(
                      'w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-sm transition-colors',
                      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400',
                      isDone
                        ? 'text-gray-400 cursor-default'
                        : 'text-gray-700 hover:bg-gray-50 cursor-pointer',
                    )}
                    aria-label={
                      isDone
                        ? `${task.label} (completed)`
                        : task.actionView
                          ? `${task.label} — click to complete and navigate`
                          : task.label
                    }
                  >
                    {/* Check indicator */}
                    <span
                      className={cn(
                        'w-4 h-4 rounded-full border flex items-center justify-center flex-shrink-0',
                        isDone
                          ? 'bg-green-500 border-green-500 text-white'
                          : 'border-gray-300',
                      )}
                      aria-hidden="true"
                    >
                      {isDone && (
                        <svg
                          className="w-2.5 h-2.5"
                          fill="none"
                          viewBox="0 0 12 12"
                          stroke="currentColor"
                          strokeWidth={2}
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M2 6l3 3 5-5"
                          />
                        </svg>
                      )}
                    </span>

                    <span className="flex-1">{task.label}</span>

                    {task.actionView && !isDone && (
                      <span className="text-xs text-cyan-500 flex-shrink-0">
                        Go
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>

          {/* ── Dismiss (all tasks done) ────────────────────────────────────── */}
          {allDone && (
            <button
              type="button"
              onClick={dismissChecklist}
              className={cn(
                'w-full text-xs text-gray-400 hover:text-gray-600 py-1.5 rounded-md',
                'transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400',
              )}
            >
              Dismiss permanently
            </button>
          )}
        </div>
      )}
    </div>
  );
};

TaskChecklist.displayName = 'TaskChecklist';

export default TaskChecklist;
