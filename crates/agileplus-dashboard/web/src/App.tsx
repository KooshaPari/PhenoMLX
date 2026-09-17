import React, { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { useAgilePlusStore } from './stores/agileplus';
import {
  Button, Badge, Card, Pill, Modal, Toast, EmptyState, Skeleton,
  OnboardingTour, SettingsView, DemoMode, TaskChecklist,
} from './components';
import { DemoProvider, useDemoMode } from './components/onboarding/DemoContext';
import type { OnboardingTourStep } from './types';
import './styles/globals.css';
import { ThemeProvider, useTheme } from './theme';
import { LocaleProvider } from './i18n';

// ─── Types ────────────────────────────────────────────────────────────────────

interface Epic {
  id: number;
  title: string;
  status: string;
  requirement_id: string | null;
}

interface Story {
  id: number;
  epic_id: number | null;
  title: string;
  status: string;
  requirement_id: string | null;
}

type View = 'dashboard' | 'epics' | 'stories' | 'evidence' | 'settings';

// ─── Status helpers ────────────────────────────────────────────────────────────

function epicBadgeVariant(status: string): 'success' | 'warning' | 'error' | 'info' | 'default' {
  switch (status.toLowerCase()) {
    case 'done':
    case 'completed': return 'success';
    case 'in_progress':
    case 'in progress': return 'info';
    case 'blocked': return 'error';
    case 'planned': return 'warning';
    default: return 'default';
  }
}

// ─── Seed / demo data (used when API is unreachable) ─────────────────────────

const SEED_EPICS: Epic[] = [
  { id: 1, title: 'Core domain entities', status: 'Done', requirement_id: 'FR-AGP-001' },
  { id: 2, title: 'SQLite persistence layer', status: 'Done', requirement_id: 'FR-AGP-002' },
  { id: 3, title: 'Axum REST API', status: 'In Progress', requirement_id: 'FR-AGP-003' },
  { id: 4, title: 'Native dashboard frontend', status: 'In Progress', requirement_id: 'FR-AGP-014' },
  { id: 5, title: 'GitHub sync adapter', status: 'Planned', requirement_id: 'FR-AGP-008' },
  { id: 6, title: 'Plane.so integration', status: 'Planned', requirement_id: 'FR-AGP-018' },
];

const SEED_STORIES: Story[] = [
  { id: 1, epic_id: 1, title: 'Define Epic/Story/WorkPackage value objects', status: 'Done', requirement_id: null },
  { id: 2, epic_id: 1, title: 'Add requirement traceability links', status: 'Done', requirement_id: null },
  { id: 3, epic_id: 2, title: 'SQLite migrations for all domain tables', status: 'Done', requirement_id: null },
  { id: 4, epic_id: 2, title: 'Repository impls (epics, stories, work_packages)', status: 'Done', requirement_id: null },
  { id: 5, epic_id: 3, title: 'GET /api/epics + /api/stories endpoints', status: 'Done', requirement_id: null },
  { id: 6, epic_id: 3, title: 'POST /api/epics + /api/stories endpoints', status: 'In Progress', requirement_id: null },
  { id: 7, epic_id: 3, title: 'Auth middleware (JWT)', status: 'In Progress', requirement_id: null },
  { id: 8, epic_id: 4, title: 'Component library (Button/Card/Badge/etc)', status: 'Done', requirement_id: null },
  { id: 9, epic_id: 4, title: 'Wire components into App views', status: 'In Progress', requirement_id: null },
  { id: 10, epic_id: 4, title: 'Evidence Gallery view (PHASE2)', status: 'Planned', requirement_id: null },
  { id: 11, epic_id: 5, title: 'GitHub webhook listener', status: 'Planned', requirement_id: null },
  { id: 12, epic_id: 6, title: 'Plane API client + sync job', status: 'Planned', requirement_id: null },
];

// ─── Onboarding tour steps ─────────────────────────────────────────────────

const TOUR_STEPS: OnboardingTourStep[] = [
  {
    id: 'welcome',
    title: 'Welcome to AgilePlus',
    description:
      "Let's take a quick tour of your project management dashboard. We'll highlight the key areas so you can hit the ground running.",
    placement: 'center',
  },
  {
    id: 'navigation',
    title: 'Navigation Bar',
    description:
      'Navigate between Dashboard, Epics, Stories, and Evidence Gallery views using these tabs. Each view provides a different perspective on your project data.',
    targetSelector: 'nav',
    placement: 'bottom',
  },
  {
    id: 'content-area',
    title: 'Content Area',
    description:
      'The main panel adapts to your current view. The Dashboard shows overview stats, Epics lists all epics with progress bars, Stories provides filtering, and Evidence Gallery displays test artifacts.',
    targetSelector: 'main',
    placement: 'top',
  },
  {
    id: 'theme-toggle',
    title: 'Theme Switcher',
    description:
      'Toggle between light, dark, and system themes using the icon button in the top-right corner of the navigation bar.',
    targetSelector: '[aria-label^="Switch theme"]',
    placement: 'left',
  },
  {
    id: 'complete',
    title: "You're All Set!",
    description:
      'You now know the key areas of AgilePlus. Use the navigation bar to explore and manage your project work items. You can restart this tour anytime from the settings.',
    placement: 'center',
  },
];

// ─── Nav ──────────────────────────────────────────────────────────────────────

interface NavProps {
  activeView: View;
  onNav: (v: View) => void;
}

const THEME_LABELS: Record<string, string> = {
  light: '☀️',
  dark: '🌙',
  system: '🖥',
};

const THEME_CYCLE: Array<'light' | 'dark' | 'system'> = ['light', 'dark', 'system'];

const NAV_ITEMS: { label: string; view: View }[] = [
  { label: 'Dashboard', view: 'dashboard' },
  { label: 'Epics', view: 'epics' },
  { label: 'Stories', view: 'stories' },
  { label: 'Evidence Gallery', view: 'evidence' },
  { label: 'Settings', view: 'settings' },
];

function Nav({ activeView, onNav }: NavProps) {
  const { theme, setTheme } = useTheme();
  const { isDemoMode, setDemoMode } = useDemoMode();

  const cycleTheme = () => {
    const idx = THEME_CYCLE.indexOf(theme);
    const next = THEME_CYCLE[(idx + 1) % THEME_CYCLE.length];
    setTheme(next);
  };

  return (
    <nav className="bg-gray-900 text-white px-6 py-3 flex items-center gap-6">
      <span className="font-bold text-cyan-400 mr-4 text-lg">AgilePlus</span>
      {NAV_ITEMS.map(({ label, view }) => (
        <button
          key={view}
          onClick={() => onNav(view)}
          className={`text-sm font-medium transition-colors px-3 py-1.5 rounded hover:bg-gray-700 ${
            activeView === view ? 'bg-gray-700 text-cyan-400' : 'text-gray-300'
          }`}
        >
          {label}
        </button>
      ))}

      <div className="ml-auto flex items-center gap-2">
        <DemoMode />
        <button
          onClick={cycleTheme}
          className="text-sm px-2.5 py-1.5 rounded transition-colors hover:bg-gray-700 text-gray-300"
          aria-label={`Switch theme (currently ${theme})`}
          title={`Theme: ${theme}`}
        >
          {THEME_LABELS[theme]}
        </button>
      </div>
    </nav>
  );
}

// ─── Dashboard view ────────────────────────────────────────────────────────────

function DashboardView({ epics, stories, loading }: { epics: Epic[]; stories: Story[]; loading?: boolean }) {
  const done = epics.filter((e) => e.status.toLowerCase() === 'done').length;
  const inProgress = epics.filter((e) => ['in_progress', 'in progress'].includes(e.status.toLowerCase())).length;
  const planned = epics.filter((e) => e.status.toLowerCase() === 'planned').length;
  const storiesDone = stories.filter((s) => s.status.toLowerCase() === 'done').length;

  const stats = [
    { label: 'Total Epics', value: epics.length, variant: 'default' as const },
    { label: 'Epics Done', value: done, variant: 'success' as const },
    { label: 'In Progress', value: inProgress, variant: 'info' as const },
    { label: 'Planned', value: planned, variant: 'warning' as const },
    { label: 'Stories', value: stories.length, variant: 'default' as const },
    { label: 'Stories Done', value: storiesDone, variant: 'success' as const },
  ];

  return (
    <div>
      <h2 className="text-xl font-bold mb-4">Overview</h2>
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4 mb-8">
        {loading
          ? Array.from({ length: 6 }).map((_, i) => (
              <Card key={`skel-${i}`} variant="elevated" className="text-center">
                <Skeleton variant="rectangular" height={80} className="mb-2" />
                <Skeleton variant="text" width="60%" className="mx-auto" />
              </Card>
            ))
          : stats.map(({ label, value, variant }) => (
              <Card key={label} variant="elevated" className="text-center">
                <div className="text-3xl font-bold mb-1">{value}</div>
                <Badge label={label} variant={variant} />
              </Card>
            ))}
      </div>

      <h2 className="text-xl font-bold mb-3">Recent Epics</h2>
      <div className="space-y-2">
        {epics.slice(0, 4).map((epic) => {
          const epicStories = stories.filter((s) => s.epic_id === epic.id);
          return (
            <Card key={epic.id}>
              <div className="flex items-center justify-between">
                <div>
                  <span className="font-semibold">{epic.title}</span>
                  {epic.requirement_id && (
                    <Pill label={epic.requirement_id} variant="primary" className="ml-2" />
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-sm text-gray-500">{epicStories.length} stories</span>
                  <Badge label={epic.status} variant={epicBadgeVariant(epic.status)} />
                </div>
              </div>
            </Card>
          );
        })}
      </div>
    </div>
  );
}

// ─── Epics view ────────────────────────────────────────────────────────────────

function EpicsView({ epics, stories }: { epics: Epic[]; stories: Story[] }) {
  const [selectedEpic, setSelectedEpic] = useState<Epic | null>(null);

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold">Epics</h2>
        <Badge label={`${epics.length} total`} variant="default" />
      </div>

      <div className="space-y-3">
        {epics.map((epic) => {
          const epicStories = stories.filter((s) => s.epic_id === epic.id);
          const doneCount = epicStories.filter((s) => s.status.toLowerCase() === 'done').length;
          const progress = epicStories.length > 0 ? Math.round((doneCount / epicStories.length) * 100) : 0;

          return (
            <Card key={epic.id} variant="elevated">
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-semibold">{epic.title}</span>
                    {epic.requirement_id && (
                      <Pill label={epic.requirement_id} variant="secondary" />
                    )}
                  </div>
                  <div className="mt-2 flex items-center gap-3">
                    <div className="flex-1 bg-gray-200 rounded-full h-1.5 max-w-xs">
                      <div
                        className="bg-cyan-500 h-1.5 rounded-full transition-all"
                        style={{ width: `${progress}%` }}
                      />
                    </div>
                    <span className="text-xs text-gray-500 whitespace-nowrap">
                      {doneCount}/{epicStories.length} stories
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <Badge label={epic.status} variant={epicBadgeVariant(epic.status)} />
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setSelectedEpic(epic)}
                  >
                    Details
                  </Button>
                </div>
              </div>
            </Card>
          );
        })}
      </div>

      <Modal
        isOpen={selectedEpic !== null}
        onClose={() => setSelectedEpic(null)}
        title={selectedEpic?.title ?? ''}
        size="md"
      >
        {selectedEpic && (
          <div>
            <div className="flex items-center gap-2 mb-3">
              <Badge label={selectedEpic.status} variant={epicBadgeVariant(selectedEpic.status)} />
              {selectedEpic.requirement_id && (
                <Pill label={selectedEpic.requirement_id} variant="primary" />
              )}
            </div>
            <h4 className="font-semibold mb-2">Stories</h4>
            <ul className="space-y-1">
              {stories
                .filter((s) => s.epic_id === selectedEpic.id)
                .map((s) => (
                  <li key={s.id} className="flex items-center justify-between text-sm py-1 border-b border-gray-100 last:border-0">
                    <span>{s.title}</span>
                    <Badge label={s.status} variant={epicBadgeVariant(s.status)} />
                  </li>
                ))}
            </ul>
          </div>
        )}
      </Modal>
    </div>
  );
}

// ─── Stories view ──────────────────────────────────────────────────────────────

function StoriesView({ epics, stories }: { epics: Epic[]; stories: Story[] }) {
  const [filterEpic, setFilterEpic] = useState<number | 'all'>('all');
  const [filterStatus, setFilterStatus] = useState<string>('all');

  const filtered = stories.filter((s) => {
    const epicMatch = filterEpic === 'all' || s.epic_id === filterEpic;
    const statusMatch = filterStatus === 'all' || s.status.toLowerCase() === filterStatus;
    return epicMatch && statusMatch;
  });

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold">Stories</h2>
        <Badge label={`${filtered.length} shown`} variant="default" />
      </div>

      <div className="flex flex-wrap gap-3 mb-4">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm text-gray-500">Epic:</span>
          <button
            onClick={() => setFilterEpic('all')}
            className={`text-xs px-2 py-1 rounded-full border transition-colors ${filterEpic === 'all' ? 'bg-cyan-500 text-white border-cyan-500' : 'border-gray-300 hover:border-cyan-400'}`}
          >
            All
          </button>
          {epics.map((e) => (
            <button
              key={e.id}
              onClick={() => setFilterEpic(e.id)}
              className={`text-xs px-2 py-1 rounded-full border transition-colors ${filterEpic === e.id ? 'bg-cyan-500 text-white border-cyan-500' : 'border-gray-300 hover:border-cyan-400'}`}
            >
              {e.title.split(' ').slice(0, 3).join(' ')}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-gray-500">Status:</span>
          {['all', 'done', 'in progress', 'planned', 'blocked'].map((s) => (
            <button
              key={s}
              onClick={() => setFilterStatus(s)}
              className={`text-xs px-2 py-1 rounded-full border transition-colors ${filterStatus === s ? 'bg-purple-500 text-white border-purple-500' : 'border-gray-300 hover:border-purple-400'}`}
            >
              {s.charAt(0).toUpperCase() + s.slice(1)}
            </button>
          ))}
        </div>
      </div>

      <div className="space-y-2">
        {filtered.map((story) => {
          const epic = epics.find((e) => e.id === story.epic_id);
          return (
            <Card key={story.id}>
              <div className="flex items-center justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="font-medium truncate">{story.title}</div>
                  {epic && (
                    <div className="text-xs text-gray-500 mt-0.5">
                      Epic: {epic.title}
                    </div>
                  )}
                </div>
                <Badge label={story.status} variant={epicBadgeVariant(story.status)} />
              </div>
            </Card>
          );
        })}
        {filtered.length === 0 && (
          <EmptyState
            title="No stories match your filters"
            description="Try adjusting your filter criteria to see more results."
          />
        )}
      </div>
    </div>
  );
}

// ─── Evidence Gallery view (PHASE2 stub) ──────────────────────────────────────

function EvidenceGalleryView({ loading }: { loading?: boolean }) {
  if (loading) {
    return (
      <div>
        <div className="flex items-center justify-between mb-4">
          <Skeleton variant="rectangular" width={200} height={28} />
          <Skeleton variant="rectangular" width={80} height={24} />
        </div>
        <Card variant="outlined" className="py-12">
          <div className="flex flex-col items-center gap-3">
            <Skeleton variant="circular" width={64} height={64} />
            <Skeleton variant="text" width="60%" />
            <Skeleton variant="text" count={2} width="80%" />
          </div>
        </Card>
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold">Evidence Gallery</h2>
        <Badge label="PHASE2" variant="warning" />
      </div>
      <Card variant="outlined" className="text-center py-12">
        <div className="text-4xl mb-3">🗂</div>
        <h3 className="font-semibold text-gray-700 mb-2">Evidence Gallery — Coming in PHASE2</h3>
        <p className="text-sm text-gray-500 max-w-md mx-auto">
          This slot (originally :5176 MFE) will display test run screenshots, logs, and
          video artifacts linked to work items. The component types are already defined in{' '}
          <code className="bg-gray-100 px-1 rounded">src/types/index.ts</code>
          {' '}(<code>EvidenceItem</code>, <code>EvidenceGalleryProps</code>).
        </p>
        <div className="mt-4">
          <Badge label="FR-AGP-014" variant="info" />
        </div>
      </Card>
    </div>
  );
}

// ─── Root App ─────────────────────────────────────────────────────────────────

export function App() {
  // Read persisted locale from localStorage
  let storedLocale = 'en';
  try {
    const stored = localStorage.getItem('agileplus_locale');
    if (stored === 'en' || stored === 'de') {
      storedLocale = stored;
    }
  } catch {
    // localStorage unavailable
  }

  return (
    <ThemeProvider>
      <LocaleProvider defaultLocale={storedLocale}>
        <DemoProvider>
          <AppContent />
        </DemoProvider>
      </LocaleProvider>
    </ThemeProvider>
  );
}

function AppContent() {
  const setWorkPackages = useAgilePlusStore((state) => state.setWorkPackages);
  const setLoading = useAgilePlusStore((state) => state.setLoading);

  const [view, setView] = useState<View>('dashboard');
  const [epics, setEpics] = useState<Epic[]>([]);
  const [stories, setStories] = useState<Story[]>([]);
  const [dataReady, setDataReady] = useState(false);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);

  // Check localStorage for onboarding completion on mount
  useEffect(() => {
    try {
      const completed = localStorage.getItem('onboarding_complete');
      if (completed !== 'true') {
        setShowOnboarding(true);
      }
    } catch {
      // localStorage unavailable (private browsing, quota exceeded) — show tour anyway
      setShowOnboarding(true);
    }
  }, []);

  // Try live API, fall back to seed data
  useEffect(() => {
    setLoading(true);
    axios
      .get('/api/dashboard/epics-stories.json', { timeout: 3000 })
      .then((res) => {
        const data = res.data as { epics: Epic[]; stories: Story[] };
        if (data.epics?.length) {
          setEpics(data.epics);
          setStories(data.stories ?? []);
        } else {
          setEpics(SEED_EPICS);
          setStories(SEED_STORIES);
        }
      })
      .catch(() => {
        setEpics(SEED_EPICS);
        setStories(SEED_STORIES);
        setToastMsg('API offline — showing seed data');
      })
      .finally(() => {
        setLoading(false);
        setDataReady(true);
      });

    // Also try the work-packages endpoint for the Zustand store
    axios
      .get('/api/dashboard/work-packages.json', { timeout: 3000 })
      .then((res) => {
        const data = res.data as { work_packages: any[] };
        setWorkPackages(
          (data.work_packages ?? []).map((wp: any) => ({
            id: String(wp.id),
            title: wp.title ?? '(untitled)',
            status: wp.status ?? 'planned',
            priority: wp.priority ?? 'medium',
            assignee: wp.assignee ?? undefined,
          }))
        );
      })
      .catch(() => {/* no-op */});
  }, [setWorkPackages, setLoading]);

  return (
    <div className="min-h-screen bg-gray-50">
      <Nav activeView={view} onNav={setView} />
      <main className="container mx-auto px-4 py-6 max-w-5xl">
        {!dataReady ? (
          <div className="py-12">
            <Skeleton variant="rectangular" height={24} className="mb-6 max-w-xs" />
            <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4 mb-8">
              {Array.from({ length: 6 }).map((_, i) => (
                <Skeleton key={`load-${i}`} variant="rectangular" height={96} />
              ))}
            </div>
            <Skeleton variant="text" count={4} />
          </div>
        ) : (
          <>
            {view === 'dashboard' && <DashboardView epics={epics} stories={stories} loading={false} />}
            {view === 'epics' && <EpicsView epics={epics} stories={stories} />}
            {view === 'stories' && <StoriesView epics={epics} stories={stories} />}
            {view === 'evidence' && <EvidenceGalleryView />}
            {view === 'settings' && <SettingsView />}
          </>
        )}
      </main>

      {toastMsg && (
        <div className="fixed top-4 right-4 z-50 w-80">
          <Toast
            type="warning"
            message={toastMsg}
            duration={4000}
            onClose={() => setToastMsg(null)}
          />
        </div>
      )}

      <OnboardingTour
        isOpen={showOnboarding}
        onClose={() => setShowOnboarding(false)}
        steps={TOUR_STEPS}
      />
    </div>
  );
}

export default App;
