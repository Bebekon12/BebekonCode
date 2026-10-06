import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ChevronRight,
  FolderPlus,
  PanelRightClose,
  Plus,
  Settings2,
  ShieldCheck,
  Sparkles,
  Square,
  X,
} from 'lucide-react';
import type { AgentEvent, ClientTransport, ReleaseCheck, Session, Snapshot } from './contracts';
import { browserPreview, getTransport } from './transport';
import { eventStatus, mergeEvents } from './timeline';
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
import { ChatControls } from './components/ChatControls';
import { ChatSettings } from './components/ChatSettings';
import { modeLabels } from './chat';
import {
  accountLabel,
  counted,
  errorText,
  permissionLabel,
  sessionTitle,
  statusLabels,
} from './locale';

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
  const [dialog, setDialog] = useState<'new' | 'settings' | 'commands' | 'files' | null>(null);
  const [context, setContext] = useState(window.innerWidth > 1100);
  const [settingsTab, setSettingsTab] = useState('Основные');
  const [newSessionProvider, setNewSessionProvider] = useState<string>();
  const [agentDialog, setAgentDialog] = useState<{ id: string; handoff: boolean } | null>(null);
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
    setHistoryPage(false);
    if (!client || !sessionId) return;
    let alive = true;
    void client
      .events(sessionId)
      .then((history) => {
        if (alive) setEvents((current) => mergeEvents(current, history));
      })
      .catch((error) => {
        if (alive) setError(errorText(error));
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
    setDialog('new');
  };
  const send = () => {
    if (
      !client ||
      !session ||
      !draft.trim() ||
      busy ||
      running ||
      data?.sessions.some((s) => s.id === session.parent_session_id && s.status === 'running')
    )
      return;
    const prompt = draft.trim();
    void action(async () => {
      await client.sendMessage(session.id, prompt);
      setDrafts((current) => ({ ...current, [session.id]: '' }));
    });
  };
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (dialog === 'files' || updateBusy) return; // File editor owns its unsaved-change navigation.
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setDialog((current) => (current === 'commands' ? null : 'commands'));
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        if (dataRef.current) setDialog('new');
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [dialog, updateBusy]);

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
        addProject={addProject}
        goHome={() => setSessionId('')}
        showSettings={showSettings}
        openFiles={() => client && workspace && setDialog('files')}
        selectProject={selectProject}
        selectSession={selectSession}
        startSession={(id) => {
          setProjectId(id);
          setDialog('new');
        }}
        preview={browserPreview}
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
                  <strong>{sessionTitle(session.title)}</strong>
                  <span>
                    {modeLabels[session.chat_mode]}
                    {session.workspace_id === 'chat-scratch' ? ' · без проекта' : ''} ·{' '}
                    {accountLabel(account)}
                  </span>
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
                {session.chat_mode !== 'single' && session.chat_mode !== 'task' && (
                  <div className="chat-branches" aria-label="Участники и подзадачи">
                    {data?.sessions
                      .filter((s) => s.parent_session_id === session.id)
                      .map((s) => (
                        <span className="branch-chip" key={s.id}>
                          <button onClick={() => selectSession(s)} title={s.model}>
                            {s.chat_mode === 'task' ? 'Подзадача' : 'Участник'}:{' '}
                            {s.title || s.role || s.model}
                            <span className={`status-dot ${s.status}`} />
                          </button>
                          {s.chat_mode !== 'task' && (
                            <button
                              className="icon-button"
                              aria-label={`Настроить ${s.role || s.model}`}
                              disabled={running}
                              onClick={() => setAgentDialog({ id: s.id, handoff: false })}
                            >
                              <Settings2 size={13} />
                            </button>
                          )}
                        </span>
                      ))}
                  </div>
                )}
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
                    Просмотр истории · вернуться к последним событиям
                  </button>
                )}
                <Timeline
                  session={session}
                  providerName={providerName(session.provider)}
                  account={accountLabel(account)}
                  permissions={permissionLabel(session.permission_profile)}
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
                <Composer
                  controls={
                    client && (
                      <ChatControls
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
                {hero}
                <div className="dashboard-heading">
                  <h2>
                    {workspace && workspace.id !== 'chat-scratch'
                      ? `Чаты · ${workspace.name}`
                      : 'Ваши чаты'}
                  </h2>
                  <span>{counted(homeSessions.length, ['чат', 'чата', 'чатов'])}</span>
                </div>
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
                        · {modeLabels[item.chat_mode]} · {item.model}
                      </small>
                    </span>
                    <span className={`step-pill ${item.status}`}>{statusLabels[item.status]}</span>
                    <ChevronRight size={17} />
                  </button>
                ))}
                {!homeSessions.length && (
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
                <div className="dashboard-note">
                  <ShieldCheck size={18} />
                  <div>
                    <strong>Подключите свой аккаунт и начните разговор</strong>
                    <p>
                      Выберите обычный чат, команду или авторазбиение. Модель, рассуждение и доступ
                      настраиваются в чате. История сохраняется на компьютере.
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
        <footer className="statusbar">
          <span>
            <ShieldCheck size={11} />{' '}
            {browserPreview ? 'Предпросмотр в памяти' : 'Локальная рабочая область'}
          </span>
          <span>
            {counted(data?.sessions.filter((session) => session.status === 'running').length ?? 0, [
              'активная сессия',
              'активные сессии',
              'активных сессий',
            ])}{' '}
            <span className="statusbar-separator">·</span> Без удалённого доступа
          </span>
        </footer>
      </main>

      {dialog === 'files' && client && workspace && (
        <FileManager
          key={workspace.id}
          client={client}
          workspace={workspace}
          close={() => setDialog(null)}
        />
      )}
      {dialog === 'new' && data && client && (
        <NewSession
          client={client}
          data={data}
          accountState={accountState}
          initialProject={projectId}
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
