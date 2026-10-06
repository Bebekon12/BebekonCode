import { useEffect, useState } from 'react';
import { FileCode2, GitBranch, RefreshCw, ShieldCheck, X } from 'lucide-react';
import type { ClientTransport, GitStatus, Workspace, Session } from '../contracts';
export function ContextPanel({
  client,
  workspace,
  session,
  close,
}: {
  client: ClientTransport | null;
  workspace?: Workspace;
  session?: Session;
  close: () => void;
}) {
  const [tab, setTab] = useState<'context' | 'changes'>('context');
  const [git, setGit] = useState<GitStatus>();
  const [diff, setDiff] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (!client || !workspace || tab !== 'changes') return;
    let alive = true;
    setBusy(true);
    setError('');
    setGit(undefined);
    setDiff('');
    void Promise.all([client.gitStatus(workspace.id), client.gitDiff(workspace.id)])
      .then(([status, diff]) => {
        if (alive) {
          setGit(status);
          setDiff(diff);
        }
      })
      .catch((error) => {
        if (alive) setError(String(error));
      })
      .finally(() => {
        if (alive) setBusy(false);
      });
    return () => {
      alive = false;
    };
  }, [client, workspace, tab, revision]);
  return (
    <aside className="context-panel" aria-label="Project context">
      <div className="panel-tabs">
        <button className={tab === 'context' ? 'active' : ''} onClick={() => setTab('context')}>
          Context
        </button>
        <button className={tab === 'changes' ? 'active' : ''} onClick={() => setTab('changes')}>
          Changes
        </button>
        <button
          className="icon-button panel-close"
          aria-label="Close context panel"
          onClick={close}
        >
          <X size={15} />
        </button>
      </div>
      {tab === 'context' ? (
        <div className="context-body">
          <div className="eyebrow">WORKSPACE</div>
          <div className="context-title">
            <FileCode2 size={17} />
            {workspace?.name ?? 'No project selected'}
          </div>
          <code className="path">{workspace?.root ?? 'Add a local folder to begin'}</code>
          <div className="context-divider" />
          <div className="eyebrow">SESSION BOUNDARIES</div>
          <dl>
            <dt>Agent</dt>
            <dd>{session ? 'Local demo' : '—'}</dd>
            <dt>Account</dt>
            <dd>{session ? 'Local demo' : '—'}</dd>
            <dt>Isolation</dt>
            <dd>Current workspace</dd>
            <dt>Permissions</dt>
            <dd>{session?.permission_profile === 'read_only' ? 'Read only' : 'Standard'}</dd>
          </dl>
          <div className="context-note">
            <ShieldCheck size={18} />
            <p>
              Local by default.
              <br />
              <span>No telemetry or remote listener. The demo does not access project files.</span>
            </p>
          </div>
          <div className="context-divider" />
          <div className="eyebrow">NEXT MILESTONE</div>
          <p className="muted small">
            Official provider adapters, isolated sign-in, process supervision and worktree sessions.
          </p>
        </div>
      ) : (
        <div className="context-body">
          <div className="row-between">
            <div className="eyebrow">GIT CHANGES</div>
            <button
              className="icon-button"
              aria-label="Refresh Git changes"
              disabled={busy}
              onClick={() => setRevision((v) => v + 1)}
            >
              <RefreshCw size={15} className={busy ? 'spin' : ''} />
            </button>
          </div>
          {!workspace && <p className="muted">Select a project to inspect changes.</p>}
          {busy && (
            <p className="muted" role="status">
              Reading Git status…
            </p>
          )}
          {error && <p className="notice">{error}</p>}
          {git && (
            <>
              <div className="branch">
                <GitBranch size={14} />
                {git.branch}
              </div>
              <p className="muted small">
                {git.files.length} changed files · untracked files have no diff
              </p>
              <div className="changed-files">
                {git.files.map((file) => (
                  <div key={file.path}>
                    <span className="file-status">{file.status}</span>
                    <code title={file.path}>{file.path}</code>
                  </div>
                ))}
              </div>
              {git.files.length === 0 && <p className="muted">Working tree is clean.</p>}
              {git.files.length > 0 && (
                <details className="diff-details">
                  <summary>View unified diff</summary>
                  <pre className="diff">{diff}</pre>
                </details>
              )}
            </>
          )}
        </div>
      )}
    </aside>
  );
}
