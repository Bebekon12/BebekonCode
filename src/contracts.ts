export interface Workspace {
  id: string;
  name: string;
  root: string;
  created_at: number;
}
export interface AttachedFile {
  name: string;
  mime: string;
  path: string;
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
  conversation_settings?: string;
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
  fast_mode?: boolean;
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
  fast_mode?: boolean;
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
  | {
      type: 'context_usage';
      used_tokens: number | null;
      window_tokens: number | null;
      source: string;
    }
  | { type: 'context_compacted' }
  | { type: 'user_followup'; prompt: string; files: AttachedFile[] }
  | { type: 'queue_changed' }
  | { type: 'provider_diagnostic'; report: ProviderRunDiagnostic }
  | { type: 'turn_started'; prompt: string }
  | { type: 'assistant_text_delta'; text: string }
  | { type: 'progress_delta'; text: string }
  | { type: 'model_resolved'; model: string }
  | { type: 'run_changes'; files: ChangedFile[]; limited: boolean }
  | {
      type: 'provider_run_started';
      provider: string;
      model: string;
      purpose: string;
      resumed: boolean;
      prompt_bytes: number;
    }
  | {
      type: 'provider_usage';
      provider: string;
      model_requests: number | null;
      input_tokens: number | null;
      output_tokens: number | null;
      cache_read_tokens: number | null;
      cache_creation_tokens: number | null;
      reasoning_tokens?: number | null;
      incomplete?: boolean;
    }
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
      fast_mode?: boolean;
    }
  | { type: 'tool_activity'; label: string; detail: string }
  | {
      type: 'tool_started';
      id: string;
      tool: string;
      kind: ToolKind;
      summary: string;
      added?: number | null;
      removed?: number | null;
    }
  | { type: 'tool_finished'; id: string; failed: boolean }
  | { type: 'todo_update'; replace: boolean; items: TodoItem[] }
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
export type ToolKind =
  'command' | 'read' | 'edit' | 'create' | 'write' | 'search' | 'web' | 'agent' | 'mcp' | 'other';
export interface TodoItem {
  id?: string | null;
  content?: string | null;
  status?: 'pending' | 'in_progress' | 'completed' | 'deleted' | null;
}
export interface ProviderRunDiagnostic {
  request_id: string;
  provider: string;
  account_profile_id: string;
  session_id: string;
  provider_session_id: string | null;
  requested_model: string;
  resolved_model: string | null;
  effort: string | null;
  permission_profile: string;
  fast_mode: boolean;
  resumed: boolean;
  prompt_bytes: number;
  elapsed_ms: number;
  tool_requests: number;
  approval_requests: number;
  reported_retries: number;
  status: string;
  error_kind: string | null;
  usage_reported: boolean;
  usage_incomplete?: boolean;
  subagent_events?: number;
  model_requests: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  cache_read_tokens: number | null;
  cache_creation_tokens: number | null;
}
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
  fast_mode_available?: boolean;
}

export interface ChangedFile {
  path: string;
  status: 'added' | 'deleted' | 'modified';
  added: number | null;
  removed: number | null;
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
export interface ProviderPlugin {
  id: string;
  name: string;
  marketplace: string;
  version: string | null;
  description: string;
  installed: boolean;
  enabled: boolean;
  can_remove: boolean;
  unavailable_reason: string | null;
  auth_policy: string | null;
}
export interface PluginInventory {
  entries: ProviderPlugin[];
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
export interface ConversationSettings {
  compact_tokens: number | null;
  claude_compact_percent: number | null;
  response_style: 'standard' | 'concise' | 'detailed' | 'visual';
}
export interface ClientTransport {
  configureConversation(sessionId: string, settings: ConversationSettings): Promise<Session>;
  steerMessage(
    sessionId: string,
    expectedRun: string,
    prompt: string,
    attachments: Attachment[],
  ): Promise<void>;
  enqueueMessage(sessionId: string, prompt: string, attachments: Attachment[]): Promise<string>;
  editQueuedTask(id: string, prompt: string, direction: number): Promise<void>;
  taskQueue(): Promise<QueuedTask[]>;
  enqueueTask(sessionId: string, prompt: string): Promise<string>;
  changeQueuedTask(id: string, resume: boolean): Promise<void>;
  checkpoints(sessionId: string): Promise<Checkpoint[]>;
  createCheckpoint(sessionId: string, label: string): Promise<Checkpoint>;
  checkpointDiff(sessionId: string, id: string): Promise<CheckpointDiff[]>;
  restoreCheckpointFile(
    sessionId: string,
    id: string,
    path: string,
    expected: string,
  ): Promise<void>;
  deleteCheckpoint(sessionId: string, id: string): Promise<void>;
  branchChat(sessionId: string, isolated: boolean): Promise<Session>;
  projectFileRules(sessionId: string): Promise<FileRule[]>;
  saveProjectFileRules(sessionId: string, rules: FileRule[]): Promise<void>;
  computerStatus(): Promise<ComputerStatus>;
  installUnityAdapter(root: string): Promise<void>;
  computerWindows(): Promise<ComputerWindow[]>;
  configureComputer(sessionId: string, windows: string[]): Promise<void>;
  stopComputer(): Promise<void>;
  approveComputer(id: string, allow: boolean, sensitive: boolean): Promise<void>;
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
  accountPlugins(accountId: string): Promise<PluginInventory>;
  changeAccountPlugin(accountId: string, pluginId: string, install: boolean): Promise<void>;
  addAccountPluginSource(accountId: string, source: 'openai' | 'unity'): Promise<void>;
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
export interface QueuedTask {
  attachments?: string;
  id: string;
  session_id: string;
  prompt: string;
  state: string;
  run_id: string | null;
  error: string | null;
  created_at: number;
}
export interface Checkpoint {
  id: string;
  session_id: string;
  root: string;
  label: string;
  created_at: number;
  file_count: number;
  limited: boolean;
}
export interface CheckpointDiff {
  path: string;
  before: string | null;
  after: string | null;
  expected: string;
}
export interface FileRule {
  root: string;
  path: string;
  tool: string;
}

export interface LocalMcpServer {
  name: string;
  command: string;
  args: string[];
}

export interface ComputerWindow {
  id: string;
  title: string;
  application: string;
  pid: number;
  bounds: { x: number; y: number; width: number; height: number };
  dpi: number;
  focused: boolean;
}
export interface ComputerStatus {
  executing: boolean;
  available: boolean;
  enabled: boolean;
  stopped: boolean;
  owner_session: string | null;
  windows: ComputerWindow[];
  pending: {
    id: string;
    account: string;
    session: string;
    window: string | null;
    tool: string;
    action: string | null;
    sensitive: boolean;
    created_ms: number;
    description: string;
  } | null;
  metrics: Record<string, number>;
  audit: {
    at_ms: number;
    tool: string;
    action: string | null;
    execution: string;
    verification: string;
    latency_ms: number;
  }[];
  detail: string;
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
