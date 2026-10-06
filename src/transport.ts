import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type { ClientTransport, AgentEvent } from './contracts';

export const desktop: ClientTransport = {
  snapshot: () => invoke('snapshot'),
  chooseFolder: async () => {
    const path = await open({ directory: true, multiple: false, title: 'Add a project folder' });
    return typeof path === 'string' ? path : null;
  },
  addWorkspace: (root) => invoke('add_workspace', { root }),
  createSession: (input) => invoke('create_session', { input }),
  sendMessage: (sessionId, prompt) => invoke('send_message', { sessionId, prompt }),
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
    'Open this application through Tauri with npm run dev. Browser preview is available only in development with ?preview=1.',
  );
}
