import type { AgentEvent, ApprovalDecision, SessionStatus } from './contracts';
export interface TimelineActivity {
  label: string;
  detail: string;
  timestamp: number;
}
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
  attachments: { name: string; path: string; mime: string }[];
  activities: TimelineActivity[];
  status: SessionStatus;
  startedAt: number;
  finishedAt?: number;
  error?: string;
  errorKind?: string;
  approvals: TimelineApproval[];
}
export type TimelineFilter = 'all' | 'agent' | 'tools';
export function mergeEvents(current: AgentEvent[], incoming: AgentEvent[]): AgentEvent[] {
  const events = new Map(current.map((event) => [event.sequence, event]));
  incoming.forEach((event) => events.set(event.sequence, event));
  return [...events.values()].sort((a, b) => a.sequence - b.sequence).slice(-600);
}
export function buildTimeline(events: AgentEvent[]): TimelineTurn[] {
  const turns = new Map<string, TimelineTurn>();
  for (const event of events) {
    let turn = turns.get(event.run_id);
    if (!turn) {
      turn = {
        id: event.run_id,
        prompt: '',
        text: '',
        progress: '',
        teamMessages: [],
        attachments: [],
        activities: [],
        approvals: [],
        status: 'running',
        startedAt: event.timestamp,
      };
      turns.set(event.run_id, turn);
    }
    switch (event.payload.type) {
      case 'turn_started':
        turn.prompt = event.payload.prompt;
        break;
      case 'assistant_text_delta':
        turn.text += event.payload.text;
        break;
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
        break;
      case 'tool_activity':
        turn.activities.push({
          label: event.payload.label,
          detail: event.payload.detail,
          timestamp: event.timestamp,
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
