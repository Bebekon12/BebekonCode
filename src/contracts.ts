export interface Workspace {
  id: string;
  name: string;
  root: string;
  created_at: number;
}
export interface Attachment {
  name: string;
  mime: string;
  data: string;
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
  reasoning_effort: string | null;
  tool_policy: string;
  parent_session_id: string | null;
  chat_mode: 'single' | 'team' | 'auto' | 'task';
  role: string;
  context_summary: string;
}
export interface ToolPolicy {
  plugins?: string[] | null;
  mcp_servers?: string[] | null;
  skills?: string[] | null;
}
export interface AgentConfig {
  provider: string;
  account_profile_id: string;
  model: string;
  reasoning_effort: string | null;
  permission_profile: string;
  tools: ToolPolicy;
  role: string;
}
export interface CreateChat {
  workspace_id: string | null;
  mode: 'single' | 'team' | 'auto';
  agents: AgentConfig[];
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
  | { type: 'progress_delta'; text: string }
  | { type: 'model_resolved'; model: string }
  | { type: 'user_attachments'; files: { name: string; path: string; mime: string }[] }
  | {
      type: 'team_message';
      session_id: string;
      stage_id?: string;
      title: string;
      text: string;
      model?: string | null;
      reasoning_effort?: string | null;
    }
  | {
      type: 'agent_configuration';
      provider: string;
      model: string;
      account_profile_id: string;
      reasoning_effort: string | null;
    }
  | { type: 'tool_activity'; label: string; detail: string }
  | {
      type: 'approval_requested';
      id: string;
      kind: string;
      title: string;
      detail: string;
      cwd: string | null;
      reason: string | null;
      available_decisions?: ApprovalDecision[] | null;
    }
  | { type: 'approval_resolved'; id: string; decision: string }
  | { type: 'turn_completed' | 'session_stopped' }
  | { type: 'provider_error'; message: string; kind?: string | null };
export type ApprovalDecision = 'allow_once' | 'allow_session' | 'deny';
export interface UsageWindow {
  label?: string | null;
  window_minutes: number | null;
  used_percent: number;
  resets_at: number | null;
  source: string;
}
export interface AccountStatus {
  sandbox?: SandboxStatus | null;
  usage_detail?: string | null;
  usage_error?: string | null;
  auth_mode?: string | null;
  account_id: string;
  state: 'signed_in' | 'signed_out' | 'not_required' | 'unavailable' | 'error';
  email: string | null;
  plan: string | null;
  usage: UsageWindow[];
  limit_reached: string | null;
  credits: string | null;
  message: string | null;
  manage_usage_url: string | null;
  plan_usage_enabled: boolean | null;
  checked_at: number;
}
export interface SandboxStatus {
  state: 'ready' | 'not_configured' | 'update_required' | 'setting_up' | 'unavailable' | 'error';
  detail: string;
}
export interface AccountEvent {
  account_id: string;
  kind: 'login_completed' | 'login_failed' | 'updated' | 'usage' | 'notice';
  status: AccountStatus | null;
  message: string | null;
}
export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  is_default: boolean;
  reasoning_efforts?: string[];
  default_reasoning_effort?: string | null;
}
export interface ExtensionItem {
  id?: string;
  name: string;
  detail: string | null;
  enabled: boolean;
  status: string | null;
}
export interface Extensions {
  plugins: ExtensionItem[];
  mcp_servers: ExtensionItem[];
  skills: ExtensionItem[];
  errors: string[];
}
export interface CatalogEntry {
  name: string;
  provider: string;
  description: string;
  category: string;
  source_url: string;
}
export interface PluginCatalog {
  entries: CatalogEntry[];
  errors: string[];
  checked_at: number;
}
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
export interface AppUpdate {
  version: string;
  currentVersion: string;
  notes: string;
  date: string | null;
}
export interface UpdateProgress {
  phase: 'downloading' | 'verifying' | 'installing';
  downloaded: number;
  total: number | null;
}
export interface ClientTransport {
  accountMcp(accountId: string): Promise<LocalMcpServer[]>;
  saveAccountMcp(accountId: string, servers: LocalMcpServer[]): Promise<void>;
  attachmentImage(sessionId: string, path: string): Promise<string>;
  reviewDocument(workspaceId: string, path: string): Promise<ReviewDocument>;
  reviewComments(workspaceId: string, path: string): Promise<ReviewComment[]>;
  addReviewComment(workspaceId: string, path: string, input: NewComment): Promise<ReviewComment>;
  resolveReviewComment(workspaceId: string, id: string, resolved: boolean): Promise<void>;
  listFiles(workspaceId: string, path: string): Promise<FileEntry[]>;
  readFile(workspaceId: string, path: string): Promise<string>;
  saveFile(workspaceId: string, path: string, expected: string, content: string): Promise<void>;
  createPath(workspaceId: string, path: string, directory: boolean): Promise<void>;
  renamePath(workspaceId: string, path: string, destination: string): Promise<void>;
  deletePath(workspaceId: string, path: string): Promise<void>;
  openProject(workspaceId: string, terminal: boolean): Promise<void>;
  snapshot(): Promise<Snapshot>;
  chooseFolder(): Promise<string | null>;
  addWorkspace(root: string): Promise<Workspace>;
  createSession(input: CreateSession): Promise<Session>;
  createChat(input: CreateChat): Promise<Session>;
  configureSession(sessionId: string, config: AgentConfig): Promise<Session>;
  handoff(sessionId: string, config: AgentConfig): Promise<string>;
  sendMessage(sessionId: string, prompt: string, attachments?: Attachment[]): Promise<string>;
  cancel(sessionId: string): Promise<void>;
  /** Deletes a stopped top-level chat with its members and local history; project files stay. */
  deleteChat(sessionId: string): Promise<void>;
  events(sessionId: string, before?: number): Promise<AgentEvent[]>;
  subscribe(onEvent: (event: AgentEvent) => void, onResync: () => void): Promise<() => void>;
  refreshProviders(): Promise<ProviderInfo[]>;
  addAccount(provider: string, label: string): Promise<AccountProfile>;
  renameAccount(accountId: string, label: string): Promise<AccountProfile>;
  removeAccount(accountId: string): Promise<void>;
  accountStatus(accountId: string): Promise<AccountStatus>;
  setupSandbox(accountId: string): Promise<SandboxStatus>;
  accountLogin(accountId: string, forUsage?: boolean): Promise<void>;
  accountLogout(accountId: string): Promise<void>;
  accountModels(accountId: string): Promise<ModelInfo[]>;
  accountExtensions(accountId: string): Promise<Extensions>;
  pluginCatalog(): Promise<PluginCatalog>;
  openCatalogSource(provider: string): Promise<void>;
  subscribeAccounts(onEvent: (event: AccountEvent) => void): Promise<() => void>;
  resolveApproval(sessionId: string, approvalId: string, decision: ApprovalDecision): Promise<void>;
  openUsage(provider: string): Promise<void>;
  /** Signed in-app update. `null` means the installed version is current. */
  checkUpdate(): Promise<AppUpdate | null>;
  installUpdate(version: string, onProgress: (progress: UpdateProgress) => void): Promise<void>;
  saveSettings(settings: Settings): Promise<void>;
  gitStatus(workspaceId: string): Promise<GitStatus>;
  gitDiff(workspaceId: string): Promise<string>;
  checkReleases(force: boolean): Promise<ReleaseCheck>;
  openReleases(): Promise<void>;
}

export interface LocalMcpServer {
  name: string;
  command: string;
  args: string[];
}

export interface FileEntry {
  name: string;
  path: string;
  directory: boolean;
  blocked: boolean;
  size: number;
}

export interface ReviewDocument {
  kind: string;
  fingerprint: string;
  text: string | null;
  parts: { name: string; content: string }[];
}
export interface ReviewComment {
  id: string;
  workspace_id: string;
  path: string;
  fingerprint: string;
  anchor: string;
  quote: string;
  body: string;
  resolved: boolean;
  created_at: number;
}
export interface NewComment {
  fingerprint: string;
  anchor: string;
  quote: string;
  body: string;
}
