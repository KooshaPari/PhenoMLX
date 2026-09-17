/**
 * Onboarding Components
 * Guided tour, overlay, demo mode, and checklist for first-run experiences
 */

export { OnboardingOverlay, default as OnboardingOverlayDefault } from './OnboardingOverlay';
export type { OnboardingOverlayProps } from './OnboardingOverlay';
export { OnboardingTour, default as OnboardingTourDefault } from './OnboardingTour';
export { DemoProvider, useDemoMode, DEFAULT_DEMO_TASKS } from './DemoContext';
export { DemoMode, default as DemoModeDefault } from './DemoMode';
export type { DemoModeProps } from './DemoMode';
export { TaskChecklist, default as TaskChecklistDefault } from './TaskChecklist';
export type { TaskChecklistProps } from './TaskChecklist';

export type { OnboardingTourStep, OnboardingProps, DemoTask, DemoContextType } from '../../types';
