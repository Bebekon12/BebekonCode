import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { checkUpdate, installUpdate } from './updates';
import type { AccountEvent, AgentEvent, ClientTransport } from './contracts';

export const desktop: ClientTransport = {
  taskQueue: () => invoke('task_queue'),
  configureConversation: (sessionId, settings) =>
    invoke('configure_conversation', { sessionId, settings }),
  steerMessage: (sessionId, expectedRun, prompt, attachments) =>
    invoke('steer_message', { sessionId, expectedRun, prompt, attachments }),
  enqueueMessage: (sessionId, prompt, attachments) =>
    invoke('enqueue_message', { sessionId, prompt, attachments }),
  editQueuedTask: (id, prompt, direction) => invoke('edit_queued_task', { id, prompt, direction }),
  enqueueTask: (sessionId, prompt) => invoke('enqueue_task', { sessionId, prompt }),
  changeQueuedTask: (id, resume) => invoke('change_queued_task', { id, resume }),
  checkpoints: (sessionId) => invoke('checkpoints', { sessionId }),
  createCheckpoint: (sessionId, label) => invoke('create_checkpoint', { sessionId, label }),
  checkpointDiff: (sessionId, id) => invoke('checkpoint_diff', { sessionId, id }),
  restoreCheckpointFile: (sessionId, id, path, expected) =>
    invoke('restore_checkpoint_file', { sessionId, id, path, expected }),
  deleteCheckpoint: (sessionId, id) => invoke('delete_checkpoint', { sessionId, id }),
  branchChat: (sessionId, isolated) => invoke('branch_chat', { sessionId, isolated }),
  projectFileRules: (sessionId) => invoke('project_file_rules', { sessionId }),
  saveProjectFileRules: (sessionId, rules) =>
    invoke('save_project_file_rules', { sessionId, rules }),
  computerStatus: () => invoke('computer_status'),
  installUnityAdapter: (root) => invoke('install_unity_adapter', { root }),
  computerWindows: () => invoke('computer_windows'),
  configureComputer: (sessionId, windows) => invoke('configure_computer', { sessionId, windows }),
  stopComputer: () => invoke('stop_computer'),
  approveComputer: (id, allow, sensitive) => invoke('approve_computer', { id, allow, sensitive }),
  accountMcp: (accountId) => invoke('account_mcp', { accountId }),
  saveAccountMcp: (accountId, servers) => invoke('save_account_mcp', { accountId, servers }),
  attachmentImage: (sessionId, path) => invoke('attachment_image', { sessionId, path }),
  reviewDocument: (workspaceId, path) => invoke('review_document', { workspaceId, path }),
  reviewComments: (workspaceId, path) => invoke('review_comments', { workspaceId, path }),
  addReviewComment: (workspaceId, path, input) =>
    invoke('add_review_comment', { workspaceId, path, input }),
  resolveReviewComment: (workspaceId, id, resolved) =>
    invoke('resolve_review_comment', { workspaceId, id, resolved }),
  listFiles: (workspaceId, path) =>
    invoke('file_operation', { workspaceId, operation: { kind: 'list', path } }),
  readFile: (workspaceId, path) =>
    invoke('file_operation', { workspaceId, operation: { kind: 'read', path } }),
  saveFile: (workspaceId, path, expected, content) =>
    invoke('file_operation', { workspaceId, operation: { kind: 'save', path, expected, content } }),
  createPath: (workspaceId, path, directory) =>
    invoke('file_operation', { workspaceId, operation: { kind: 'create', path, directory } }),
  renamePath: (workspaceId, path, destination) =>
    invoke('file_operation', { workspaceId, operation: { kind: 'rename', path, destination } }),
  deletePath: (workspaceId, path) =>
    invoke('file_operation', { workspaceId, operation: { kind: 'delete', path } }),
  openProject: (workspaceId, terminal) => invoke('open_project', { workspaceId, terminal }),
  snapshot: () => invoke('snapshot'),
  chooseFolder: async () => {
    const path = await open({ directory: true, multiple: false, title: 'Добавить папку проекта' });
    return typeof path === 'string' ? path : null;
  },
  addWorkspace: (root) => invoke('add_workspace', { root }),
  createSession: (input) => invoke('create_session', { input }),
  createChat: (input) => invoke('create_chat', { input }),
  configureSession: (sessionId, config) => invoke('configure_session', { sessionId, config }),
  handoff: (sessionId, config) => invoke('handoff_session', { sessionId, config }),
  sendMessage: (sessionId, prompt, attachments = []) =>
    invoke('send_message', { sessionId, prompt, attachments }),
  cancel: (sessionId) => invoke('cancel_session', { sessionId }),
  deleteChat: (sessionId) => invoke('delete_chat', { sessionId }),
  events: (sessionId, before) => invoke('session_events', { sessionId, before: before ?? null }),
  subscribe: async (onEvent, onResync) => {
    const unlistenEvent = await listen<AgentEvent>('agent-event', (event) =>
      onEvent(event.payload),
    );
    try {
      const unlistenResync = await listen('agent-resync', onResync);
      return () => {
        unlistenEvent();
        unlistenResync();
      };
    } catch (error) {
      unlistenEvent();
      throw error;
    }
  },
  refreshProviders: () => invoke('refresh_providers'),
  addAccount: (provider, label) => invoke('add_account', { provider, label }),
  renameAccount: (accountId, label) => invoke('rename_account', { accountId, label }),
  removeAccount: (accountId) => invoke('remove_account', { accountId }),
  accountStatus: (accountId) => invoke('account_status', { accountId }),
  setupSandbox: (accountId) => invoke('setup_sandbox', { accountId }),
  accountLogin: (accountId, forUsage = false) => invoke('account_login', { accountId, forUsage }),
  accountLogout: (accountId) => invoke('account_logout', { accountId }),
  accountModels: (accountId) => invoke('account_models', { accountId }),
  accountExtensions: (accountId) => invoke('account_extensions', { accountId }),
  pluginCatalog: () => invoke('plugin_catalog'),
  accountPlugins: (accountId) => invoke('account_plugins', { accountId }),
  changeAccountPlugin: (accountId, pluginId, install) =>
    invoke('change_account_plugin', { accountId, pluginId, install }),
  addAccountPluginSource: (accountId, source) =>
    invoke('add_account_plugin_source', { accountId, source }),
  openCatalogSource: (provider) => invoke('open_catalog_source', { provider }),
  subscribeAccounts: (onEvent) =>
    listen<AccountEvent>('account-event', (event) => onEvent(event.payload)),
  resolveApproval: (sessionId, approvalId, decision) =>
    invoke('resolve_approval', { sessionId, approvalId, decision }),
  openUsage: (provider) => invoke('open_usage', { provider }),
  checkUpdate,
  installUpdate,
  saveSettings: (settings) => invoke('save_settings', { settings }),
  gitStatus: (workspaceId) => invoke('git_status', { workspaceId }),
  gitDiff: (workspaceId) => invoke('git_diff', { workspaceId }),
  checkReleases: (force) => invoke('check_releases', { force }),
  openReleases: () => invoke('open_releases'),
};

export const browserPreview =
  !isTauri() && import.meta.env.DEV && new URLSearchParams(location.search).has('preview');
export async function getTransport(): Promise<ClientTransport> {
  if (isTauri()) return desktop;
  if (browserPreview) return (await import('./preview')).preview;
  throw new Error(
    'Запустите установленное приложение BebekonCode. Для разработки используйте npm run dev; браузерный предпросмотр доступен только с ?preview=1.',
  );
}
