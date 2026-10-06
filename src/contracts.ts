export interface Workspace {
  id: string;
  name: string;
  root: string;
  created_at: number;
}
export interface AccountProfile {
  id: string;
  provider: string;
  label: string;
  credential_ref: string | null;
  config_dir: string | null;
  auth_status: string;
  created_at: number;
}
export type SessionStatus = 'idle' | 'running' | 'completed' | 'stopped' | 'failed' | 'interrupted';
export interface Session {
  id: string;
  workspace_id: string;
  provider: string;
  account_profile_id: string;
  model: string;
  title: string;
  status: SessionStatus;
  permission_profile: string;
  working_directory: string;
  provider_session_id: string | null;
  worktree_id: string | null;
  created_at: number;
  updated_at: number;
}
export interface CreateSession {
  workspace_id: string;
  provider: string;
  account_profile_id: string;
  model: string;
  permission_profile: string;
}
export type EventPayload =
  | { type: 'turn_started'; prompt: string }
  | { type: 'assistant_text_delta'; text: string }
  | { type: 'tool_activity'; label: string; detail: string }
  | { type: 'turn_completed' | 'session_stopped' }
  | { type: 'provider_error'; message: string };
export interface AgentEvent {
  sequence: number;
  session_id: string;
  run_id: string;
  timestamp: number;
  payload: EventPayload;
}
export interface ProviderInfo {
  id: string;
  name: string;
  available: boolean;
  detected_path: string | null;
  detail: string;
  models: string[];
}
export interface Settings {
  check_updates_on_start: boolean;
}
export interface Snapshot {
  workspaces: Workspace[];
  sessions: Session[];
  accounts: AccountProfile[];
  providers: ProviderInfo[];
  settings: Settings;
}
export interface GitStatus {
  branch: string;
  files: { path: string; status: string }[];
}
export interface ReleaseCheck {
  current_version: string;
  latest_version: string | null;
  available: boolean;
  notes: string;
  published_at: string | null;
  release_url: string;
  checked_at: number;
}
export interface ClientTransport {
  snapshot(): Promise<Snapshot>;
  chooseFolder(): Promise<string | null>;
  addWorkspace(root: string): Promise<Workspace>;
  createSession(input: CreateSession): Promise<Session>;
  sendMessage(sessionId: string, prompt: string): Promise<string>;
  cancel(sessionId: string): Promise<void>;
  events(sessionId: string, before?: number): Promise<AgentEvent[]>;
  subscribe(onEvent: (event: AgentEvent) => void, onResync: () => void): Promise<() => void>;
  refreshProviders(): Promise<ProviderInfo[]>;
  saveSettings(settings: Settings): Promise<void>;
  gitStatus(workspaceId: string): Promise<GitStatus>;
  gitDiff(workspaceId: string): Promise<string>;
  checkReleases(force: boolean): Promise<ReleaseCheck>;
  openReleases(): Promise<void>;
}
