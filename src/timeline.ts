import type { AgentEvent, SessionStatus } from './contracts';
export interface TimelineTurn {
  id: string;
  prompt: string;
  text: string;
  activities: { label: string; detail: string }[];
  status: SessionStatus;
  error?: string;
}
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
      turn = { id: event.run_id, prompt: '', text: '', activities: [], status: 'running' };
      turns.set(event.run_id, turn);
    }
    switch (event.payload.type) {
      case 'turn_started':
        turn.prompt = event.payload.prompt;
        break;
      case 'assistant_text_delta':
        turn.text += event.payload.text;
        break;
      case 'tool_activity':
        turn.activities.push(event.payload);
        break;
      case 'turn_completed':
        turn.status = 'completed';
        break;
      case 'session_stopped':
        turn.status = 'stopped';
        break;
      case 'provider_error':
        turn.status = 'failed';
        turn.error = event.payload.message;
        break;
    }
  }
  return [...turns.values()];
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
