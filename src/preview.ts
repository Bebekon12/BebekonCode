// Development-only visual preview, deliberately in memory. Never bundled into a working desktop adapter.
import type { AgentEvent, ClientTransport, Snapshot } from './contracts';
import pkg from '../package.json';
import product from '../product.json';
const state: Snapshot = {
  workspaces: [],
  sessions: [],
  accounts: [
    {
      id: 'mock-local',
      provider: 'mock',
      label: 'Local demo',
      credential_ref: null,
      config_dir: null,
      auth_status: 'not_required',
      created_at: 0,
    },
  ],
  providers: [
    {
      id: 'mock',
      name: 'Local demo',
      available: true,
      detected_path: null,
      models: ['mock-stream-v1'],
      detail: 'Deterministic local simulator. No AI requests, tools, or file changes.',
    },
    {
      id: 'openai',
      name: 'OpenAI / Codex',
      available: false,
      detected_path: null,
      models: [],
      detail: 'Official adapter is planned. No authentication is connected.',
    },
    {
      id: 'anthropic',
      name: 'Anthropic / Claude Code',
      available: false,
      detected_path: null,
      models: [],
      detail: 'Official adapter is planned. No authentication is connected.',
    },
  ],
  settings: { check_updates_on_start: false },
};
let sequence = 0;
const history: AgentEvent[] = [];
const listeners = new Set<(event: AgentEvent) => void>();
const cancelled = new Set<string>();
const now = () => Math.floor(Date.now() / 1000);
export const preview: ClientTransport = {
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
    if (!workspace || input.provider !== 'mock')
      throw new Error('Select an available project and provider');
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
    if (!session || session.status === 'running') throw new Error('Session is unavailable or busy');
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
      emit({
        type: 'tool_activity',
        label: 'Simulated planning step',
        detail: 'No files were read or commands executed. This is a browser preview.',
      });
      const text =
        'This is a local demo response. Each session keeps its provider, account and permission profile. The desktop core stores its timeline in SQLite.\n\nStreaming and cancellation work independently for each session. This browser preview uses memory only; real provider adapters are planned for the next milestone.';
      for (const word of text.split(/(?<= )/u)) {
        await new Promise((resolve) => setTimeout(resolve, 35));
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
  saveSettings: async (settings) => {
    state.settings = settings;
  },
  gitStatus: async () => {
    throw new Error('Git is available in the desktop app. Preview has no filesystem access.');
  },
  gitDiff: async () => {
    throw new Error('Diff is available in the desktop app.');
  },
  checkReleases: async () => ({
    current_version: pkg.version,
    latest_version: null,
    available: false,
    notes: 'Release checks run in the desktop app. Browser preview does not contact GitHub.',
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
