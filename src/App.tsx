import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ChevronRight,
  FileCode2,
  FolderPlus,
  GitBranch,
  PanelRightClose,
  Plus,
  Settings2,
  Sparkles,
  Square,
  Terminal,
  Trash2,
  X,
} from 'lucide-react';
import type {
  AgentEvent,
  ClientTransport,
  CreateChat,
  ReleaseCheck,
  Session,
  Snapshot,
} from './contracts';
import { browserPreview, getTransport } from './transport';
import { eventStatus, historyPageSize, mergeEvents } from './timeline';
import { usePreference } from './preferences';
import { useAccountState } from './accounts';
import { Timeline } from './components/Timeline';
import { ContextPanel } from './components/ContextPanel';
import { Settings } from './components/Settings';
import { FileManager } from './components/FileManager';
import { Titlebar } from './components/Titlebar';
import { Sidebar } from './components/Sidebar';
import { Composer } from './components/Composer';
import { WelcomeHero } from './components/WelcomeHero';
import { NewSession } from './components/NewSessionDialog';
import { CommandPalette } from './components/CommandPalette';
import { PlanWelcome } from './components/PlanWelcome';
import { TeamComposerAgents } from './components/TeamComposerAgents';
import { ChatControls } from './components/ChatControls';
import { ChatSettings } from './components/ChatSettings';
import { TeamPanel } from './components/TeamPanel';
import { ProjectChanges } from './components/ProjectChanges';
import { ExtensionsDialog } from './components/ExtensionsDialog';
import { ContextMenu } from './components/ContextMenu';
import { Dialog } from './components/Dialog';
import { modeLabels } from './chat';
import type { DraftAttachment } from './attachments';
import { counted, errorText, sessionTitle, statusLabels } from './locale';

export function App() {
  const [client, setClient] = useState<ClientTransport | null>(null);
  const [data, setData] = useState<Snapshot | null>(null);
  const [projectId, setProjectId] = useState('');
  const [sessionId, setSessionId] = useState('');
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [attachmentDrafts, setAttachmentDrafts] = useState<Record<string, DraftAttachment[]>>({});
  const [historyLoading, setHistoryLoading] = useState(false);
  const [deleting, setDeleting] = useState<Session | null>(null);
  const [dialog, setDialog] = useState<
    'new' | 'settings' | 'commands' | 'files' | 'changes' | 'plugins' | null
  >(null);
  const [newChatMode, setNewChatMode] = useState<CreateChat['mode']>('single');
  const [newChatProject, setNewChatProject] = useState('');
  const [context, setContext] = usePreference('context-visible', false);
  const [theme, setTheme] = usePreference<'dark' | 'light'>('theme', 'dark');
  const [textSize, setTextSize] = usePreference<'comfortable' | 'large'>(
    'text-size',
    'comfortable',
  );
  useEffect(() => {
    document.documentElement.dataset.theme = theme === 'light' ? 'light' : 'dark';
    document.documentElement.dataset.textSize = textSize === 'large' ? 'large' : 'comfortable';
  }, [theme, textSize]);
  const [settingsTab, setSettingsTab] = useState('Основные');
  const [newSessionProvider, setNewSessionProvider] = useState<string>();
  const [agentDialog, setAgentDialog] = useState<{
    id: string;
    handoff: boolean;
    tools?: boolean;
  } | null>(null);
  const showSettings = (tab = 'Основные') => {
    setSettingsTab(tab);
    setDialog('settings');
  };
  const openTerminal = () => {
    if (client && workspace) void action(() => client.openProject(workspace.id, true));
  };
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [updateBusy, setUpdateBusy] = useState(false);
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
  const attachments = attachmentDrafts[sessionId] ?? [];
  const running = session?.status === 'running';
  const homeSessions =
    data?.sessions.filter(
      (item) => !item.parent_session_id && (!workspace || item.workspace_id === workspace.id),
    ) ?? [];
  const [tabIds, setTabIds] = usePreference<string[]>('project-tabs', []);
  const [sidebarCollapsed, setSidebarCollapsed] = usePreference('sidebar-collapsed', false);
  const [heroHidden, setHeroHidden] = usePreference('hero-hidden', false);

  // The open project always has a tab; closing a tab never removes the project itself.
  useEffect(() => {
    if (workspace && workspace.id !== 'chat-scratch' && !tabIds.includes(workspace.id))
      setTabIds((ids) => [...ids, workspace.id]);
  }, [workspace, tabIds, setTabIds]);

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
      if (
        incoming.some(
          (event) =>
            event.payload.type === 'turn_completed' ||
            event.payload.type === 'session_stopped' ||
            event.payload.type === 'provider_error' ||
            (event.payload.type === 'tool_activity' && event.payload.label === 'Подзадача создана'),
        )
      ) {
        void getTransport()
          .then((transport) => transport.snapshot())
          .then((snapshot) => {
            if (alive) setData(snapshot);
          })
          .catch((error) => alive && setError(errorText(error)));
      }
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
        setSessionId(snapshot.sessions.find((s) => !s.parent_session_id)?.id ?? '');
        if (snapshot.settings.check_updates_on_start) {
          const check = await transport.checkReleases(false);
          if (alive) setRelease(check);
        }
      } catch (error) {
        if (alive) setError(errorText(error));
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
    setHistoryLoading(false);
    if (!client || !sessionId) return;
    let alive = true;
    void (async () => {
      try {
        // Newest page first so the chat opens at once, then every older page of the stored
        // history; live events keep merging in meanwhile.
        let page = await client.events(sessionId);
        if (!alive) return;
        setEvents((current) => mergeEvents(current, page));
        setHistoryLoading(page.length >= historyPageSize);
        while (alive && page.length >= historyPageSize && page[0]) {
          page = await client.events(sessionId, page[0].sequence);
          if (!alive) return;
          const older = page;
          setEvents((current) => mergeEvents(current, older));
        }
      } catch (error) {
        if (alive) setError(errorText(error));
      } finally {
        if (alive) setHistoryLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [client, sessionId]);

  // Reconcile a snapshot/live race from the durable latest timeline, including UI reloads.
  useEffect(() => {
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
  }, [events, sessionId]);

  // Accounts live in the core; the UI re-reads them after account changes or sign-in events.
  const reloadAccounts = useCallback(() => {
    if (!client) return;
    void client
      .snapshot()
      .then((snapshot) =>
        setData((current) =>
          current
            ? { ...current, accounts: snapshot.accounts, providers: snapshot.providers }
            : snapshot,
        ),
      )
      .catch((error) => setError(errorText(error)));
  }, [client]);
  const accountState = useAccountState(client, data?.accounts ?? [], reloadAccounts);
  // First successful ChatGPT-plan sign-in per account gets a one-time confirmation.
  const [welcomed, setWelcomed] = usePreference<string[]>('plan-welcomed', []);
  const [planWelcome, setPlanWelcome] = useState('');
  useEffect(() => {
    for (const account of data?.accounts ?? []) {
      const status = accountState.statuses[account.id];
      if (
        account.provider === 'openai' &&
        status?.state === 'signed_in' &&
        status.plan_usage_enabled &&
        !welcomed.includes(account.id)
      ) {
        setWelcomed((ids) => [...ids, account.id]);
        setPlanWelcome(account.id);
        break;
      }
    }
  }, [data?.accounts, accountState.statuses, welcomed, setWelcomed]);

  const action = async (work: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await work();
    } catch (error) {
      setError(errorText(error));
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
    setNewChatMode('single');
    setNewChatProject(projectId);
    setDialog('new');
  };
  const openNewChat = (mode: CreateChat['mode'], project: string) => {
    setNewChatMode(mode);
    setNewChatProject(project);
    setDialog('new');
  };
  const send = () => {
    if (
      !client ||
      !session ||
      (!draft.trim() && !attachments.length) ||
      busy ||
      running ||
      data?.sessions.some((s) => s.id === session.parent_session_id && s.status === 'running')
    )
      return;
    const prompt = draft.trim() || 'Изучи прикреплённые файлы.';
    void action(async () => {
      await client.sendMessage(
        session.id,
        prompt,
        attachments.map(({ name, mime, data }) => ({ name, mime, data })),
      );
      setDrafts((current) => ({ ...current, [session.id]: '' }));
      setAttachmentDrafts((current) => ({ ...current, [session.id]: [] }));
    });
  };
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (dialog === 'files' || updateBusy) return; // File editor owns its unsaved-change navigation.
      if (
        (event.ctrlKey || event.metaKey) &&
        (event.code === 'KeyK' || event.key.toLowerCase() === 'k')
      ) {
        event.preventDefault();
        setDialog((current) => (current === 'commands' ? null : 'commands'));
      }
      if (
        (event.ctrlKey || event.metaKey) &&
        (event.code === 'KeyN' || event.key.toLowerCase() === 'n')
      ) {
        event.preventDefault();
        if (dataRef.current) {
          setNewChatMode('single');
          setNewChatProject(projectId);
          setDialog('new');
        }
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [dialog, updateBusy, projectId]);

  const providerName = (id?: string) =>
    data?.providers.find((provider) => provider.id === id)?.name ?? id ?? '';
  const tabs = tabIds.flatMap((id) => data?.workspaces.find((item) => item.id === id) ?? []);
  const selectProject = (id: string) => {
    setProjectId(id);
    setSessionId(
      data?.sessions.find((session) => !session.parent_session_id && session.workspace_id === id)
        ?.id ?? '',
    );
  };
  const closeTab = (id: string) => {
    const index = tabs.findIndex((tab) => tab.id === id);
    const remaining = tabs.filter((tab) => tab.id !== id);
    setTabIds(remaining.map((tab) => tab.id));
    if (workspace?.id !== id) return;
    const next = remaining[Math.min(index, remaining.length - 1)];
    if (next) selectProject(next.id);
    else {
      setProjectId('');
      setSessionId('');
    }
  };
  const cancelCurrent = () => client && session && void action(() => client.cancel(session.id));
  const hero = !heroHidden && (
    <WelcomeHero
      start={newSession}
      providers={() => showSettings('Провайдеры')}
      hide={() => setHeroHidden(true)}
      disabled={busy || !client}
    />
  );
  const projectTools = (
    <div className="workspace-actions" role="toolbar" aria-label="Инструменты проекта">
      <button
        className="text-button"
        aria-label="Файлы проекта"
        disabled={!workspace}
        onClick={() => setDialog('files')}
        title="Открыть файлы проекта"
      >
        <FileCode2 size={17} />
        <span>Файлы</span>
      </button>
      <button
        className="text-button"
        aria-label="Терминал"
        disabled={!workspace}
        onClick={openTerminal}
        title="Открыть PowerShell в папке проекта"
      >
        <Terminal size={17} />
        <span>Терминал</span>
      </button>
      <button
        className="text-button"
        aria-label="Изменения"
        disabled={!workspace}
        onClick={() => setDialog('changes')}
      >
        <GitBranch size={17} />
        <span>Изменения</span>
      </button>
    </div>
  );

  return (
    <div className={`app-shell ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      <Titlebar
        tabs={tabs}
        activeId={workspace?.id}
        selectTab={selectProject}
        closeTab={closeTab}
        addProject={client && !busy ? addProject : undefined}
        search={() => data && setDialog('commands')}
        toggleContext={() => setContext((value) => !value)}
        contextVisible={context}
        onError={(error) => setError(errorText(error))}
      />
      <Sidebar
        data={data}
        workspace={workspace}
        sessionId={sessionId}
        home={!session}
        collapsed={sidebarCollapsed}
        disabled={busy || !client}
        toggleCollapsed={() => setSidebarCollapsed((value) => !value)}
        newSession={newSession}
        createChat={(mode) => openNewChat(mode, '')}
        addProject={addProject}
        goHome={() => setSessionId('')}
        showSettings={showSettings}
        search={() => setDialog('commands')}
        openPlugins={() => setDialog('plugins')}
        selectProject={selectProject}
        selectSession={selectSession}
        startSession={(id) => {
          openNewChat('single', id);
        }}
        accountState={accountState}
        openUsage={(provider) => client && void action(() => client.openUsage(provider))}
        preview={browserPreview}
        deleteChat={setDeleting}
      />
      <ContextMenu
        extraActions={(target) => {
          const id = target.closest<HTMLElement>('[data-session-id]')?.dataset.sessionId;
          const chat = data?.sessions.find((item) => item.id === id && !item.parent_session_id);
          return chat && chat.status !== 'running' && !busy
            ? [
                {
                  label: 'Удалить чат',
                  icon: <Trash2 size={15} />,
                  danger: true,
                  run: () => setDeleting(chat),
                },
              ]
            : [];
        }}
      />

      <main className="main-workspace">
        {browserPreview && (
          <div className="preview-banner">
            Предпросмотр для разработки · данные хранятся в памяти · файлы и обновления доступны в
            приложении
          </div>
        )}
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="Закрыть сообщение об ошибке"
              onClick={() => setError('')}
            >
              <X size={15} />
            </button>
          </div>
        )}
        {release?.available && (
          <button
            className="release-banner"
            onClick={() => showSettings('О программе и обновления')}
          >
            Доступна версия {release.latest_version} · посмотреть изменения{' '}
            <ChevronRight size={14} />
          </button>
        )}
        <div className="workspace-content">
          <div className="conversation-column">
            {session ? (
              <>
                <div className="chat-title chat-titlebar">
                  <div className="chat-heading-copy">
                    <strong>{sessionTitle(session.title)}</strong>
                    <span className="chat-mode-label">
                      {modeLabels[session.chat_mode]} ·{' '}
                      <span className="chat-breadcrumb">
                        {session.workspace_id === 'chat-scratch' ? 'Без проекта' : workspace?.name}
                      </span>
                    </span>
                  </div>
                  {projectTools}
                </div>
                {session.parent_session_id && (
                  <div className="chat-branches">
                    <button
                      className="text-button"
                      onClick={() => {
                        const parent = data?.sessions.find(
                          (s) => s.id === session.parent_session_id,
                        );
                        if (parent) selectSession(parent);
                      }}
                    >
                      ← Вернуться в основной чат
                    </button>
                    <span>Отдельный контекст · общая задача</span>
                  </div>
                )}
                {(session.chat_mode === 'team' || session.chat_mode === 'auto') && data && (
                  <TeamPanel
                    session={session}
                    sessions={data.sessions}
                    select={selectSession}
                    configure={(member) => setAgentDialog({ id: member.id, handoff: false })}
                  />
                )}
                <Timeline
                  loadImage={
                    client ? (path) => client.attachmentImage(session.id, path) : undefined
                  }
                  session={session}
                  providerName={providerName(session.provider)}
                  openFiles={() => setDialog('files')}
                  openTerminal={openTerminal}
                  cancel={cancelCurrent}
                  resolveApproval={(approvalId, decision) =>
                    client
                      ? client.resolveApproval(session.id, approvalId, decision)
                      : Promise.resolve()
                  }
                  retry={(prompt) => {
                    if (client)
                      void action(async () => {
                        await client.sendMessage(session.id, prompt);
                      });
                  }}
                  manageUsage={
                    session.provider === 'openai' && client
                      ? () => void action(() => client.openUsage('openai'))
                      : undefined
                  }
                  chooseAnotherAccount={() => {
                    setAgentDialog({ id: session.id, handoff: true });
                  }}
                  changes={
                    client &&
                    workspace &&
                    workspace.id !== 'chat-scratch' &&
                    session.provider !== 'mock'
                      ? {
                          load: async () => {
                            const [status, diff] = await Promise.all([
                              client.gitStatus(workspace.id),
                              client.gitDiff(workspace.id),
                            ]);
                            return { status, diff };
                          },
                          open: () => setDialog('changes'),
                        }
                      : undefined
                  }
                  events={events.filter((event) => event.session_id === session.id)}
                  historyLoading={historyLoading}
                />
                <Composer
                  key={session.id}
                  attachments={attachments}
                  setAttachments={(value) =>
                    setAttachmentDrafts((current) => ({ ...current, [session.id]: value }))
                  }
                  controls={
                    client && (
                      <>
                        {session.chat_mode !== 'single' && (
                          <TeamComposerAgents
                            session={session}
                            sessions={data?.sessions ?? []}
                            busy={busy || running}
                            configure={(id) => setAgentDialog({ id, handoff: false })}
                          />
                        )}
                        <ChatControls
                          team={session.chat_mode !== 'single'}
                          client={client}
                          session={session}
                          busy={busy}
                          configure={(config) =>
                            void action(async () => {
                              const updated = await client.configureSession(session.id, config);
                              setData((current) =>
                                current
                                  ? {
                                      ...current,
                                      sessions: current.sessions.map((s) =>
                                        s.id === updated.id ? updated : s,
                                      ),
                                    }
                                  : current,
                              );
                            })
                          }
                          settings={() => setAgentDialog({ id: session.id, handoff: false })}
                          handoff={() => setAgentDialog({ id: session.id, handoff: true })}
                        />
                      </>
                    )
                  }
                  ref={composer}
                  draft={draft}
                  setDraft={(value) =>
                    setDrafts((current) => ({ ...current, [session.id]: value }))
                  }
                  running={running}
                  busy={busy}
                  send={send}
                  cancel={cancelCurrent}
                  demo={session.provider === 'mock'}
                  manageUsage={
                    session.provider === 'openai' && client
                      ? () => void action(() => client.openUsage('openai'))
                      : undefined
                  }
                />
              </>
            ) : (
              <div className="dashboard">
                {workspace && workspace.id !== 'chat-scratch' && (
                  <div className="dashboard-project-header">
                    <strong>{workspace.name}</strong>
                    {projectTools}
                  </div>
                )}
                {hero}
                {homeSessions.length > 0 && (
                  <div className="dashboard-heading">
                    <h2>
                      {workspace && workspace.id !== 'chat-scratch'
                        ? `Чаты · ${workspace.name}`
                        : 'Ваши чаты'}
                    </h2>
                    <span>{counted(homeSessions.length, ['чат', 'чата', 'чатов'])}</span>
                  </div>
                )}
                {homeSessions.map((item) => (
                  <button
                    className="dashboard-session"
                    key={item.id}
                    onClick={() => selectSession(item)}
                  >
                    <span className="dashboard-session-icon">
                      <Sparkles size={19} />
                    </span>
                    <span>
                      <strong>{sessionTitle(item.title)}</strong>
                      <small>
                        {data?.workspaces.find((project) => project.id === item.workspace_id)?.name}{' '}
                        · {modeLabels[item.chat_mode]} ·{' '}
                        {item.provider === 'mock' ? 'Локальное демо' : item.model}
                      </small>
                    </span>
                    <span className={`step-pill ${item.status}`}>{statusLabels[item.status]}</span>
                    <ChevronRight size={17} />
                  </button>
                ))}
                {!homeSessions.length && !hero && (
                  <div className="getting-started">
                    <div className="agent-glyph">
                      <FolderPlus size={25} />
                    </div>
                    {workspace ? (
                      <>
                        <h2>В проекте пока нет чатов</h2>
                        <p>Выберите агента и опишите задачу — история сохранится локально.</p>
                        <button
                          className="secondary-button"
                          onClick={() => setDialog('new')}
                          disabled={busy || !client}
                        >
                          <Plus size={16} /> Новый чат
                        </button>
                      </>
                    ) : (
                      <>
                        <h2>Что хотите сделать?</h2>
                        <p>
                          Создайте чат для вопроса или задачи. Папку проекта можно выбрать при
                          создании.
                        </p>
                        <button
                          className="secondary-button"
                          onClick={newSession}
                          disabled={busy || !client}
                        >
                          <Plus size={16} /> Новый чат
                        </button>
                      </>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
          {context && (
            <ContextPanel
              client={client}
              workspace={workspace}
              session={session}
              account={account}
              accountStatus={
                session ? accountState.statuses[session.account_profile_id] : undefined
              }
              providerName={session && providerName(session.provider)}
              openFiles={() => setDialog('files')}
              cancel={cancelCurrent}
              continueSession={() => composer.current?.focus()}
              close={() => setContext(false)}
            />
          )}
        </div>
      </main>

      {dialog === 'files' && client && workspace && (
        <FileManager
          key={workspace.id}
          client={client}
          onDiscuss={
            session
              ? (prompt) => {
                  // Stay within the core's UTF-8 byte limit, including non-Latin text.
                  const draft =
                    prompt.length > 10000
                      ? `${prompt.slice(0, 9800)}\n\n[Список сокращён до размера сообщения. Полные замечания доступны в просмотре файла.]`
                      : prompt;
                  setDrafts((current) => ({ ...current, [session.id]: draft }));
                  setDialog(null);
                  composer.current?.focus();
                }
              : undefined
          }
          workspace={workspace}
          close={() => setDialog(null)}
        />
      )}
      {dialog === 'changes' && client && workspace && (
        <ProjectChanges
          key={workspace.id}
          client={client}
          workspace={workspace}
          close={() => setDialog(null)}
        />
      )}
      {deleting && client && (
        <Dialog title="Удалить чат?" close={() => setDeleting(null)}>
          <p className="dialog-description">
            Чат «{sessionTitle(deleting.title)}», его участники и вся история будут удалены с этого
            компьютера без возможности восстановления. Файлы проекта не изменятся. История на
            стороне ChatGPT или Claude этим не удаляется.
          </p>
          <div className="dialog-footer">
            <button className="secondary-button" autoFocus onClick={() => setDeleting(null)}>
              Отмена
            </button>
            <button
              className="primary-button danger-solid"
              disabled={busy}
              onClick={() => {
                const target = deleting;
                setDeleting(null);
                void action(async () => {
                  await client.deleteChat(target.id);
                  const snapshot = await client.snapshot();
                  setData(snapshot);
                  if (!snapshot.sessions.some((item) => item.id === currentId.current))
                    setSessionId('');
                });
              }}
            >
              <Trash2 size={15} /> Удалить
            </button>
          </div>
        </Dialog>
      )}
      {dialog === 'plugins' && client && data && (
        <ExtensionsDialog
          client={client}
          data={data}
          session={session}
          prepareInstall={async (accountId, skill) => {
            const models = await client.accountModels(accountId);
            const model = models.find((m) => m.is_default) ?? models[0];
            if (!model) throw new Error('Codex не предоставил доступную модель. Обновите аккаунт.');
            const created = await client.createChat({
              workspace_id: session?.workspace_id ?? null,
              mode: 'single',
              agents: [
                {
                  provider: 'openai',
                  account_profile_id: accountId,
                  model: model.id,
                  reasoning_effort: null,
                  permission_profile: 'standard',
                  tools: {},
                  role: '',
                },
              ],
            });
            setData(await client.snapshot());
            selectSession(created);
            setDrafts((current) => ({
              ...current,
              [created.id]: skill
                ? `$skill-installer Установи навык ${skill} из официального каталога openai/skills для текущего изолированного профиля Codex. Сохрани песочницу и подтверждения. Не обращайся к учётным данным. Сообщи результат установки и путь навыка.`
                : '$skill-installer Покажи доступные для установки навыки из официального каталога openai/skills. Пока ничего не устанавливай.',
            }));
            setDialog(null);
          }}
          close={() => setDialog(null)}
          configure={(member) => {
            setDialog(null);
            setAgentDialog({ id: member.id, handoff: false, tools: true });
          }}
        />
      )}
      {dialog === 'new' && data && client && (
        <NewSession
          client={client}
          data={data}
          accountState={accountState}
          initialProject={newChatProject}
          initialMode={newChatMode}
          initialProvider={newSessionProvider}
          openAccounts={() => showSettings('Провайдеры')}
          busy={busy}
          close={() => {
            setDialog(null);
            setNewSessionProvider(undefined);
          }}
          create={(input) =>
            void action(async () => {
              const session = await client.createChat(input);
              setData(await client.snapshot());
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
          appearance={{ theme, textSize, setTheme, setTextSize }}
          initialTab={settingsTab}
          settings={data.settings}
          providers={data.providers}
          accounts={data.accounts}
          accountState={accountState}
          onAccountsChanged={reloadAccounts}
          setUpdateBusy={setUpdateBusy}
          runningSessions={data.sessions.filter((item) => item.status === 'running').length}
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
      {agentDialog && client && data && data.sessions.find((s) => s.id === agentDialog.id) && (
        <ChatSettings
          key={agentDialog.id + String(agentDialog.handoff)}
          client={client}
          data={data}
          session={data.sessions.find((s) => s.id === agentDialog.id)!}
          handoff={agentDialog.handoff}
          initialToolsOpen={agentDialog.tools}
          busy={busy}
          close={() => setAgentDialog(null)}
          save={(config, transition) =>
            void action(async () => {
              if (transition) await client.handoff(agentDialog.id, config);
              else await client.configureSession(agentDialog.id, config);
              setData(await client.snapshot());
              setAgentDialog(null);
            })
          }
        />
      )}
      {planWelcome && client && (
        <PlanWelcome
          close={() => setPlanWelcome('')}
          manageUsage={() => void action(() => client.openUsage('openai'))}
        />
      )}
      {dialog === 'commands' && data && (
        <CommandPalette
          sessions={data.sessions.filter((s) => !s.parent_session_id)}
          close={() => setDialog(null)}
          run={(command) => {
            setDialog(null);
            command();
          }}
          commands={[
            { label: 'Новый чат', icon: <Plus size={16} />, action: newSession },
            { label: 'Добавить папку проекта', icon: <FolderPlus size={16} />, action: addProject },
            {
              label: 'Открыть настройки',
              icon: <Settings2 size={16} />,
              action: () => setDialog('settings'),
            },
            {
              label: 'Показать или скрыть контекст',
              icon: <PanelRightClose size={16} />,
              action: () => setContext((value) => !value),
            },
            ...(running && client
              ? [
                  {
                    label: 'Остановить текущего агента',
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
