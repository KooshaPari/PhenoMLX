import { useState, useEffect, useCallback } from 'react';
import { HashRouter, Routes, Route, NavLink, useNavigate } from 'react-router-dom';
import Overview from './components/Overview';
import Suites from './components/Suites';
import CellsTable from './components/CellsTable';
import Comparison from './components/Comparison';
import LangfuseEmbed from './components/LangfuseEmbed';
import { LangfusePanel } from './components/LangfusePanel';
import type { Cell, GroupBy, SummaryData } from './types';
import './App.css';

const MLX_DEFAULT = 'http://localhost:8080';

function Sidebar({ mlxOk, onNavigate }: { mlxOk: boolean; onNavigate: (path: string) => void }) {
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <span className="brand-icon">◈</span>
        <span className="brand-text">hwLedger</span>
      </div>
      <nav className="sidebar-nav">
        <NavLink to="/" end className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">◉</span> Overview
        </NavLink>
        <NavLink to="/suites" className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">▦</span> Suites
        </NavLink>
        <NavLink to="/cells" className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">☰</span> Cells
        </NavLink>
        <NavLink to="/comparison" className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">⇔</span> Comparison
        </NavLink>
        <NavLink to="/langfuse" className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">◎</span> Langfuse
        </NavLink>
        <NavLink to="/langfuse-api" className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">⊕</span> Langfuse API
        </NavLink>
        <NavLink to="/settings" className={({ isActive }: { isActive: boolean }) => `nav-item ${isActive ? 'active' : ''}`}>
          <span className="nav-icon">⚙</span> Settings
        </NavLink>
      </nav>
      <div className="sidebar-footer">
        <div className={`status-dot ${mlxOk ? 'ok' : 'err'}`} />
        <span className="status-text">MLX {mlxOk ? 'connected' : 'disconnected'}</span>
      </div>
    </aside>
  );
}

function StatusBar({ mlxOk, cellCount }: { mlxOk: boolean; cellCount: number }) {
  return (
    <div className="status-bar">
      <span className={`status-pill ${mlxOk ? 'ok' : 'err'}`}>
        MLX: {mlxOk ? 'connected' : 'disconnected'}
      </span>
      <span className="status-pill">{cellCount} cells</span>
      <span className="status-pill faint">hwLedger v0.1.0</span>
    </div>
  );
}

function RunnerPanel({ mlxOk }: { mlxOk: boolean }) {
  const [running, setRunning] = useState(false);
  const [lastResult, setLastResult] = useState<string | null>(null);

  const triggerRun = useCallback(async () => {
    setRunning(true);
    setLastResult(null);
    try {
      const r = await fetch(`${MLX_DEFAULT}/v1/models`);
      if (r.ok) {
        const j = await r.json();
        setLastResult(`MLX models: ${JSON.stringify(j)}`);
      } else {
        setLastResult(`Error: ${r.status} ${r.statusText}`);
      }
    } catch (e) {
      setLastResult(`Connection failed: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  }, []);

  return (
    <div className="runner-panel">
      <h3>Benchmark Runner</h3>
      <p className="muted">Trigger benchmark runs against the local MLX server.</p>
      <div className="runner-actions">
        <button type="button" className="gt-btn" onClick={triggerRun} disabled={running || !mlxOk}>
          {running ? 'Running…' : 'Run Benchmarks'}
        </button>
        <span className="faint">{MLX_DEFAULT}</span>
      </div>
      {lastResult && <pre className="reply-box">{lastResult}</pre>}
    </div>
  );
}

function SettingsPage({ mlxUrl, onMlxUrlChange }: { mlxUrl: string; onMlxUrlChange: (v: string) => void }) {
  return (
    <div className="view-content">
      <h2>Settings</h2>
      <div className="settings-grid">
        <label className="setting-row">
          <span className="setting-label">MLX Server URL</span>
          <input className="setting-input" value={mlxUrl} onChange={(e) => onMlxUrlChange(e.target.value)} />
        </label>
        <label className="setting-row">
          <span className="setting-label">Langfuse Cloud URL</span>
          <input className="setting-input" value="https://us.cloud.langfuse.com" readOnly />
        </label>
      </div>
    </div>
  );
}

function OverviewPage({ cells, summary, onJumpToSuite }: {
  cells: Cell[];
  summary: SummaryData;
  onJumpToSuite: (suite: string) => void;
}) {
  return (
    <div className="view-content">
      <Overview cells={cells} summary={summary} onJumpToSuite={onJumpToSuite} />
      <RunnerPanel mlxOk={true} />
    </div>
  );
}

function AppInner() {
  const [cells, setCells] = useState<Cell[]>([]);
  const [summary, setSummary] = useState<SummaryData | null>(null);
  const [mlxOk, setMlxOk] = useState(false);
  const [sortKey, setSortKey] = useState('task_id');
  const [sortDir, setSortDir] = useState<1 | -1>(1);
  const [groupBy, setGroupBy] = useState<GroupBy>('none');
  const [mlxUrl, setMlxUrl] = useState(MLX_DEFAULT);
  const navigate = useNavigate();

  useEffect(() => {
    const checkMlx = async () => {
      try {
        const r = await fetch(`${mlxUrl}/v1/models`);
        setMlxOk(r.ok);
      } catch {
        setMlxOk(false);
      }
    };
    void checkMlx();
    const id = setInterval(checkMlx, 10000);
    return () => clearInterval(id);
  }, [mlxUrl]);

  useEffect(() => {
    const loadData = async () => {
      try {
        const r = await fetch('/api/state');
        if (r.ok) {
          const j = await r.json();
          if (j?.data?.cells) setCells(j.data.cells);
          if (j?.data?.summary) setSummary(j.data.summary);
        }
      } catch {
        // silent — data loads from bench-cockpit backend when available
      }
    };
    void loadData();
  }, []);

  const handleSort = useCallback((key: string) => {
    setSortKey((prev) => {
      if (prev === key) {
        setSortDir((d) => (d === 1 ? -1 : 1));
        return key;
      }
      setSortDir(1);
      return key;
    });
  }, []);

  const handleJumpToSuite = useCallback((_suite: string) => {
    navigate('/suites');
  }, [navigate]);

  const defaultSummary: SummaryData = {
    meta: { model: 'unknown', n_cells: 0, n_suites: 0 },
    by_variant: {},
  };

  return (
    <div className="app-layout">
      <Sidebar mlxOk={mlxOk} onNavigate={(p) => navigate(p)} />
      <main className="main-panel">
        <Routes>
          <Route path="/" element={
            <OverviewPage
              cells={cells}
              summary={summary ?? defaultSummary}
              onJumpToSuite={handleJumpToSuite}
            />
          } />
          <Route path="/suites" element={
            <Suites cells={cells} onOpenSuite={handleJumpToSuite} />
          } />
          <Route path="/cells" element={
            <CellsTable
              cells={cells}
              sortKey={sortKey}
              sortDir={sortDir}
              groupBy={groupBy}
              onSelect={(c) => { if (c) console.log('selected', c); }}
              onSort={handleSort}
              onGroup={setGroupBy}
            />
          } />
          <Route path="/comparison" element={
            <Comparison cells={cells} />
          } />
          <Route path="/langfuse" element={
            <div className="view-content">
              <LangfuseEmbed />
            </div>
          } />
          <Route path="/langfuse-api" element={
            <LangfusePanel />
          } />
          <Route path="/settings" element={
            <SettingsPage mlxUrl={mlxUrl} onMlxUrlChange={setMlxUrl} />
          } />
        </Routes>
      </main>
      <StatusBar mlxOk={mlxOk} cellCount={cells.length} />
    </div>
  );
}

export default function App() {
  return (
    <HashRouter>
      <AppInner />
    </HashRouter>
  );
}
