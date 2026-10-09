// Development-only visual preview, deliberately in memory. Never bundled into a working desktop
// adapter. Simulated OpenAI accounts exist only so the account, limit and approval UI can be built
// without spending real usage; every value here is labelled as a preview in the UI.
import type {
  AccountEvent,
  AccountStatus,
  AgentEvent,
  ApprovalDecision,
  ClientTransport,
  ReviewComment,
  Snapshot,
  LocalMcpServer,
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
      name: 'Claude · официальный Claude Code CLI',
      available: true,
      detected_path: null,
      models: [],
      detail: 'Предпросмотр Claude. Настоящий CLI, вход и файлы доступны в приложении Windows.',
    },
  ],
  settings: { check_updates_on_start: false },
};
let sequence = 0;
const history: AgentEvent[] = [];
const attachmentImages = new Map<string, string>();
const listeners = new Set<(event: AgentEvent) => void>();
const accountListeners = new Set<(event: AccountEvent) => void>();
const cancelled = new Set<string>();
const sandboxReady = new Set<string>();
const approvals = new Map<string, { sessionId: string; resolve: (decision: string) => void }>();
const now = () => Math.floor(Date.now() / 1000);
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function previewStatus(accountId: string): AccountStatus {
  const account = state.accounts.find((item) => item.id === accountId);
  const signedIn = account?.auth_status === 'signed_in';
  return {
    account_id: accountId,
    sandbox:
      signedIn && account?.provider === 'openai'
        ? {
            state: sandboxReady.has(accountId) ? 'ready' : 'not_configured',
            detail: sandboxReady.has(accountId)
              ? 'Демонстрация: песочница готова. Windows не изменялась.'
              : 'Демонстрация: настройте песочницу Windows для этого аккаунта.',
          }
        : null,
    state: signedIn ? 'signed_in' : 'signed_out',
    email: signedIn ? 'preview@example.com' : null,
    plan: signedIn ? (account?.provider === 'anthropic' ? 'max' : 'plus') : null,
    usage:
      signedIn && account?.provider !== 'anthropic'
        ? [
            {
              window_minutes: 300,
              used_percent: 25,
              resets_at: now() + 2 * 3600,
              source: 'preview',
            },
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
    manage_usage_url:
      account?.provider === 'anthropic'
        ? 'https://claude.ai/settings/usage'
        : 'https://chatgpt.com/settings/usage',
    plan_usage_enabled: signedIn && account?.provider === 'openai' ? true : null,
    checked_at: now(),
  };
}

const reviewComments: ReviewComment[] = [];
const localMcp = new Map<string, LocalMcpServer[]>();
export const preview: ClientTransport = {
  accountMcp: async (id) => structuredClone(localMcp.get(id) ?? []),
  saveAccountMcp: async (id, servers) => {
    if (new Set(servers.map((s) => s.name)).size !== servers.length)
      throw new Error('Имена MCP должны быть уникальны.');
    if (
      servers.some(
        (s) => !/^[A-Za-z0-9-]+$/.test(s.name) || !/^[A-Za-z]:[\\/].*\.exe$/i.test(s.command),
      )
    )
      throw new Error('Нужен абсолютный путь к exe.');
    localMcp.set(id, structuredClone(servers));
  },
  attachmentImage: async (sessionId, path) => {
    const src = attachmentImages.get(`${sessionId}:${path}`);
    if (!src) throw new Error('Изображение не найдено в истории предпросмотра.');
    return src;
  },
  reviewDocument: async (_workspaceId, path) => ({
    kind: path.split('.').at(-1) ?? 'txt',
    fingerprint: 'preview-document-v1',
    text: path.endsWith('.csv')
      ? 'Задача,Статус,Исполнитель\nДизайн,Готово,Claude\nПроверка,В работе,GPT'
      : '# Обзор проекта\n\nВыберите абзац или ячейку, чтобы оставить комментарий.\nКомментарии сохраняются в приложении отдельно от документа.',
    parts: [],
  }),
  reviewComments: async (workspaceId, path) =>
    structuredClone(
      reviewComments.filter((c) => c.workspace_id === workspaceId && c.path === path),
    ),
  addReviewComment: async (workspaceId, path, input) => {
    const c = {
      ...input,
      id: crypto.randomUUID(),
      workspace_id: workspaceId,
      path,
      resolved: false,
      created_at: now(),
    };
    reviewComments.push(c);
    return structuredClone(c);
  },
  resolveReviewComment: async (workspaceId, id, resolved) => {
    const c = reviewComments.find((c) => c.id === id && c.workspace_id === workspaceId);
    if (!c) throw new Error('Комментарий не найден');
    c.resolved = resolved;
  },
  listFiles: async () => {
    if (new URLSearchParams(location.search).has('review'))
      return ['обзор.md', 'задачи.csv'].map((name) => ({
        name,
        path: name,
        directory: false,
        blocked: false,
        size: 128,
      }));
    throw new Error('Файлы проекта доступны в приложении для Windows.');
  },
  readFile: async () => {
    if (new URLSearchParams(location.search).has('review'))
      return '# Обзор проекта\n\nПлан работы команды.\n\nКомментарий к этому абзацу.';
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
      reasoning_effort: null,
      tool_policy: '{}',
      parent_session_id: null,
      chat_mode: 'single' as const,
      role: '',
      context_summary: '',
    };
    state.sessions.unshift(session);
    return structuredClone(session);
  },
  createChat: async (input) => {
    let workspaceId = input.workspace_id;
    if (!workspaceId) {
      if (!state.workspaces.some((w) => w.id === 'chat-scratch'))
        state.workspaces.push({
          id: 'chat-scratch',
          name: 'Без проекта',
          root: 'Предпросмотр · файлы обычного чата',
          created_at: now(),
        });
      workspaceId = 'chat-scratch';
    }
    let root = '';
    for (const [index, agent] of input.agents.entries()) {
      const session = await preview.createSession({
        workspace_id: workspaceId,
        provider: agent.provider,
        account_profile_id: agent.account_profile_id,
        model: agent.model,
        permission_profile: agent.permission_profile,
      });
      const stored = state.sessions.find((s) => s.id === session.id)!;
      Object.assign(stored, {
        reasoning_effort: agent.reasoning_effort,
        tool_policy: JSON.stringify(agent.tools),
        role: agent.role,
        chat_mode: index === 0 ? input.mode : 'single',
        parent_session_id: index === 0 ? null : root,
      });
      if (index === 0) root = session.id;
      else stored.title = agent.role;
    }
    return structuredClone(state.sessions.find((s) => s.id === root)!);
  },
  configureSession: async (id, config) => {
    const session = state.sessions.find((s) => s.id === id);
    if (!session || session.status === 'running') throw new Error('Чат занят');
    if (config.account_profile_id !== session.account_profile_id)
      throw new Error('Используйте переход с контекстом');
    Object.assign(session, {
      ...config,
      reasoning_effort: config.reasoning_effort,
      tool_policy: JSON.stringify(config.tools),
    });
    return structuredClone(session);
  },
  handoff: async (id, config) => {
    const session = state.sessions.find((s) => s.id === id);
    if (!session || session.status === 'running') throw new Error('Чат занят');
    Object.assign(session, config, {
      tool_policy: JSON.stringify(config.tools),
      context_summary: 'Предпросмотр: настоящая передача контекста работает в приложении.',
    });
    return crypto.randomUUID();
  },
  sendMessage: async (sessionId, prompt, attachments = []) => {
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
    if (attachments.length)
      emit({
        type: 'user_attachments',
        files: attachments.map((file) => {
          const path = `Предпросмотр/${run}/${file.name}`;
          if (file.mime.startsWith('image/'))
            attachmentImages.set(`${sessionId}:${path}`, `data:${file.mime};base64,${file.data}`);
          return { name: file.name, mime: file.mime, path };
        }),
      });
    emit({
      type: 'agent_configuration',
      provider: session.provider,
      model: session.model,
      account_profile_id: session.account_profile_id,
      reasoning_effort: session.reasoning_effort,
    });
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
          available_decisions: ['allow_once', 'deny'],
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
  deleteChat: async (sessionId) => {
    const chat = state.sessions.find((session) => session.id === sessionId);
    if (!chat || chat.parent_session_id) throw new Error('Удалить можно только чат целиком.');
    const ids = new Set([sessionId]);
    for (const session of state.sessions)
      if (session.parent_session_id && ids.has(session.parent_session_id)) ids.add(session.id);
    if (state.sessions.some((session) => ids.has(session.id) && session.status === 'running'))
      throw new Error('Остановите выполнение в этом чате, затем удалите его.');
    state.sessions = state.sessions.filter((session) => !ids.has(session.id));
    for (let index = history.length - 1; index >= 0; index--)
      if (ids.has(history[index]?.session_id ?? '')) history.splice(index, 1);
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
    if (!['openai', 'anthropic'].includes(provider))
      throw new Error('Для этого провайдера аккаунты не нужны');
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
  setupSandbox: async (accountId) => {
    sandboxReady.add(accountId);
    return { state: 'ready', detail: 'Демонстрация: песочница готова. Windows не изменялась.' };
  },
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
    if (account?.provider === 'anthropic')
      return ['sonnet', 'opus', 'haiku', 'claude-opus-5-5', 'claude-opus-4-6'].map((id) => ({
        id,
        name: `Claude ${id}`,
        description: 'Псевдоним CLI · предпросмотр',
        is_default: id === 'sonnet',
        reasoning_efforts:
          id === 'claude-opus-4-6'
            ? ['low', 'medium', 'high', 'max']
            : ['low', 'medium', 'high', 'xhigh', 'max'],
      }));
    return [
      {
        id: 'preview-model',
        name: 'Модель предпросмотра',
        description: 'Имитация каталога Codex',
        is_default: true,
        reasoning_efforts: ['low', 'medium', 'high'],
      },
    ];
  },
  pluginCatalog: async () => ({
    checked_at: now(),
    errors: [],
    entries: [
      {
        name: 'github',
        provider: 'openai',
        description: 'Работа с репозиториями и задачами · пример каталога.',
        category: 'Developer Tools',
        source_url: 'https://github.com/openai/plugins',
      },
      {
        name: 'code-review',
        provider: 'anthropic',
        description: 'Проверка изменений кода · пример каталога.',
        category: 'development',
        source_url: 'https://github.com/anthropics/claude-plugins-official',
      },
    ],
  }),
  openCatalogSource: async () => {},
  accountExtensions: async (accountId) =>
    state.accounts.find((a) => a.id === accountId)?.provider === 'anthropic'
      ? {
          plugins: [],
          mcp_servers: (localMcp.get(accountId) ?? []).map((s) => ({
            id: s.name,
            name: s.name,
            detail: 'Предпросмотр: локальный stdio',
            enabled: true,
            status: 'configured',
          })),
          skills: [
            {
              id: 'preview-claude-skill',
              name: 'Пример навыка Claude',
              detail: 'Предпросмотр',
              enabled: true,
              status: 'account',
            },
          ],
          errors: [],
        }
      : {
          plugins: [
            {
              id: 'preview-plugin',
              name: 'Пример плагина',
              detail: '1.0.0',
              enabled: true,
              status: null,
            },
          ],
          mcp_servers: [
            {
              id: 'preview-mcp',
              name: 'preview-mcp',
              detail: 'инструментов: 3',
              enabled: true,
              status: 'unsupported',
            },
          ],
          skills: [
            {
              id: 'preview-skill',
              name: 'Пример навыка',
              detail: 'Предпросмотр',
              enabled: true,
              status: 'user',
            },
          ],
          errors: [],
        },
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
  checkUpdate: async () => {
    throw new Error('Подписанные обновления доступны только в приложении для Windows.');
  },
  installUpdate: async () => {
    throw new Error('Обновление устанавливается только в приложении для Windows.');
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
