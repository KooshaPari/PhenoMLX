import React, { useEffect, useState } from 'react';
import ReactDOM from 'react-dom/client';
import axios from 'axios';
import { useAgilePlusStore } from './stores/agileplus';
import './styles/globals.css';

// ── Types ──────────────────────────────────────────────────────────────────

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

// ── Styles ─────────────────────────────────────────────────────────────────

const NAV_STYLE: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '0.25rem',
  background: '#1e293b',
  padding: '0 1.5rem',
  height: '56px',
  boxShadow: '0 1px 3px rgba(0,0,0,0.3)',
};

const NAV_BRAND: React.CSSProperties = {
  color: '#f8fafc',
  fontWeight: 700,
  fontSize: '1.1rem',
  marginRight: '1.5rem',
  letterSpacing: '-0.01em',
};

const CARD: React.CSSProperties = {
  background: 'white',
  borderRadius: '8px',
  padding: '1.5rem',
  boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
  border: '1px solid rgba(17,24,39,0.08)',
  marginBottom: '1rem',
};

const BADGE = (status: string): React.CSSProperties => ({
  display: 'inline-block',
  padding: '0.15rem 0.55rem',
  borderRadius: '9999px',
  fontSize: '0.72rem',
  fontWeight: 600,
  background:
    status === 'Done' || status === 'done'
      ? '#dcfce7'
      : status === 'In Progress' || status === 'in_progress'
      ? '#dbeafe'
      : status === 'Blocked' || status === 'blocked'
      ? '#fee2e2'
      : '#f1f5f9',
  color:
    status === 'Done' || status === 'done'
      ? '#166534'
      : status === 'In Progress' || status === 'in_progress'
      ? '#1d4ed8'
      : status === 'Blocked' || status === 'blocked'
      ? '#991b1b'
      : '#475569',
});

// ── Nav button ─────────────────────────────────────────────────────────────

function NavBtn({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        background: active ? '#334155' : 'transparent',
        border: 'none',
        color: active ? '#f8fafc' : '#94a3b8',
        padding: '0.4rem 0.85rem',
        borderRadius: '6px',
        cursor: 'pointer',
        fontSize: '0.875rem',
        fontWeight: active ? 600 : 400,
        transition: 'all 0.15s',
      }}
    >
      {label}
    </button>
  );
}

// ── Views ──────────────────────────────────────────────────────────────────

function DashboardView({
  epics,
  stories,
  workPackageCount,
  loading,
}: {
  epics: Epic[];
  stories: Story[];
  workPackageCount: number;
  loading: boolean;
}) {
  if (loading) return <p style={{ color: '#64748b' }}>Loading dashboard…</p>;
  const doneStories = stories.filter((s) => s.status === 'Done' || s.status === 'done').length;
  const inProgress = stories.filter(
    (s) => s.status === 'In Progress' || s.status === 'in_progress',
  ).length;

  return (
    <div>
      <h2 style={{ marginTop: 0, marginBottom: '1.25rem', color: '#0f172a' }}>Dashboard</h2>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(180px,1fr))', gap: '1rem', marginBottom: '1.5rem' }}>
        {[
          { label: 'Epics', value: epics.length, color: '#6366f1' },
          { label: 'Stories', value: stories.length, color: '#0ea5e9' },
          { label: 'Done', value: doneStories, color: '#22c55e' },
          { label: 'In Progress', value: inProgress, color: '#f59e0b' },
          { label: 'Work Packages', value: workPackageCount, color: '#8b5cf6' },
        ].map((stat) => (
          <div
            key={stat.label}
            style={{
              ...CARD,
              marginBottom: 0,
              textAlign: 'center',
              borderTop: `3px solid ${stat.color}`,
            }}
          >
            <div style={{ fontSize: '2rem', fontWeight: 700, color: stat.color }}>{stat.value}</div>
            <div style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '0.25rem' }}>{stat.label}</div>
          </div>
        ))}
      </div>

      <div style={CARD}>
        <h3 style={{ marginTop: 0 }}>Recent Epics</h3>
        {epics.slice(0, 5).map((epic) => (
          <div
            key={epic.id}
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              padding: '0.5rem 0',
              borderBottom: '1px solid #f1f5f9',
            }}
          >
            <span style={{ fontWeight: 500 }}>{epic.title}</span>
            <span style={BADGE(epic.status)}>{epic.status}</span>
          </div>
        ))}
        {epics.length === 0 && <p style={{ color: '#94a3b8', margin: 0 }}>No epics yet.</p>}
      </div>
    </div>
  );
}

function EpicsView({ epics, stories, loading }: { epics: Epic[]; stories: Story[]; loading: boolean }) {
  if (loading) return <p style={{ color: '#64748b' }}>Loading epics…</p>;
  return (
    <div>
      <h2 style={{ marginTop: 0, marginBottom: '1.25rem', color: '#0f172a' }}>
        Epics <span style={{ fontSize: '0.9rem', color: '#64748b', fontWeight: 400 }}>({epics.length})</span>
      </h2>
      {epics.length === 0 && (
        <div style={CARD}>
          <p style={{ margin: 0, color: '#94a3b8' }}>No epics found. Make sure the backend API is running on :4000.</p>
        </div>
      )}
      {epics.map((epic) => {
        const epicStories = stories.filter((s) => s.epic_id === epic.id);
        return (
          <details key={epic.id} style={{ ...CARD, cursor: 'pointer' }}>
            <summary style={{ fontWeight: 600, display: 'flex', justifyContent: 'space-between', alignItems: 'center', listStyle: 'none' }}>
              <span>{epic.title}</span>
              <span style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                <span style={{ fontSize: '0.8rem', color: '#64748b' }}>{epicStories.length} stories</span>
                <span style={BADGE(epic.status)}>{epic.status}</span>
              </span>
            </summary>
            <ul style={{ marginTop: '0.75rem', paddingLeft: '1.25rem' }}>
              {epicStories.map((story) => (
                <li key={story.id} style={{ marginBottom: '0.35rem', fontSize: '0.875rem', display: 'flex', justifyContent: 'space-between', alignItems: 'center', maxWidth: '600px' }}>
                  <span style={{ color: '#334155' }}>{story.title}</span>
                  <span style={BADGE(story.status)}>{story.status}</span>
                </li>
              ))}
              {epicStories.length === 0 && <li style={{ color: '#94a3b8' }}>No stories in this epic.</li>}
            </ul>
          </details>
        );
      })}
    </div>
  );
}

function StoriesView({ epics, stories, loading }: { epics: Epic[]; stories: Story[]; loading: boolean }) {
  const [filterEpic, setFilterEpic] = useState<number | null>(null);
  const [filterStatus, setFilterStatus] = useState<string>('');

  if (loading) return <p style={{ color: '#64748b' }}>Loading stories…</p>;

  const filtered = stories.filter((s) => {
    if (filterEpic !== null && s.epic_id !== filterEpic) return false;
    if (filterStatus && s.status !== filterStatus) return false;
    return true;
  });

  const statuses = Array.from(new Set(stories.map((s) => s.status)));

  return (
    <div>
      <h2 style={{ marginTop: 0, marginBottom: '1.25rem', color: '#0f172a' }}>
        Stories <span style={{ fontSize: '0.9rem', color: '#64748b', fontWeight: 400 }}>({filtered.length}/{stories.length})</span>
      </h2>
      <div style={{ display: 'flex', gap: '0.75rem', marginBottom: '1.25rem', flexWrap: 'wrap' }}>
        <select
          value={filterEpic ?? ''}
          onChange={(e) => setFilterEpic(e.target.value ? Number(e.target.value) : null)}
          style={{ padding: '0.35rem 0.65rem', borderRadius: '6px', border: '1px solid #e2e8f0', fontSize: '0.875rem' }}
        >
          <option value="">All Epics</option>
          {epics.map((ep) => (
            <option key={ep.id} value={ep.id}>{ep.title}</option>
          ))}
        </select>
        <select
          value={filterStatus}
          onChange={(e) => setFilterStatus(e.target.value)}
          style={{ padding: '0.35rem 0.65rem', borderRadius: '6px', border: '1px solid #e2e8f0', fontSize: '0.875rem' }}
        >
          <option value="">All Statuses</option>
          {statuses.map((st) => (
            <option key={st} value={st}>{st}</option>
          ))}
        </select>
      </div>
      <div style={CARD}>
        {filtered.length === 0 && <p style={{ margin: 0, color: '#94a3b8' }}>No stories match the filter.</p>}
        {filtered.map((story) => {
          const epic = epics.find((e) => e.id === story.epic_id);
          return (
            <div
              key={story.id}
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '0.5rem 0',
                borderBottom: '1px solid #f1f5f9',
              }}
            >
              <div>
                <span style={{ fontWeight: 500, color: '#1e293b' }}>{story.title}</span>
                {epic && (
                  <span style={{ marginLeft: '0.5rem', fontSize: '0.75rem', color: '#94a3b8' }}>
                    ({epic.title})
                  </span>
                )}
              </div>
              <span style={BADGE(story.status)}>{story.status}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function EvidenceView({ epics, stories }: { epics: Epic[]; stories: Story[] }) {
  const tracedEpics = epics.filter((e) => e.requirement_id);
  const tracedStories = stories.filter((s) => s.requirement_id);
  const coverage = stories.length > 0 ? Math.round((tracedStories.length / stories.length) * 100) : 0;

  return (
    <div>
      <h2 style={{ marginTop: 0, marginBottom: '1.25rem', color: '#0f172a' }}>Evidence &amp; Traceability</h2>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px,1fr))', gap: '1rem', marginBottom: '1.5rem' }}>
        {[
          { label: 'Traced Epics', value: `${tracedEpics.length}/${epics.length}`, color: '#6366f1' },
          { label: 'Traced Stories', value: `${tracedStories.length}/${stories.length}`, color: '#0ea5e9' },
          { label: 'Trace Coverage', value: `${coverage}%`, color: coverage >= 80 ? '#22c55e' : coverage >= 50 ? '#f59e0b' : '#ef4444' },
        ].map((stat) => (
          <div key={stat.label} style={{ ...CARD, marginBottom: 0, textAlign: 'center', borderTop: `3px solid ${stat.color}` }}>
            <div style={{ fontSize: '1.75rem', fontWeight: 700, color: stat.color }}>{stat.value}</div>
            <div style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '0.25rem' }}>{stat.label}</div>
          </div>
        ))}
      </div>

      <div style={CARD}>
        <h3 style={{ marginTop: 0 }}>Requirement Trace Links</h3>
        {tracedEpics.length === 0 && tracedStories.length === 0 ? (
          <p style={{ margin: 0, color: '#94a3b8' }}>No requirement_id fields populated yet.</p>
        ) : (
          <>
            {tracedEpics.map((epic) => (
              <div key={epic.id} style={{ padding: '0.4rem 0', borderBottom: '1px solid #f1f5f9', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ fontWeight: 500 }}>{epic.title}</span>
                <code style={{ fontSize: '0.75rem', color: '#6366f1', background: '#eef2ff', padding: '0.1rem 0.4rem', borderRadius: '4px' }}>{epic.requirement_id}</code>
              </div>
            ))}
            {tracedStories.map((story) => (
              <div key={story.id} style={{ padding: '0.4rem 0', borderBottom: '1px solid #f1f5f9', display: 'flex', justifyContent: 'space-between' }}>
                <span>{story.title}</span>
                <code style={{ fontSize: '0.75rem', color: '#0ea5e9', background: '#f0f9ff', padding: '0.1rem 0.4rem', borderRadius: '4px' }}>{story.requirement_id}</code>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

// ── App ────────────────────────────────────────────────────────────────────

type View = 'dashboard' | 'epics' | 'stories' | 'evidence';

function App() {
  const workPackages = useAgilePlusStore((state) => state.workPackages);
  const loading = useAgilePlusStore((state) => state.loading);
  const setWorkPackages = useAgilePlusStore((state) => state.setWorkPackages);
  const setLoading = useAgilePlusStore((state) => state.setLoading);

  const [epics, setEpics] = useState<Epic[]>([]);
  const [stories, setStories] = useState<Story[]>([]);
  const [epicStoriesLoading, setEpicStoriesLoading] = useState(true);
  const [view, setView] = useState<View>('dashboard');
  const [apiError, setApiError] = useState<string | null>(null);

  // Fetch work packages
  useEffect(() => {
    setLoading(true);
    axios
      .get('/api/dashboard/work-packages.json')
      .then((res) => {
        const data = res.data as { work_packages: any[] };
        setWorkPackages(
          (data.work_packages ?? []).map((wp: any) => ({
            id: String(wp.id),
            title: wp.title ?? '(untitled)',
            status: wp.status ?? 'planned',
            priority: wp.priority ?? 'medium',
            assignee: wp.assignee ?? undefined,
          })),
        );
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [setWorkPackages, setLoading]);

  // Fetch epics + stories
  useEffect(() => {
    setEpicStoriesLoading(true);
    setApiError(null);
    axios
      .get('/api/dashboard/epics-stories.json')
      .then((res) => {
        const data = res.data as { epics: Epic[]; stories: Story[]; error?: string };
        if (data.error) setApiError(data.error);
        setEpics(data.epics ?? []);
        setStories(data.stories ?? []);
      })
      .catch((err) => {
        setApiError(`API unavailable: ${err.message}. Start backend with API_PORT=4000 DATABASE_PATH=agileplus.db`);
      })
      .finally(() => setEpicStoriesLoading(false));
  }, []);

  const views: { id: View; label: string }[] = [
    { id: 'dashboard', label: 'Dashboard' },
    { id: 'epics', label: `Epics (${epics.length})` },
    { id: 'stories', label: `Stories (${stories.length})` },
    { id: 'evidence', label: 'Evidence' },
  ];

  return (
    <>
      {/* Top navigation */}
      <nav style={NAV_STYLE}>
        <span style={NAV_BRAND}>AgilePlus</span>
        {views.map((v) => (
          <NavBtn key={v.id} label={v.label} active={view === v.id} onClick={() => setView(v.id)} />
        ))}
        <span style={{ marginLeft: 'auto', fontSize: '0.75rem', color: '#475569' }}>
          {epicStoriesLoading ? '⏳ loading…' : apiError ? '⚠ API error' : `✓ ${epics.length} epics · ${stories.length} stories`}
        </span>
      </nav>

      {/* Main content */}
      <main style={{ maxWidth: '960px', margin: '0 auto', padding: '2rem 1rem' }}>
        {apiError && (
          <div
            style={{
              background: '#fef2f2',
              border: '1px solid #fecaca',
              borderRadius: '6px',
              padding: '0.75rem 1rem',
              marginBottom: '1.25rem',
              fontSize: '0.85rem',
              color: '#991b1b',
            }}
          >
            {apiError}
          </div>
        )}

        {view === 'dashboard' && (
          <DashboardView
            epics={epics}
            stories={stories}
            workPackageCount={workPackages.length}
            loading={epicStoriesLoading}
          />
        )}
        {view === 'epics' && (
          <EpicsView epics={epics} stories={stories} loading={epicStoriesLoading} />
        )}
        {view === 'stories' && (
          <StoriesView epics={epics} stories={stories} loading={epicStoriesLoading} />
        )}
        {view === 'evidence' && <EvidenceView epics={epics} stories={stories} />}
      </main>
    </>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
