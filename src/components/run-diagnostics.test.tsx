import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { RunChanges } from './RunChanges';
import { UsageDiagnostics } from './UsageDiagnostics';
import { buildTimeline } from '../timeline';
import type { AgentEvent, EventPayload } from '../contracts';

describe('turn diagnostics rendering', () => {
  it('shows Codex counters separately and does not add reasoning or cached tokens to totals', () => {
    const payloads: EventPayload[] = [
      { type: 'turn_started', prompt: 'Codex' },
      {
        type: 'provider_run_started',
        provider: 'openai',
        model: 'test',
        purpose: 'execution',
        resumed: true,
        prompt_bytes: 40,
      },
      {
        type: 'provider_usage',
        provider: 'openai',
        model_requests: null,
        input_tokens: 100,
        output_tokens: 20,
        cache_read_tokens: 80,
        cache_creation_tokens: null,
        reasoning_tokens: 10,
        incomplete: true,
      },
      { type: 'turn_completed' },
    ];
    const [turn] = buildTimeline(
      payloads.map((payload, sequence) => ({
        payload,
        sequence,
        session_id: 'session',
        run_id: 'run',
        timestamp: 0,
      })),
    );
    const html = renderToStaticMarkup(<UsageDiagnostics turn={turn!} />);
    expect(html).toContain('Расход Codex');
    expect(html).not.toContain('Расход Claude');
    expect(html).toContain('Входных токенов, включая кэш');
    expect(html).toContain('Из них рассуждения');
    expect(html).toContain('100 · часть запусков');
    expect(html).not.toContain('180');
    expect(html).toContain('Не сообщается');
  });
  it('hides an empty snapshot and explains partial line counts without using worktree totals', () => {
    expect(renderToStaticMarkup(<RunChanges summary={{ files: [], limited: false }} />)).toBe('');
    expect(renderToStaticMarkup(<RunChanges summary={{ files: [], limited: true }} />)).toBe('');
    const html = renderToStaticMarkup(
      <RunChanges
        summary={{
          files: [
            { path: 'new.rs', status: 'modified', added: 1, removed: 0 },
            { path: 'asset.bin', status: 'modified', added: null, removed: null },
          ],
          limited: true,
        }}
      />,
    );
    expect(html).toContain('За последнее сообщение');
    expect(html).toContain('new.rs');
    expect(html).toContain('+1');
    expect(html).toContain('часть файлов или строк');
    expect(html).not.toContain('строки могут учитываться дважды');
  });

  it('labels incomplete reported Claude counters and never invents quota percentages', () => {
    const payloads: EventPayload[] = [
      { type: 'turn_started', prompt: 'Review' },
      {
        type: 'provider_run_started',
        provider: 'anthropic',
        model: 'opus',
        purpose: 'execution',
        resumed: true,
        prompt_bytes: 20,
      },
      {
        type: 'provider_run_started',
        provider: 'anthropic',
        model: 'opus',
        purpose: 'synthesis',
        resumed: false,
        prompt_bytes: 50,
      },
      {
        type: 'provider_usage',
        provider: 'anthropic',
        model_requests: 2,
        input_tokens: 10,
        output_tokens: 8,
        cache_read_tokens: 20,
        cache_creation_tokens: null,
      },
      { type: 'turn_completed' },
    ];
    const events: AgentEvent[] = payloads.map((payload, sequence) => ({
      payload,
      sequence,
      session_id: 'session',
      run_id: 'run',
      timestamp: 0,
    }));
    const [turn] = buildTimeline(events);
    const html = renderToStaticMarkup(<UsageDiagnostics turn={turn!} />);
    expect(html).toContain('2 запусков');
    expect(html).toContain('часть запусков');
    expect(html).toContain('Не сообщается');
    expect(html).toContain('Новых сессий: 1');
    expect(html).toContain('продолжений: 1');
    expect(html).toContain('не процент лимита подписки');
    expect(html).not.toContain('%');
  });
});
