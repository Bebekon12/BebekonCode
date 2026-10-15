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
  const reports = turn.diagnostics.filter((report) => report.provider === provider);
  if (!runs.length && !usage.length && !reports.length) return null;
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
            <dt>Ходов основного цикла CLI</dt>
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
      {!!runs.length && (
        <ul className="diagnostic-launches">
          {runs.map((run, index) => (
            <li key={index}>
              {index + 1}.{' '}
              {{
                execution: 'Исполнение',
                planning: 'Планирование',
                synthesis: 'Общий итог',
                specialist: 'Записка специалиста',
                implementation: 'Реализация и проверка',
              }[run.purpose] ?? run.purpose}
              {' · '}
              {run.model}
              {' · '}
              {run.resumed ? 'продолжение' : 'новая сессия'}
              {' · '}
              {format(run.prompt_bytes)} байт запроса
            </li>
          ))}
        </ul>
      )}
      {reports.map((report) => (
        <details className="diagnostic-report" key={report.request_id}>
          <summary>
            {report.resolved_model ?? report.requested_model} ·{' '}
            {{ completed: 'Завершено', stopped: 'Остановлено', failed: 'Ошибка' }[report.status] ??
              report.status}
            {' · '}
            {(report.elapsed_ms / 1000).toFixed(1)} с
          </summary>
          <dl>
            <div>
              <dt>Запуск</dt>
              <dd>{report.request_id}</dd>
            </div>
            <div>
              <dt>Профиль</dt>
              <dd>{report.account_profile_id}</dd>
            </div>
            <div>
              <dt>Сессия приложения</dt>
              <dd>{report.session_id}</dd>
            </div>
            <div>
              <dt>Сессия {codex ? 'Codex' : 'Claude'}</dt>
              <dd>{report.provider_session_id ?? 'Не сообщается'}</dd>
            </div>
            <div>
              <dt>Запрошенное обдумывание</dt>
              <dd>{report.effort ?? 'Настройка CLI'}</dd>
            </div>
            <div>
              <dt>Доступ / Fast</dt>
              <dd>
                {report.permission_profile} / {report.fast_mode ? 'вкл.' : 'выкл.'}
              </dd>
            </div>
            <div>
              <dt>Событий инструментов / запросов разрешений</dt>
              <dd>
                {report.tool_requests} / {report.approval_requests}
              </dd>
            </div>
            <div>
              <dt>Повторов, сообщённых CLI</dt>
              <dd>{report.reported_retries}</dd>
            </div>
            <div>
              <dt>Статистика токенов</dt>
              <dd>
                {report.usage_reported
                  ? report.usage_incomplete
                    ? 'Получена частично'
                    : 'Получена'
                  : 'Не получена'}
              </dd>
            </div>
            <div>
              <dt>Ходов основного цикла CLI</dt>
              <dd>
                {report.model_requests == null ? 'Не сообщается' : format(report.model_requests)}
              </dd>
            </div>
            <div>
              <dt>Вход / выход</dt>
              <dd>
                {report.input_tokens ?? '—'} / {report.output_tokens ?? '—'}
              </dd>
            </div>
            <div>
              <dt>Кэш: чтение / создание</dt>
              <dd>
                {report.cache_read_tokens ?? '—'} / {report.cache_creation_tokens ?? '—'}
              </dd>
            </div>
            {report.error_kind && (
              <div>
                <dt>Причина ошибки</dt>
                <dd>{report.error_kind}</dd>
              </div>
            )}
            {codex && (
              <div>
                <dt>Событий вложенных агентов</dt>
                <dd>{report.subagent_events ?? 'Не сообщается'}</dd>
              </div>
            )}
          </dl>
        </details>
      ))}
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
