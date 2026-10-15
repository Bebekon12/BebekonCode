import { expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { ChatMarkdown } from './ChatMarkdown';
import { Composer } from './Composer';
import { contextSnapshot } from './ConversationControls';
import { publicImageUrl } from './AnswerMedia';
import type { AgentEvent, Session } from '../contracts';
import { buildTimeline } from '../timeline';

const session = { provider: 'openai', model: 'selected' } as Session;
it('a new model does not inherit the previous model context observation', () => {
  const config = (model: string): AgentEvent['payload'] => ({
    type: 'agent_configuration',
    model,
    provider: 'openai',
    account_profile_id: 'account',
    reasoning_effort: null,
  });
  const events = [
    event(config('old')),
    event(
      {
        type: 'context_usage',
        used_tokens: 800,
        window_tokens: 1000,
        source: 'codex_last_request',
      },
      2,
    ),
    event(config('selected'), 3),
  ];
  expect(contextSnapshot(events, session).percent).toBeNull();
});
const event = (payload: AgentEvent['payload'], sequence = 1): AgentEvent => ({
  payload,
  sequence,
  run_id: 'run',
  session_id: 'chat',
  timestamp: 0,
});
it('context uses the latest observation and becomes unknown after compaction, never sums bills', () => {
  const events = [
    event({
      type: 'context_usage',
      used_tokens: 500,
      window_tokens: 1000,
      source: 'codex_last_request',
    }),
    event(
      {
        type: 'context_usage',
        used_tokens: 100,
        window_tokens: 1000,
        source: 'codex_last_request',
      },
      2,
    ),
  ];
  expect(contextSnapshot(events, session).percent).toBe(10);
  expect(
    contextSnapshot([...events, event({ type: 'context_compacted' }, 3)], session).used,
  ).toBeNull();
  expect(contextSnapshot([], session).percent).toBeNull();
});
it('followups stay attached to the original turn without creating a second model run', () => {
  const turns = buildTimeline([
    event({ type: 'turn_started', prompt: 'First' }),
    event({ type: 'user_followup', prompt: 'Correction', files: [] }, 2),
  ]);
  expect(turns).toHaveLength(1);
  expect(turns[0]?.followups[0]?.prompt).toBe('Correction');
  expect(turns[0]?.providerRuns).toHaveLength(0);
});
it('rich answers render safe images, source links and chart data without executing markup', () => {
  const html = renderToStaticMarkup(
    <ChatMarkdown
      streaming={false}
      text={
        '![Sample](https://images.example.com/a.png)\n\nText beside the image. [Source](https://example.com/source)\n\n```bebekon-line\n{"title":"Observed","items":[{"label":"A","value":-2},{"label":"B","value":8}]}\n```\n\n<script>alert(1)</script>'
      }
    />,
  );
  expect(html).toContain('crossorigin="anonymous"');
  expect(html).toContain('referrerPolicy="no-referrer"');
  expect(html).toContain('answer-image');
  expect(html).toContain('<polyline');
  expect(html).toContain('Данные графика');
  expect(html).not.toContain('<script>');
  for (const url of [
    'javascript:alert(1)',
    'file:///x',
    'https://127.0.0.1/a',
    'https://u:secret@example.com/a',
    'https://[::1]/a',
    'https://host.local/a',
  ])
    expect(publicImageUrl(url)).toBeUndefined();
});
it('a working agent does not lock the composer or attachments and exposes queue plus stop', () => {
  const html = renderToStaticMarkup(
    <Composer
      draft="Next"
      setDraft={() => {}}
      running
      busy={false}
      send={() => {}}
      cancel={() => {}}
      controls={null}
      demo
      attachments={[]}
      setAttachments={() => {}}
      followupMode="queue"
    />,
  );
  expect(html).toContain('Добавить в очередь');
  expect(html).toContain('Остановить агента');
  expect(html).not.toMatch(/<textarea[^>]*disabled/);
  expect(html).not.toMatch(/aria-label="Прикрепить файлы"[^>]*disabled/);
});
