import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { checkUpdate, installUpdate } from './updates';
import type { AccountEvent, AgentEvent, ClientTransport } from './contracts';

export const desktop: ClientTransport = {
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
  accountLogin: (accountId) => invoke('account_login', { accountId }),
  accountLogout: (accountId) => invoke('account_logout', { accountId }),
  accountModels: (accountId) => invoke('account_models', { accountId }),
  accountExtensions: (accountId) => invoke('account_extensions', { accountId }),
  pluginCatalog: () => invoke('plugin_catalog'),
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
