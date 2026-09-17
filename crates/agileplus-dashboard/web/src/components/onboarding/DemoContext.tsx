import React, { createContext, useContext, useState, useCallback, useMemo } from 'react';
import type { DemoContextType, DemoTask } from '../../types';

// ============================================================================
// DemoContext — shared state for demo mode and onboarding task checklist
// ============================================================================

export const DEFAULT_DEMO_TASKS: DemoTask[] = [
  { id: 'create-epic', label: 'Create your first Epic', actionView: 'epics' },
  { id: 'add-story', label: 'Add a Story', actionView: 'stories' },
  { id: 'view-evidence', label: 'View Evidence Gallery', actionView: 'evidence' },
  { id: 'dark-mode', label: 'Switch to dark mode' },
  { id: 'complete-tour', label: 'Complete the tour' },
];

const STORAGE_KEYS = {
  demoMode: 'agileplus_demo_mode',
  tasks: 'agileplus_completed_tasks',
  dismissed: 'agileplus_checklist_dismissed',
} as const;

/** Safe localStorage getter — returns null when storage is unavailable. */
function safeRead(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Safe localStorage setter — noop when storage is unavailable. */
function safeWrite(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // localStorage unavailable (private browsing, quota exceeded)
  }
}

const DemoContext = createContext<DemoContextType | null>(null);

export function DemoProvider({ children }: { children: React.ReactNode }) {
  const [isDemoMode, setDemoModeState] = useState<boolean>(
    () => safeRead(STORAGE_KEYS.demoMode) === 'true',
  );
  const [completedTasks, setCompletedTasks] = useState<string[]>(() => {
    const stored = safeRead(STORAGE_KEYS.tasks);
    if (!stored) return [];
    try {
      const parsed = JSON.parse(stored);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  });
  const [isChecklistDismissed, setDismissed] = useState<boolean>(
    () => safeRead(STORAGE_KEYS.dismissed) === 'true',
  );

  const setDemoMode = useCallback((mode: boolean) => {
    setDemoModeState(mode);
    safeWrite(STORAGE_KEYS.demoMode, String(mode));
  }, []);

  const completeTask = useCallback((taskId: string) => {
    setCompletedTasks((prev) => {
      if (prev.includes(taskId)) return prev;
      const next = [...prev, taskId];
      safeWrite(STORAGE_KEYS.tasks, JSON.stringify(next));
      return next;
    });
  }, []);

  const dismissChecklist = useCallback(() => {
    setDismissed(true);
    safeWrite(STORAGE_KEYS.dismissed, 'true');
  }, []);

  const taskProgress = useMemo(() => {
    const total = DEFAULT_DEMO_TASKS.length;
    if (total === 0) return 100;
    return Math.round((completedTasks.length / total) * 100);
  }, [completedTasks.length]);

  const value = useMemo<DemoContextType>(
    () => ({
      isDemoMode,
      setDemoMode,
      completedTasks,
      completeTask,
      taskProgress,
      isChecklistDismissed,
      dismissChecklist,
    }),
    [
      isDemoMode,
      setDemoMode,
      completedTasks,
      completeTask,
      taskProgress,
      isChecklistDismissed,
      dismissChecklist,
    ],
  );

  return <DemoContext.Provider value={value}>{children}</DemoContext.Provider>;
}

export function useDemoMode(): DemoContextType {
  const ctx = useContext(DemoContext);
  if (!ctx) {
    throw new Error('useDemoMode must be used within a <DemoProvider>');
  }
  return ctx;
}

DemoProvider.displayName = 'DemoProvider';
