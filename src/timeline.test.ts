import { describe, expect, it } from 'vitest';
import { buildTimeline, mergeEvents } from './timeline';
import type { AgentEvent, EventPayload } from './contracts';
const event = (sequence: number, run_id: string, payload: EventPayload): AgentEvent => ({
  sequence,
  run_id,
  payload,
  session_id: 'session',
  timestamp: 0,
});
describe('event replay', () => {
  it('deduplicates persisted events racing live events without losing text', () => {
    const first = event(1, 'a', { type: 'turn_started', prompt: 'Hello' });
    const second = event(2, 'a', { type: 'assistant_text_delta', text: 'Hi ' });
    const third = event(3, 'a', { type: 'assistant_text_delta', text: 'there' });
    const timeline = buildTimeline(mergeEvents([third, second], [first, second]));
    expect(timeline[0]?.text).toBe('Hi there');
    expect(timeline[0]?.prompt).toBe('Hello');
  });
  it('keeps independent turns and failure states separate', () => {
    const turns = buildTimeline([
      event(1, 'a', { type: 'turn_started', prompt: 'A' }),
      event(2, 'b', { type: 'turn_started', prompt: 'B' }),
      event(3, 'a', { type: 'session_stopped' }),
      event(4, 'b', { type: 'provider_error', message: 'Limit reached' }),
    ]);
    expect(turns.map((turn) => turn.status)).toEqual(['stopped', 'failed']);
  });
});
