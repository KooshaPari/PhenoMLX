import React, { useState } from 'react';

const LANGFUSE_CLOUD_URL = 'https://us.cloud.langfuse.com';

export default function LangfuseEmbed() {
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);

  return (
    <div className="langfuse-embed" data-testid="langfuse-embed">
      <div className="langfuse-embed-header">
        <h3>Langfuse Cloud Dashboard</h3>
        <span className="faint">Embedded view — {LANGFUSE_CLOUD_URL}</span>
        <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
          <button
            type="button"
            className="gt-btn"
            onClick={() => {
              setLoaded(false);
              setError(false);
              const iframe = document.getElementById('langfuse-iframe') as HTMLIFrameElement;
              if (iframe) iframe.src = iframe.src;
            }}
          >
            Reload
          </button>
          <a className="gt-btn" href={LANGFUSE_CLOUD_URL} target="_blank" rel="noreferrer">
            Open in new tab
          </a>
        </div>
      </div>

      {!loaded && !error && (
        <div className="langfuse-embed-loading">
          Loading Langfuse Cloud…
        </div>
      )}

      {error && (
        <div className="langfuse-embed-error">
          <p>Failed to load Langfuse Cloud iframe.</p>
          <p className="faint">
            This may be due to X-Frame-Options or CSP restrictions on the Langfuse dashboard.
            Use "Open in new tab" instead.
          </p>
          <a className="gt-btn" href={LANGFUSE_CLOUD_URL} target="_blank" rel="noreferrer" style={{ marginTop: 12 }}>
            Open Langfuse in browser
          </a>
        </div>
      )}

      <iframe
        id="langfuse-iframe"
        src={LANGFUSE_CLOUD_URL}
        title="Langfuse Cloud Dashboard"
        className={`langfuse-iframe ${loaded ? 'loaded' : 'hidden'}`}
        sandbox="allow-same-origin allow-scripts allow-popups allow-forms allow-modals"
        onLoad={() => setLoaded(true)}
        onError={() => setError(true)}
      />
    </div>
  );
}
