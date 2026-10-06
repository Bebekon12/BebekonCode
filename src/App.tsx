import { useEffect, useRef, useState } from 'react';
import {
  ArrowUp,
  ArrowUpRight,
  ChevronDown,
  ChevronRight,
  Command,
  Folder,
  FolderPlus,
  GitBranch,
  Layers3,
  MoreHorizontal,
  PanelRightClose,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  Square,
  Terminal,
  X,
} from 'lucide-react';
import type {
  AgentEvent,
  ClientTransport,
  CreateSession,
  ReleaseCheck,
  Session,
  Snapshot,
} from './contracts';
import { browserPreview, getTransport } from './transport';
import { eventStatus, mergeEvents } from './timeline';
import { Dialog } from './components/Dialog';
import { Timeline } from './components/Timeline';
import { ContextPanel } from './components/ContextPanel';
import { Settings } from './components/Settings';
import product from '../product.json';

export function App() {
  const [client, setClient] = useState<ClientTransport | null>(null);
  const [data, setData] = useState<Snapshot | null>(null);
  const [projectId, setProjectId] = useState('');
  const [sessionId, setSessionId] = useState('');
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [historyPage, setHistoryPage] = useState(false);
  const browsingHistory = useRef(historyPage);
  browsingHistory.current = historyPage;
  const [dialog, setDialog] = useState<'new' | 'settings' | 'commands' | null>(null);
  const [context, setContext] = useState(true);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [release, setRelease] = useState<ReleaseCheck | null>(null);
  const currentId = useRef(sessionId);
  currentId.current = sessionId;
  const dataRef = useRef(data);
  dataRef.current = data;
  const composer = useRef<HTMLTextAreaElement>(null);
  const session = data?.sessions.find((session) => session.id === sessionId);
  const workspace = data?.workspaces.find(
    (workspace) => workspace.id === (session?.workspace_id ?? projectId),
  );
  const account = data?.accounts.find((account) => account.id === session?.account_profile_id);
  const draft = drafts[sessionId] ?? '';
  const running = session?.status === 'running';

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let batch: AgentEvent[] = [];
    const flush = () => {
      timer = undefined;
      const incoming = batch;
      batch = [];
      if (!alive) return;
      if (!browsingHistory.current)
        setEvents((current) =>
          mergeEvents(
            current,
            incoming.filter((event) => event.session_id === currentId.current),
          ),
        );
      setData((current) =>
        current
          ? {
              ...current,
              sessions: current.sessions.map((session) => {
                const relevant = incoming.filter((event) => event.session_id === session.id);
                if (!relevant.length) return session;
                let result = session;
                for (const event of relevant) {
                  const status = eventStatus(event);
                  const title =
                    event.payload.type === 'turn_started' && result.title === 'New session'
                      ? event.payload.prompt.slice(0, 64)
                      : result.title;
                  result = {
                    ...result,
                    title,
                    status: status ?? result.status,
                    updated_at: event.timestamp,
                  };
                }
                return result;
              }),
            }
          : current,
      );
    };
    void (async () => {
      try {
        const transport = await getTransport();
        if (!alive) return;
        unlisten = await transport.subscribe(
          (event) => {
            batch.push(event);
            if (!timer) timer = setTimeout(flush, 50);
          },
          () => {
            const id = currentId.current;
            void Promise.all([
              transport.snapshot(),
              id ? transport.events(id) : Promise.resolve([]),
            ])
              .then(([snapshot, history]) => {
                if (!alive) return;
                setData(snapshot);
                if (currentId.current === id) setEvents((current) => mergeEvents(current, history));
              })
              .catch((error) => {
                if (alive) setError(String(error));
              });
          },
        );
        if (!alive) {
          unlisten();
          return;
        }
        const snapshot = await transport.snapshot();
        if (!alive) return;
        setClient(transport);
        setData(snapshot);
        setProjectId(snapshot.workspaces[0]?.id ?? '');
        setSessionId(snapshot.sessions[0]?.id ?? '');
        if (snapshot.settings.check_updates_on_start) {
          const check = await transport.checkReleases(false);
          if (alive) setRelease(check);
        }
      } catch (error) {
        if (alive) setError(String(error));
      }
    })();
    return () => {
      alive = false;
      unlisten?.();
      if (timer) clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    setEvents([]);
    setHistoryPage(false);
    if (!client || !sessionId) return;
    let alive = true;
    void client
      .events(sessionId)
      .then((history) => {
        if (alive) setEvents((current) => mergeEvents(current, history));
      })
      .catch((error) => {
        if (alive) setError(String(error));
      });
    return () => {
      alive = false;
    };
  }, [client, sessionId]);

  // Reconcile a snapshot/live race from the durable latest timeline, including UI reloads.
  useEffect(() => {
    if (historyPage) return;
    const status = events
      .filter((event) => event.session_id === sessionId)
      .reverse()
      .map(eventStatus)
      .find((value) => value !== undefined);
    if (!status) return;
    setData((current) => {
      const session = current?.sessions.find((session) => session.id === sessionId);
      if (
        !current ||
        !session ||
        session.status === status ||
        (session.status === 'interrupted' && status === 'running')
      )
        return current;
      return {
        ...current,
        sessions: current.sessions.map((session) =>
          session.id === sessionId ? { ...session, status } : session,
        ),
      };
    });
  }, [events, sessionId, historyPage]);

  const action = async (work: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await work();
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  };
  const addProject = () => {
    if (!client) return;
    void action(async () => {
      const root = await client.chooseFolder();
      if (!root) return;
      const workspace = await client.addWorkspace(root);
      setData((current) =>
        current
          ? {
              ...current,
              workspaces: current.workspaces.some((item) => item.id === workspace.id)
                ? current.workspaces
                : [...current.workspaces, workspace],
            }
          : current,
      );
      setProjectId(workspace.id);
      setSessionId('');
    });
  };
  const selectSession = (session: Session) => {
    setSessionId(session.id);
    setProjectId(session.workspace_id);
  };
  const newSession = () => {
    if (!data?.workspaces.length) addProject();
    else setDialog('new');
  };
  const send = () => {
    if (!client || !session || !draft.trim() || busy || running) return;
    const prompt = draft.trim();
    void action(async () => {
      await client.sendMessage(session.id, prompt);
      setDrafts((current) => ({ ...current, [session.id]: '' }));
    });
  };
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setDialog((current) => (current === 'commands' ? null : 'commands'));
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        if (dataRef.current?.workspaces.length) setDialog('new');
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Projects and sessions">
        <div className="brand">
          <img src="/mark.svg" alt="" />
          <span>{product.name}</span>
          <span className="version-label">EARLY ACCESS</span>
        </div>
        <button className="command-button" onClick={() => setDialog('commands')} disabled={!data}>
          <Search size={15} />
          <span>Search & commands</span>
          <kbd>Ctrl K</kbd>
        </button>
        <div className="sidebar-heading">
          <span>WORKSPACES</span>
          <button
            className="icon-button"
            aria-label="Add project"
            onClick={addProject}
            disabled={busy || !client}
          >
            <Plus size={15} />
          </button>
        </div>
        <div className="workspace-list">
          {data?.workspaces.map((project) => (
            <section className="workspace-group" key={project.id}>
              <button
                className={`project-button ${workspace?.id === project.id ? 'current-project' : ''}`}
                onClick={() => {
                  setProjectId(project.id);
                  setSessionId(
                    data.sessions.find((session) => session.workspace_id === project.id)?.id ?? '',
                  );
                }}
              >
                <ChevronDown size={13} />
                <Folder size={15} />
                <span>{project.name}</span>
              </button>
              {data.sessions
                .filter((session) => session.workspace_id === project.id)
                .map((item) => (
                  <button
                    className={`session-button ${sessionId === item.id ? 'selected' : ''}`}
                    key={item.id}
                    onClick={() => selectSession(item)}
                    title={item.title}
                  >
                    <span className={`status-dot ${item.status}`} aria-label={item.status} />
                    <span className="session-copy">
                      <span className="session-title">{item.title}</span>
                      <span className="session-meta">
                        Demo ·{' '}
                        {data.accounts.find((account) => account.id === item.account_profile_id)
                          ?.label ?? item.account_profile_id}
                      </span>
                    </span>
                  </button>
                ))}
              {!data.sessions.some((session) => session.workspace_id === project.id) && (
                <button
                  className="project-new"
                  onClick={() => {
                    setProjectId(project.id);
                    setDialog('new');
                  }}
                >
                  <Plus size={13} /> Start a session
                </button>
              )}
            </section>
          ))}
          {data && !data.workspaces.length && (
            <div className="sidebar-empty">
              Your projects live here.
              <br />
              Add a folder to get started.
            </div>
          )}
        </div>
        <div className="sidebar-bottom">
          <button className="sidebar-action" onClick={newSession} disabled={busy || !client}>
            <Plus size={16} /> New session <kbd>Ctrl N</kbd>
          </button>
          <button className="sidebar-action" onClick={() => setDialog('settings')} disabled={!data}>
            <Settings2 size={16} /> Settings{' '}
            {release?.available && <span className="update-dot" aria-label="Update available" />}
          </button>
          <div className="local-status">
            <span className="status-dot completed" /> Local core{' '}
            <span>{browserPreview ? 'Preview' : 'Desktop'}</span>
          </div>
        </div>
      </aside>

      <main className="main-workspace">
        <header className="topbar">
          <div className="breadcrumb">
            <Folder size={16} />
            <span>{workspace?.name ?? 'Workspace'}</span>
            <ChevronRight size={13} />
            <span className="muted">{session ? 'Agent session' : 'Overview'}</span>
          </div>
          <div className="topbar-actions">
            <span className="local-badge">
              <ShieldCheck size={13} /> Local-first
            </span>
            <button
              className="icon-button"
              aria-label="Toggle context panel"
              onClick={() => setContext((value) => !value)}
            >
              <PanelRightClose size={17} />
            </button>
          </div>
        </header>
        {browserPreview && (
          <div className="preview-banner">
            Development browser preview · data stays in memory · filesystem and release checks
            require the desktop app
          </div>
        )}
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button className="icon-button" aria-label="Dismiss error" onClick={() => setError('')}>
              <X size={15} />
            </button>
          </div>
        )}
        {release?.available && (
          <button className="release-banner" onClick={() => setDialog('settings')}>
            Version {release.latest_version} is available · view release notes{' '}
            <ArrowUpRight size={14} />
          </button>
        )}
        <div className="workspace-content">
          <div className="conversation-column">
            {session ? (
              <>
                <div className="session-header">
                  <div>
                    <div className="eyebrow">AGENT SESSION</div>
                    <h1>{session.title}</h1>
                    <div className="session-subtitle">
                      <span className={`status-dot ${session.status}`} />
                      <span>{session.status}</span>
                      <span className="separator">/</span>
                      <span>Local demo</span>
                      <span className="separator">/</span>
                      <span>{account?.label}</span>
                    </div>
                  </div>
                  <button
                    className="icon-button"
                    aria-label="Session details"
                    onClick={() => setContext(true)}
                  >
                    <MoreHorizontal size={20} />
                  </button>
                </div>
                {historyPage && (
                  <button
                    className="history-banner"
                    onClick={() =>
                      client &&
                      void action(async () => {
                        const selected = session.id;
                        const latest = await client.events(selected);
                        if (currentId.current === selected) {
                          setEvents(latest);
                          setHistoryPage(false);
                        }
                      })
                    }
                  >
                    Viewing earlier activity · back to latest
                  </button>
                )}
                <Timeline
                  session={session}
                  events={events.filter((event) => event.session_id === session.id)}
                  loadOlder={() => {
                    if (!client || !events.length) return;
                    const selected = session.id;
                    void action(async () => {
                      const older = await client.events(selected, events[0]?.sequence);
                      if (currentId.current === selected && older.length) {
                        setHistoryPage(true);
                        setEvents(older);
                      }
                    });
                  }}
                />
                <div className="composer-region">
                  <div className={`composer ${running ? 'composer-running' : ''}`}>
                    <textarea
                      ref={composer}
                      aria-label="Message to agent"
                      placeholder="Describe a task, ask a question, or explore an idea…"
                      value={draft}
                      maxLength={16000}
                      disabled={running}
                      onChange={(event) =>
                        setDrafts((current) => ({ ...current, [session.id]: event.target.value }))
                      }
                      onKeyDown={(event) => {
                        if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
                          event.preventDefault();
                          send();
                        }
                      }}
                    />
                    <div className="composer-toolbar">
                      <div className="composer-controls">
                        <span className="control-pill">
                          <Sparkles size={13} /> Local demo
                        </span>
                        <span className="control-pill">mock-stream-v1</span>
                        <span className="control-pill account-pill">{account?.label}</span>
                        <span className="permission-pill">
                          <ShieldCheck size={13} />
                          {session.permission_profile === 'read_only' ? 'Read only' : 'Standard'}
                        </span>
                      </div>
                      {running ? (
                        <button
                          className="send-button stop-button"
                          aria-label="Stop agent"
                          onClick={() => client && void action(() => client.cancel(session.id))}
                        >
                          <Square size={14} />
                        </button>
                      ) : (
                        <button
                          className="send-button"
                          aria-label="Send message"
                          disabled={!draft.trim() || busy}
                          onClick={send}
                        >
                          <ArrowUp size={17} />
                        </button>
                      )}
                    </div>
                  </div>
                  <div className="composer-caption">
                    <span>Local simulator · no project files are read or modified</span>
                    <span>
                      <kbd>Ctrl ↵</kbd> to send
                    </span>
                  </div>
                </div>
              </>
            ) : (
              <div className="welcome">
                <div className="welcome-top">
                  <span className="eyebrow">YOUR DEVELOPMENT WORKSPACE</span>
                  <span className="welcome-version">01 / GET STARTED</span>
                </div>
                <div className="welcome-main">
                  <div className="welcome-mark">
                    <Layers3 size={30} />
                  </div>
                  <h1>
                    A place for your
                    <br />
                    <span>next great idea.</span>
                  </h1>
                  <p>
                    Bring your projects and coding agents together.
                    <br />
                    One calm workspace. Everything under your control.
                  </p>
                  <div className="welcome-actions">
                    <button
                      className="primary-button"
                      onClick={data?.workspaces.length ? newSession : addProject}
                      disabled={busy || !client}
                    >
                      {data?.workspaces.length ? <Plus size={16} /> : <FolderPlus size={16} />}{' '}
                      {data?.workspaces.length ? 'Start a session' : 'Add your first project'}{' '}
                      <ArrowUpRight size={15} />
                    </button>
                    <button
                      className="text-button"
                      onClick={() => setDialog('settings')}
                      disabled={!data}
                    >
                      Explore providers <ChevronRight size={14} />
                    </button>
                  </div>
                  <div className="welcome-note">
                    <ShieldCheck size={14} />
                    <span>Your folders stay yours. No account required for the demo.</span>
                  </div>
                </div>
                <div className="welcome-features">
                  <div>
                    <Layers3 size={18} />
                    <strong>Independent sessions</strong>
                    <p>
                      Separate agents, accounts
                      <br />
                      and task histories.
                    </p>
                  </div>
                  <div>
                    <GitBranch size={18} />
                    <strong>Project context</strong>
                    <p>
                      Read Git status and diffs
                      <br />
                      from your local workspace.
                    </p>
                  </div>
                  <div>
                    <Terminal size={18} />
                    <strong>A core you control</strong>
                    <p>
                      Rust, SQLite and native IPC.
                      <br />
                      No cloud workspace server.
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>
          {context && (
            <ContextPanel
              client={client}
              workspace={workspace}
              session={session}
              close={() => setContext(false)}
            />
          )}
        </div>
        <footer className="statusbar">
          <span>
            <ShieldCheck size={11} />{' '}
            {browserPreview ? 'In-memory browser preview' : 'Local workspace'}
          </span>
          <span>
            {data?.sessions.filter((session) => session.status === 'running').length ?? 0} running{' '}
            <span className="statusbar-separator">·</span> No remote access
          </span>
        </footer>
      </main>

      {dialog === 'new' && data && client && (
        <NewSession
          data={data}
          initialProject={projectId}
          busy={busy}
          close={() => setDialog(null)}
          create={(input) =>
            void action(async () => {
              const session = await client.createSession(input);
              setData((current) =>
                current ? { ...current, sessions: [session, ...current.sessions] } : current,
              );
              selectSession(session);
              setDialog(null);
              setTimeout(() => composer.current?.focus(), 0);
            })
          }
        />
      )}
      {dialog === 'settings' && data && client && (
        <Settings
          client={client}
          settings={data.settings}
          providers={data.providers}
          release={release}
          updateRelease={setRelease}
          close={() => setDialog(null)}
          updateSettings={(settings) =>
            setData((current) => (current ? { ...current, settings } : current))
          }
          updateProviders={(providers) =>
            setData((current) => (current ? { ...current, providers } : current))
          }
        />
      )}
      {dialog === 'commands' && data && (
        <CommandPalette
          sessions={data.sessions}
          close={() => setDialog(null)}
          run={(command) => {
            setDialog(null);
            command();
          }}
          commands={[
            { label: 'New agent session', icon: <Plus size={16} />, action: newSession },
            { label: 'Add project folder', icon: <FolderPlus size={16} />, action: addProject },
            {
              label: 'Open settings',
              icon: <Settings2 size={16} />,
              action: () => setDialog('settings'),
            },
            {
              label: 'Toggle context panel',
              icon: <PanelRightClose size={16} />,
              action: () => setContext((value) => !value),
            },
            ...(running && client
              ? [
                  {
                    label: 'Stop current agent',
                    icon: <Square size={16} />,
                    action: () => void action(() => client.cancel(sessionId)),
                  },
                ]
              : []),
          ]}
          select={selectSession}
        />
      )}
    </div>
  );
}

function NewSession({
  data,
  initialProject,
  close,
  create,
  busy,
}: {
  data: Snapshot;
  initialProject: string;
  close: () => void;
  create: (input: CreateSession) => void;
  busy: boolean;
}) {
  const [project, setProject] = useState(initialProject || data.workspaces[0]?.id || '');
  const [provider, setProvider] = useState(
    data.providers.find((provider) => provider.available)?.id ?? '',
  );
  const availableAccounts = data.accounts.filter((account) => account.provider === provider);
  const [account, setAccount] = useState(availableAccounts[0]?.id ?? '');
  const models = data.providers.find((item) => item.id === provider)?.models ?? [];
  const [model, setModel] = useState(models[0] ?? '');
  const [permissions, setPermissions] = useState('standard');
  return (
    <Dialog title="New agent session" close={close}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          create({
            workspace_id: project,
            provider,
            account_profile_id: account,
            model,
            permission_profile: permissions,
          });
        }}
      >
        <p className="muted dialog-description">Choose the engine and boundaries for this task.</p>
        <label className="field">
          Project
          <select
            aria-label="Project"
            value={project}
            onChange={(event) => setProject(event.target.value)}
          >
            {data.workspaces.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <div className="field-grid">
          <label className="field">
            Agent
            <select
              aria-label="Agent"
              value={provider}
              onChange={(event) => {
                setProvider(event.target.value);
                setAccount(
                  data.accounts.find((account) => account.provider === event.target.value)?.id ??
                    '',
                );
                setModel(
                  data.providers.find((provider) => provider.id === event.target.value)
                    ?.models[0] ?? '',
                );
              }}
            >
              {data.providers.map((item) => (
                <option key={item.id} value={item.id} disabled={!item.available}>
                  {item.name}
                  {item.available ? '' : ' · not integrated'}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            Account
            <select
              aria-label="Account"
              value={account}
              onChange={(event) => setAccount(event.target.value)}
            >
              {availableAccounts.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label className="field">
          Model
          <select
            aria-label="Model"
            value={model}
            onChange={(event) => setModel(event.target.value)}
          >
            {models.map((model) => (
              <option key={model}>{model}</option>
            ))}
          </select>
        </label>
        <div className="field">
          <span>Isolation</span>
          <div className="isolation-choice">
            <Folder size={18} />
            <div>
              <strong>Current workspace</strong>
              <p>The simulator does not read or change files.</p>
            </div>
            <CheckMark />
          </div>
          <p className="small muted">Git worktree sessions are planned for the next milestone.</p>
        </div>
        <label className="field">
          Permissions
          <select
            aria-label="Permissions"
            value={permissions}
            onChange={(event) => setPermissions(event.target.value)}
          >
            <option value="standard">Standard</option>
            <option value="read_only">Read only</option>
          </select>
        </label>
        <div className="dialog-footer">
          <button type="button" className="secondary-button" onClick={close}>
            Cancel
          </button>
          <button
            className="primary-button"
            type="submit"
            disabled={busy || !project || !provider || !account || !model}
          >
            <Plus size={15} /> Create session
          </button>
        </div>
      </form>
    </Dialog>
  );
}
function CheckMark() {
  return <span className="choice-check">✓</span>;
}
interface PaletteCommand {
  label: string;
  icon: React.ReactNode;
  action: () => void;
}
function CommandPalette({
  commands,
  sessions,
  close,
  run,
  select,
}: {
  commands: PaletteCommand[];
  sessions: Session[];
  close: () => void;
  run: (command: () => void) => void;
  select: (session: Session) => void;
}) {
  const [query, setQuery] = useState('');
  const all = [
    ...commands,
    ...sessions.map((session) => ({
      label: `Switch to ${session.title}`,
      icon: <Sparkles size={16} />,
      action: () => select(session),
    })),
  ];
  const matches = all.filter((command) => fuzzyMatch(query, command.label));
  return (
    <Dialog title="Commands" close={close}>
      <div className="palette-search">
        <Search size={18} />
        <input
          autoFocus
          aria-label="Search commands"
          placeholder="Find a command or session…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && matches[0]) run(matches[0].action);
          }}
        />
        <Command size={15} />
      </div>
      <div className="palette-list">
        {matches.map((command, index) => (
          <button key={`${command.label}-${index}`} onClick={() => run(command.action)}>
            {command.icon}
            <span>{command.label}</span>
            <ChevronRight size={14} />
          </button>
        ))}
        {!matches.length && <p className="muted">No matching commands.</p>}
      </div>
    </Dialog>
  );
}
function fuzzyMatch(query: string, text: string): boolean {
  let position = 0;
  const haystack = text.toLowerCase();
  for (const character of query.trim().toLowerCase()) {
    position = haystack.indexOf(character, position);
    if (position < 0) return false;
    position++;
  }
  return true;
}
