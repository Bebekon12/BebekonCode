import { describe, expect, it } from 'vitest';
import { buildTimeline, filterTimeline, mergeEvents } from './timeline';
import type { AgentEvent, EventPayload } from './contracts';
const event = (sequence: number, run_id: string, payload: EventPayload): AgentEvent => ({
  sequence,
  run_id,
  payload,
  session_id: 'session',
  timestamp: 0,
});
describe('event replay', () => {
  it('preserves provider approval choices including an empty subset', () => {
    const request = {
      type: 'approval_requested' as const,
      id: 'worker:approval',
      kind: 'command',
      title: 'Команда',
      detail: 'npm test',
      cwd: null,
      reason: null,
    };
    const [turn] = buildTimeline([
      event(1, 'a', { ...request, available_decisions: ['allow_once', 'deny'] }),
      event(2, 'a', { ...request, id: 'empty', available_decisions: [] }),
      event(3, 'a', { ...request, id: 'old' }),
    ]);
    expect(turn?.approvals[0]?.available_decisions).toEqual(['allow_once', 'deny']);
    expect(turn?.approvals[1]?.available_decisions).toEqual([]);
    expect(turn?.approvals[2]?.available_decisions).toBeUndefined();
  });
  it('keeps worker stages separate and updates the reported model without duplicating text', () => {
    const message = {
      type: 'team_message' as const,
      session_id: 'worker',
      title: 'Код',
      reasoning_effort: 'high',
    };
    const [turn] = buildTimeline([
      event(1, 'a', { ...message, stage_id: 'write', text: 'Готово' }),
      event(2, 'a', { ...message, stage_id: 'write', text: '', model: 'claude-opus-4-6' }),
      event(3, 'a', { ...message, stage_id: 'review', text: 'Проверено' }),
      event(4, 'a', {
        type: 'user_attachments',
        files: [{ name: 'video.mp4', path: '/project/video.mp4', mime: 'video/mp4' }],
      }),
    ]);
    expect(turn?.teamMessages).toHaveLength(2);
    expect(turn?.teamMessages[0]).toMatchObject({
      text: 'Готово',
      model: 'claude-opus-4-6',
      reasoningEffort: 'high',
    });
    expect(turn?.teamMessages[1]?.text).toBe('Проверено');
    expect(turn?.attachments[0]?.name).toBe('video.mp4');
  });
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

describe('timeline filters', () => {
  const turns = buildTimeline([
    event(1, 'a', { type: 'turn_started', prompt: 'Почини навигацию' }),
    event(2, 'a', { type: 'assistant_text_delta', text: 'Готово' }),
    event(3, 'b', { type: 'turn_started', prompt: 'Запусти тесты' }),
    event(4, 'b', { type: 'tool_activity', label: 'Ran', detail: 'cargo test' }),
  ]);
  it('keeps only turns with tool activity in the tools view', () => {
    expect(filterTimeline(turns, 'tools', '').map((turn) => turn.id)).toEqual(['b']);
  });
  it('searches case-insensitively in Cyrillic', () => {
    expect(filterTimeline(turns, 'all', 'НАВИГАЦ').map((turn) => turn.id)).toEqual(['a']);
  });
  it('does not match text hidden by the active filter', () => {
    expect(filterTimeline(turns, 'agent', 'навигац')).toEqual([]);
    expect(filterTimeline(turns, 'agent', 'cargo')).toEqual([]);
    expect(filterTimeline(turns, 'tools', 'cargo').map((turn) => turn.id)).toEqual(['b']);
  });
  it('records when each turn started', () => {
    expect(turns[0]?.startedAt).toBe(0);
  });
});

describe('approvals and provider errors', () => {
  it('pairs approval requests with their resolution', () => {
    const [turn] = buildTimeline([
      event(1, 'a', { type: 'turn_started', prompt: 'Запусти тесты' }),
      event(2, 'a', {
        type: 'approval_requested',
        id: 'x',
        kind: 'command',
        title: 'Codex хочет выполнить команду',
        detail: 'npm test',
        cwd: 'C:\app',
        reason: null,
      }),
      event(3, 'a', { type: 'approval_resolved', id: 'x', decision: 'allow_once' }),
    ]);
    expect(turn?.approvals).toHaveLength(1);
    expect(turn?.approvals[0]?.decision).toBe('allow_once');
    expect(filterTimeline([turn!], 'tools', 'npm').map((item) => item.id)).toEqual(['a']);
  });
  it('keeps the provider error kind for manual recovery', () => {
    const [turn] = buildTimeline([
      event(1, 'a', { type: 'turn_started', prompt: 'x' }),
      event(2, 'a', { type: 'provider_error', message: 'limit', kind: 'usage_limit' }),
    ]);
    expect(turn?.status).toBe('failed');
    expect(turn?.errorKind).toBe('usage_limit');
  });
});
