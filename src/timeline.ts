import type {
  AttachedFile,
  AgentEvent,
  ApprovalDecision,
  SessionStatus,
  TodoItem,
  ToolKind,
} from './contracts';
export interface TimelineActivity {
  label: string;
  detail: string;
  timestamp: number;
}
/** One tool call in a collapsed group, as in Claude Desktop ("Ran 3 commands"). */
export interface ToolItem {
  id: string;
  tool: string;
  kind: ToolKind | 'activity';
  summary: string;
  detail?: string;
  added?: number | null;
  removed?: number | null;
  state: 'running' | 'done' | 'failed';
  startedAt: number;
  finishedAt?: number;
}
export interface TodoEntry {
  key: string;
  content: string;
  status: 'pending' | 'in_progress' | 'completed';
}
/** Answer text and tool groups in the order they happened. */
export type TurnPart =
  | { kind: 'text'; text: string }
  | { kind: 'tools'; items: ToolItem[] }
  | { kind: 'todos'; items: TodoEntry[] };
export interface TimelineApproval {
  id: string;
  kind: string;
  title: string;
  detail: string;
  cwd: string | null;
  reason: string | null;
  available_decisions?: ApprovalDecision[] | null;
  decision?: string;
  timestamp: number;
}
export interface TimelineTurn {
  id: string;
  prompt: string;
  followups: { prompt: string; files: AttachedFile[] }[];
  text: string;
  progress: string;
  teamMessages: {
    sessionId: string;
    stageId: string;
    title: string;
    text: string;
    model?: string | null;
    reasoningEffort?: string | null;
  }[];
  provider?: string;
  model?: string;
  reasoningEffort?: string | null;
  fastMode?: boolean;
  changes?: { files: import('./contracts').ChangedFile[]; limited: boolean };
  providerRuns: Extract<import('./contracts').EventPayload, { type: 'provider_run_started' }>[];
  usage: Extract<import('./contracts').EventPayload, { type: 'provider_usage' }>[];
  diagnostics: import('./contracts').ProviderRunDiagnostic[];
  attachments: { name: string; path: string; mime: string }[];
  activities: TimelineActivity[];
  parts: TurnPart[];
  status: SessionStatus;
  startedAt: number;
  finishedAt?: number;
  error?: string;
  errorKind?: string;
  approvals: TimelineApproval[];
}
export type TimelineFilter = 'all' | 'agent' | 'tools';
/** Events per page returned by the core (`Storage::events`); a full page means more remain. */
export const historyPageSize = 300;
// The whole stored history stays in view: trimming here used to drop early turns.
export function mergeEvents(current: AgentEvent[], incoming: AgentEvent[]): AgentEvent[] {
  const events = new Map(current.map((event) => [event.sequence, event]));
  incoming.forEach((event) => events.set(event.sequence, event));
  return [...events.values()].sort((a, b) => a.sequence - b.sequence);
}
// Task tools update single items across turns, so the list is folded over the whole chat.
function applyTodos(current: TodoEntry[], replace: boolean, items: TodoItem[]): TodoEntry[] {
  const status = (value: TodoItem['status']): TodoEntry['status'] =>
    value === 'completed' || value === 'in_progress' ? value : 'pending';
  if (replace)
    return items
      .filter((item) => item.content && item.status !== 'deleted')
      .map((item, index) => ({
        key: `w${index}`,
        content: item.content ?? '',
        status: status(item.status),
      }));
  const next = current.filter((entry) => entry.key.startsWith('t'));
  for (const item of items) {
    if (!item.id) continue;
    const key = `t${item.id}`;
    const index = next.findIndex((entry) => entry.key === key);
    if (item.status === 'deleted') {
      if (index >= 0) next.splice(index, 1);
      continue;
    }
    const previous = next[index];
    const entry = {
      key,
      content: item.content ?? previous?.content ?? `Задача ${item.id}`,
      status: item.status ? status(item.status) : (previous?.status ?? 'pending'),
    };
    if (index >= 0) next[index] = entry;
    else next.push(entry);
  }
  return next;
}
function addTool(turn: TimelineTurn, item: ToolItem) {
  for (const part of turn.parts)
    if (part.kind === 'tools') {
      const existing = part.items.find((entry) => entry.id === item.id);
      if (existing) return void Object.assign(existing, { ...item, state: existing.state });
    }
  const last = turn.parts.at(-1);
  if (last?.kind === 'tools') last.items.push(item);
  else turn.parts.push({ kind: 'tools', items: [item] });
}
export function buildTimeline(events: AgentEvent[]): TimelineTurn[] {
  const turns = new Map<string, TimelineTurn>();
  let todos: TodoEntry[] = [];
  for (const event of events) {
    let turn = turns.get(event.run_id);
    if (!turn) {
      turn = {
        id: event.run_id,
        prompt: '',
        followups: [],
        text: '',
        progress: '',
        teamMessages: [],
        attachments: [],
        activities: [],
        parts: [],
        approvals: [],
        providerRuns: [],
        usage: [],
        diagnostics: [],
        status: 'running',
        startedAt: event.timestamp,
      };
      turns.set(event.run_id, turn);
    }
    switch (event.payload.type) {
      case 'user_followup':
        turn.followups.push({ prompt: event.payload.prompt, files: event.payload.files });
        break;
      case 'context_compacted':
        turn.activities.push({
          label: 'Контекст сжат',
          detail: 'Провайдер обновляет рабочий контекст; история чата сохранена.',
          timestamp: event.timestamp,
        });
        break;
      case 'turn_started':
        turn.prompt = event.payload.prompt;
        break;
      case 'assistant_text_delta': {
        turn.text += event.payload.text;
        const last = turn.parts.at(-1);
        if (last?.kind === 'text') last.text += event.payload.text;
        else turn.parts.push({ kind: 'text', text: event.payload.text });
        break;
      }
      case 'tool_started':
        turn.activities.push({
          label: event.payload.tool,
          detail: event.payload.summary,
          timestamp: event.timestamp,
        });
        addTool(turn, {
          id: event.payload.id,
          tool: event.payload.tool,
          kind: event.payload.kind,
          summary: event.payload.summary,
          added: event.payload.added,
          removed: event.payload.removed,
          state: 'running',
          startedAt: event.timestamp,
        });
        break;
      case 'tool_finished': {
        const id = event.payload.id;
        for (const part of turn.parts)
          if (part.kind === 'tools')
            for (const item of part.items)
              if (item.id === id) {
                item.state = event.payload.failed ? 'failed' : 'done';
                item.finishedAt = event.timestamp;
              }
        break;
      }
      case 'todo_update': {
        todos = applyTodos(todos, event.payload.replace, event.payload.items);
        turn.parts = turn.parts.filter((part) => part.kind !== 'todos');
        if (todos.length) turn.parts.push({ kind: 'todos', items: todos });
        break;
      }
      case 'progress_delta':
        turn.progress += event.payload.text;
        break;
      case 'model_resolved':
        turn.model = event.payload.model;
        break;
      case 'user_attachments':
        turn.attachments = event.payload.files;
        break;
      case 'team_message': {
        const payload = event.payload;
        const stageId = payload.stage_id ?? '';
        const existing = turn.teamMessages.find(
          (m) => m.sessionId === payload.session_id && m.stageId === stageId,
        );
        if (existing) {
          existing.text += payload.text;
          if (payload.model) existing.model = payload.model;
        } else
          turn.teamMessages.push({
            sessionId: payload.session_id,
            stageId,
            title: payload.title,
            text: payload.text,
            model: payload.model,
            reasoningEffort: payload.reasoning_effort,
          });
        break;
      }
      case 'agent_configuration':
        turn.provider = event.payload.provider;
        turn.model = event.payload.model;
        turn.reasoningEffort = event.payload.reasoning_effort;
        turn.fastMode = event.payload.fast_mode ?? false;
        break;
      case 'run_changes':
        turn.changes = { files: event.payload.files, limited: event.payload.limited };
        break;
      case 'provider_run_started':
        turn.providerRuns.push(event.payload);
        break;
      case 'provider_usage':
        turn.usage.push(event.payload);
        break;
      case 'provider_diagnostic': {
        const report = event.payload.report;
        const index = turn.diagnostics.findIndex((item) => item.request_id === report.request_id);
        if (index < 0) turn.diagnostics.push(report);
        else turn.diagnostics[index] = report;
        break;
      }
      case 'tool_activity':
        turn.activities.push({
          label: event.payload.label,
          detail: event.payload.detail,
          timestamp: event.timestamp,
        });
        addTool(turn, {
          id: `activity-${event.sequence}`,
          tool: event.payload.label,
          kind: 'activity',
          summary: event.payload.label,
          detail: event.payload.detail,
          state: 'done',
          startedAt: event.timestamp,
        });
        break;
      case 'approval_requested':
        turn.approvals.push({
          id: event.payload.id,
          kind: event.payload.kind,
          title: event.payload.title,
          detail: event.payload.detail,
          cwd: event.payload.cwd,
          reason: event.payload.reason,
          available_decisions: event.payload.available_decisions,
          timestamp: event.timestamp,
        });
        break;
      case 'approval_resolved': {
        const id = event.payload.id;
        const approval = turn.approvals.find((item) => item.id === id);
        if (approval) approval.decision = event.payload.decision;
        break;
      }
      case 'turn_completed':
        turn.status = 'completed';
        turn.finishedAt = event.timestamp;
        break;
      case 'session_stopped':
        turn.status = 'stopped';
        turn.finishedAt = event.timestamp;
        break;
      case 'provider_error':
        turn.status = 'failed';
        turn.error = event.payload.message;
        turn.errorKind = event.payload.kind ?? undefined;
        turn.finishedAt = event.timestamp;
        break;
    }
  }
  for (const turn of turns.values())
    if (turn.status !== 'running')
      for (const part of turn.parts)
        if (part.kind === 'tools')
          for (const item of part.items) if (item.state === 'running') item.state = 'done';
  return [...turns.values()];
}
// Matches only the fields the active filter renders, so hidden text never produces a hit.
export function filterTimeline(
  turns: TimelineTurn[],
  filter: TimelineFilter,
  query: string,
): TimelineTurn[] {
  const needle = query.trim().toLocaleLowerCase('ru-RU');
  return turns.filter((turn) => {
    // A search or display filter must never hide an actionable permission request.
    if (turn.status === 'running' && turn.approvals.some((approval) => !approval.decision))
      return true;
    if (filter === 'tools' && turn.activities.length === 0 && turn.approvals.length === 0)
      return false;
    if (!needle) return true;
    const haystack = [
      filter === 'all' ? turn.prompt : '',
      filter === 'tools'
        ? ''
        : `${turn.text} ${turn.progress} ${turn.teamMessages.map((m) => `${m.title} ${m.text}`).join(' ')}`,
      filter === 'agent' ? '' : turn.activities.map((a) => `${a.label} ${a.detail}`).join(' '),
      turn.error ?? '',
      filter === 'agent' ? '' : turn.approvals.map((a) => `${a.title} ${a.detail}`).join(' '),
    ];
    return haystack.join('\n').toLocaleLowerCase('ru-RU').includes(needle);
  });
}
export function eventStatus(event: AgentEvent): SessionStatus | undefined {
  switch (event.payload.type) {
    case 'turn_started':
      return 'running';
    case 'turn_completed':
      return 'completed';
    case 'session_stopped':
      return 'stopped';
    case 'provider_error':
      return 'failed';
    default:
      return undefined;
  }
}
