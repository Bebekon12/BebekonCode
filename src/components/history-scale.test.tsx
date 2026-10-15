import { expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { Timeline } from './Timeline';
import type { AgentEvent, Session } from '../contracts';

it('mounts only forty recent turns and leaves tool details unmounted for a 1200-turn history', () => {
  const events: AgentEvent[] = [];
  for (let turn = 0; turn < 1200; turn++) {
    const common = { session_id: 'scale', run_id: `turn-${turn}`, timestamp: turn };
    events.push({
      ...common,
      sequence: events.length + 1,
      payload: { type: 'turn_started', prompt: `Question ${turn}` },
    });
    for (let step = 0; step < 12; step++)
      events.push({
        ...common,
        sequence: events.length + 1,
        payload: { type: 'tool_activity', label: 'Read', detail: `File ${step}` },
      });
    events.push({
      ...common,
      sequence: events.length + 1,
      payload: { type: 'assistant_text_delta', text: `Answer ${turn}` },
    });
    events.push({ ...common, sequence: events.length + 1, payload: { type: 'turn_completed' } });
  }
  const session: Session = {
    id: 'scale',
    workspace_id: 'project',
    provider: 'anthropic',
    account_profile_id: 'account',
    model: 'sonnet',
    title: 'Scale',
    status: 'completed',
    permission_profile: 'standard',
    working_directory: 'C:/fixture',
    provider_session_id: null,
    worktree_id: null,
    created_at: 0,
    updated_at: 1200,
    reasoning_effort: null,
    tool_policy: '{}',
    parent_session_id: null,
    chat_mode: 'single',
    role: '',
    context_summary: '',
  };
  const html = renderToStaticMarkup(
    <Timeline
      events={events}
      session={session}
      providerName="Claude"
      openFiles={() => {}}
      openTerminal={() => {}}
      cancel={() => {}}
      resolveApproval={async () => {}}
      retry={() => {}}
      chooseAnotherAccount={() => {}}
    />,
  );
  expect((html.match(/data-turn-id=/g) ?? []).length).toBe(40);
  expect(html).not.toContain('class="activity-table"');
  expect(html).toContain('Answer 1199');
  expect(html).not.toContain('Answer 0');
  expect(html).toContain('Показать более ранние сообщения');
});
