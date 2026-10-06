// Development-only visual preview, deliberately in memory. Never bundled into a working desktop
// adapter. Simulated OpenAI accounts exist only so the account, limit and approval UI can be built
// without spending real usage; every value here is labelled as a preview in the UI.
import type {
  AccountEvent,
  AccountStatus,
  AgentEvent,
  ApprovalDecision,
  ClientTransport,
  Snapshot,
} from './contracts';
import pkg from '../package.json';
import product from '../product.json';
const state: Snapshot = {
  workspaces: [],
  sessions: [],
  accounts: [
    {
      id: 'mock-local',
      provider: 'mock',
      label: 'Локальное демо',
      credential_ref: null,
      config_dir: null,
      auth_status: 'not_required',
      created_at: 0,
    },
  ],
  providers: [
    {
      id: 'mock',
      name: 'Локальное демо',
      available: true,
      detected_path: null,
      models: ['mock-stream-v1'],
      detail: 'Локальный симулятор. Без запросов к ИИ, запуска инструментов и изменений файлов.',
    },
    {
      id: 'openai',
      name: 'OpenAI / Codex',
      available: true,
      detected_path: null,
      models: [],
      detail: 'Предпросмотр: имитация Codex app-server. Настоящий вход работает в приложении.',
    },
    {
      id: 'anthropic',
      name: 'Anthropic / Claude Code',
      available: false,
      detected_path: null,
      models: [],
      detail: 'Адаптер с изолированными профилями — следующий этап.',
    },
  ],
  settings: { check_updates_on_start: false },
};
let sequence = 0;
const history: AgentEvent[] = [];
const listeners = new Set<(event: AgentEvent) => void>();
const accountListeners = new Set<(event: AccountEvent) => void>();
const cancelled = new Set<string>();
const approvals = new Map<string, { sessionId: string; resolve: (decision: string) => void }>();
const now = () => Math.floor(Date.now() / 1000);
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function previewStatus(accountId: string): AccountStatus {
  const account = state.accounts.find((item) => item.id === accountId);
  const signedIn = account?.auth_status === 'signed_in';
  return {
    account_id: accountId,
    state: signedIn ? 'signed_in' : 'signed_out',
    email: signedIn ? 'preview@example.com' : null,
    plan: signedIn ? 'plus' : null,
    usage: signedIn
      ? [
          { window_minutes: 300, used_percent: 25, resets_at: now() + 2 * 3600, source: 'preview' },
          {
            window_minutes: 10080,
            used_percent: 58,
            resets_at: now() + 3 * 86400,
            source: 'preview',
          },
        ]
      : [],
    limit_reached: null,
    credits: null,
    message: signedIn ? 'Предпросмотр: значения лимитов не настоящие' : null,
    manage_usage_url: 'https://chatgpt.com/settings/usage',
    plan_usage_enabled: signedIn ? true : null,
    checked_at: now(),
  };
}

export const preview: ClientTransport = {
  listFiles: async () => {
    throw new Error('Файлы проекта доступны в приложении для Windows.');
  },
  readFile: async () => {
    throw new Error('Редактор файлов доступен в приложении для Windows.');
  },
  saveFile: async () => {
    throw new Error('Редактор файлов доступен в приложении для Windows.');
  },
  createPath: async () => {
    throw new Error('Создание файлов доступно в приложении для Windows.');
  },
  renamePath: async () => {
    throw new Error('Переименование файлов доступно в приложении для Windows.');
  },
  deletePath: async () => {
    throw new Error('Удаление файлов доступно в приложении для Windows.');
  },
  openProject: async () => {
    throw new Error('Проводник и PowerShell доступны в приложении для Windows.');
  },
  snapshot: async () => structuredClone(state),
  chooseFolder: async () => 'D:\\Preview\\sample-project',
  addWorkspace: async (root) => {
    const existing = state.workspaces.find((workspace) => workspace.root === root);
    if (existing) return existing;
    const workspace = { id: crypto.randomUUID(), root, name: 'sample-project', created_at: now() };
    state.workspaces.push(workspace);
    return workspace;
  },
  createSession: async (input) => {
    const workspace = state.workspaces.find((workspace) => workspace.id === input.workspace_id);
    const account = state.accounts.find((account) => account.id === input.account_profile_id);
    if (!workspace || !account || account.provider !== input.provider)
      throw new Error('Выберите доступный проект, провайдера и аккаунт');
    const session = {
      ...input,
      id: crypto.randomUUID(),
      title: 'New session',
      status: 'idle' as const,
      working_directory: workspace.root,
      worktree_id: null,
      provider_session_id: null,
      created_at: now(),
      updated_at: now(),
    };
    state.sessions.unshift(session);
    return structuredClone(session);
  },
  sendMessage: async (sessionId, prompt) => {
    const session = state.sessions.find((session) => session.id === sessionId);
    if (!session || session.status === 'running') throw new Error('Сессия недоступна или занята');
    session.status = 'running';
    session.title = prompt.slice(0, 64);
    cancelled.delete(sessionId);
    const run = crypto.randomUUID();
    const emit = (payload: AgentEvent['payload']) => {
      const event = {
        sequence: ++sequence,
        session_id: sessionId,
        run_id: run,
        timestamp: now(),
        payload,
      };
      history.push(event);
      listeners.forEach((listener) => listener(event));
    };
    emit({ type: 'turn_started', prompt });
    void (async () => {
      // Preview-only sample rows for UI work; nothing here touches the disk.
      for (const [label, detail] of [
        ['Демонстрация планирования', 'Файлы не читались, команды не выполнялись.'],
        ['Просмотр структуры', './ · пример для предпросмотра'],
        ['Чтение файла', 'package.json · пример для предпросмотра'],
      ] as const) {
        await wait(120);
        if (cancelled.has(sessionId)) break;
        emit({ type: 'tool_activity', label, detail });
      }
      if (session.provider === 'openai' && !cancelled.has(sessionId)) {
        const id = crypto.randomUUID();
        const decision = new Promise<string>((resolve) =>
          approvals.set(id, { sessionId, resolve }),
        );
        emit({
          type: 'approval_requested',
          id,
          kind: 'command',
          title: 'Codex хочет выполнить команду',
          detail: 'npm test',
          cwd: session.working_directory,
          reason: 'Предпросмотр: запуск тестов проекта',
        });
        const answer = await Promise.race([
          decision,
          (async () => {
            while (!cancelled.has(sessionId)) await wait(100);
            return 'expired';
          })(),
        ]);
        approvals.delete(id);
        emit({ type: 'approval_resolved', id, decision: answer });
        if (answer === 'allow_once' || answer === 'allow_session')
          emit({ type: 'tool_activity', label: 'Команда', detail: 'npm test · предпросмотр' });
      }
      const text =
        'Это ответ локального демо. Каждая сессия привязана к своему провайдеру, аккаунту и профилю разрешений. Приложение хранит историю в SQLite.\n\nПотоковый вывод и остановка работают независимо в каждой сессии. Этот предпросмотр хранит данные только в памяти.';
      for (const word of text.split(/(?<= )/u)) {
        await wait(35);
        if (cancelled.has(sessionId)) break;
        emit({ type: 'assistant_text_delta', text: word });
      }
      session.status = cancelled.has(sessionId) ? 'stopped' : 'completed';
      emit({ type: cancelled.has(sessionId) ? 'session_stopped' : 'turn_completed' });
    })();
    return run;
  },
  cancel: async (sessionId) => {
    cancelled.add(sessionId);
  },
  events: async (sessionId, before) =>
    history
      .filter((event) => event.session_id === sessionId && event.sequence < (before ?? Infinity))
      .slice(-300),
  subscribe: async (onEvent) => {
    listeners.add(onEvent);
    return () => {
      listeners.delete(onEvent);
    };
  },
  refreshProviders: async () => state.providers,
  addAccount: async (provider, label) => {
    if (provider !== 'openai') throw new Error('Для этого провайдера аккаунты не нужны');
    if (!label.trim()) throw new Error('Название аккаунта должно содержать от 1 до 40 символов');
    const account = {
      id: crypto.randomUUID(),
      provider,
      label: label.trim(),
      credential_ref: null,
      config_dir: `preview\\providers\\${provider}`,
      auth_status: 'signed_out',
      created_at: now(),
    };
    state.accounts.push(account);
    return structuredClone(account);
  },
  renameAccount: async (accountId, label) => {
    const account = state.accounts.find((item) => item.id === accountId);
    if (!account) throw new Error('Запись не найдена');
    account.label = label.trim();
    return structuredClone(account);
  },
  removeAccount: async (accountId) => {
    if (state.sessions.some((session) => session.account_profile_id === accountId))
      throw new Error('Аккаунт используется сессиями. Выйдите из него, чтобы отключить доступ.');
    state.accounts = state.accounts.filter((item) => item.id !== accountId);
  },
  accountStatus: async (accountId) => previewStatus(accountId),
  accountLogin: async (accountId) => {
    // Preview never opens a provider page; it simulates the completion notification.
    setTimeout(() => {
      const account = state.accounts.find((item) => item.id === accountId);
      if (account) account.auth_status = 'signed_in';
      accountListeners.forEach((listener) =>
        listener({ account_id: accountId, kind: 'login_completed', status: null, message: null }),
      );
    }, 400);
  },
  accountLogout: async (accountId) => {
    const account = state.accounts.find((item) => item.id === accountId);
    if (account) account.auth_status = 'signed_out';
  },
  accountModels: async (accountId) => {
    const account = state.accounts.find((item) => item.id === accountId);
    if (account?.provider === 'mock')
      return [{ id: 'mock-stream-v1', name: 'mock-stream-v1', description: '', is_default: true }];
    return [
      {
        id: 'preview-model',
        name: 'Модель предпросмотра',
        description: 'Имитация каталога Codex',
        is_default: true,
      },
    ];
  },
  accountExtensions: async () => ({
    plugins: [{ name: 'Пример плагина', detail: '1.0.0', enabled: true, status: null }],
    mcp_servers: [
      { name: 'preview-mcp', detail: 'инструментов: 3', enabled: true, status: 'unsupported' },
    ],
    skills: [{ name: 'Пример навыка', detail: 'Предпросмотр', enabled: true, status: 'user' }],
    errors: [],
  }),
  subscribeAccounts: async (onEvent) => {
    accountListeners.add(onEvent);
    return () => {
      accountListeners.delete(onEvent);
    };
  },
  resolveApproval: async (sessionId, approvalId, decision: ApprovalDecision) => {
    const pending = approvals.get(approvalId);
    if (!pending || pending.sessionId !== sessionId) throw new Error('Запрос уже закрыт');
    pending.resolve(decision);
  },
  openUsage: async () => {
    window.open('https://chatgpt.com/settings/usage', '_blank', 'noopener,noreferrer');
  },
  saveSettings: async (settings) => {
    state.settings = settings;
  },
  gitStatus: async () => {
    throw new Error('Git доступен в приложении. Предпросмотр не имеет доступа к файловой системе.');
  },
  gitDiff: async () => {
    throw new Error('Просмотр изменений доступен в приложении.');
  },
  checkReleases: async () => ({
    current_version: pkg.version,
    latest_version: null,
    available: false,
    notes: 'Проверка обновлений работает в приложении. Предпросмотр не обращается к GitHub.',
    published_at: null,
    release_url: `https://github.com/${product.repository}/releases`,
    checked_at: now(),
  }),
  openReleases: async () => {
    window.open(
      `https://github.com/${product.repository}/releases`,
      '_blank',
      'noopener,noreferrer',
    );
  },
};
