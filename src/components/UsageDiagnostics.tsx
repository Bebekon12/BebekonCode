import type { TimelineTurn } from '../timeline';

/** Report only observed launches and per-turn main-loop counters; never infer plan usage. */
export function UsageDiagnostics({ turn }: { turn: TimelineTurn }) {
  return (
    <>
      {['openai', 'anthropic'].map((provider) => (
        <ProviderDiagnostics key={provider} turn={turn} provider={provider} />
      ))}
    </>
  );
}

function ProviderDiagnostics({ turn, provider }: { turn: TimelineTurn; provider: string }) {
  const runs = turn.providerRuns.filter((run) => run.provider === provider);
  const usage = turn.usage.filter((sample) => sample.provider === provider);
  const codex = provider === 'openai';
  if (!runs.length && !usage.length) return null;
  const format = (value: number) => value.toLocaleString('ru-RU');
  const total = (
    key:
      | 'model_requests'
      | 'input_tokens'
      | 'output_tokens'
      | 'cache_read_tokens'
      | 'cache_creation_tokens'
      | 'reasoning_tokens',
  ) => {
    const values = usage
      .map((sample) => sample[key])
      .filter((value): value is number => value != null);
    if (!values.length) return 'Не сообщается';
    const partial = values.length < runs.length || usage.some((sample) => sample.incomplete);
    return `${format(values.reduce((sum, value) => sum + value, 0))}${partial ? ' · часть запусков' : ''}`;
  };
  return (
    <details className="usage-diagnostics">
      <summary>
        Расход {codex ? 'Codex' : 'Claude'} ·{' '}
        {runs.length ? `${runs.length} запусков` : 'данные CLI'}
      </summary>
      <dl>
        {!codex && (
          <div>
            <dt>Основных запросов модели</dt>
            <dd>{total('model_requests')}</dd>
          </div>
        )}
        <div>
          <dt>{codex ? 'Входных токенов, включая кэш' : 'Входных токенов без кэша'}</dt>
          <dd>{total('input_tokens')}</dd>
        </div>
        <div>
          <dt>Чтение кэша</dt>
          <dd>{total('cache_read_tokens')}</dd>
        </div>
        <div>
          <dt>Создание кэша</dt>
          <dd>{total('cache_creation_tokens')}</dd>
        </div>
        <div>
          <dt>Выходных токенов</dt>
          <dd>{total('output_tokens')}</dd>
        </div>
        {codex && (
          <div>
            <dt>Из них рассуждения</dt>
            <dd>{total('reasoning_tokens')}</dd>
          </div>
        )}
      </dl>
      {!!runs.length && (
        <p>
          Новых сессий: {runs.filter((run) => !run.resumed).length}; продолжений:{' '}
          {runs.filter((run) => run.resumed).length}.
        </p>
      )}
      {codex && (
        <p>
          По всем агентам сообщения — завершённых MCP-вызовов:{' '}
          {turn.activities.filter((item) => item.label.includes('MCP · ')).length}; изображений во
          вложениях: {turn.attachments.filter((file) => file.mime.startsWith('image/')).length}.
        </p>
      )}
      <p className="muted small">
        Запуск адаптера может содержать много запросов модели.{' '}
        {codex
          ? 'Токены рассчитаны по обновлениям app-server за этот запуск. При неизвестной исходной сумме, сбросе счётчиков или обрыве данные неполные. События других потоков не учитываются.'
          : 'Счётчики CLI относятся к основному циклу; вспомогательные запросы могут не входить в них.'}{' '}
        Это не процент лимита подписки и не счёт за оплату. При оборванном запуске часть данных
        недоступна.
      </p>
    </details>
  );
}
