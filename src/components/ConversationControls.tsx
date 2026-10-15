import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import type { AgentEvent, ClientTransport, ConversationSettings, Session } from '../contracts';

export function contextSnapshot(events: AgentEvent[], session: Session) {
  let used: number | null = null,
    window: number | null = null,
    source = '',
    compacted = false;
  let identity = '',
    matches = true;
  for (const event of events) {
    const p = event.payload;
    if (p.type === 'agent_configuration') {
      const next = `${p.provider}:${p.model}:${p.account_profile_id}`;
      matches =
        p.provider === session.provider &&
        p.model === session.model &&
        (!session.account_profile_id || p.account_profile_id === session.account_profile_id);
      if (!matches || (identity && identity !== next)) {
        used = null;
        window = null;
        source = '';
      }
      identity = next;
    }
    if (p.type === 'context_usage' && matches) {
      used = p.used_tokens;
      window = p.window_tokens ?? window;
      source = p.source;
      compacted = false;
    }
    if (p.type === 'context_compacted' && matches) {
      used = null;
      compacted = true;
    }
  }
  const last = events.filter((e) => e.payload.type === 'agent_configuration').at(-1)?.payload;
  if (
    last?.type === 'agent_configuration' &&
    (last.model !== session.model || last.provider !== session.provider)
  ) {
    used = null;
    window = null;
  }
  return {
    used,
    window,
    source,
    compacted,
    percent: used !== null && window ? Math.round((100 * used) / window) : null,
  };
}
export function ConversationControls({
  session,
  events,
  client,
  running,
  updated,
}: {
  session: Session;
  events: AgentEvent[];
  client: ClientTransport;
  running: boolean;
  updated: (s: Session) => void;
}) {
  const initial = (): ConversationSettings => ({
    compact_tokens: null,
    claude_compact_percent: null,
    response_style: 'standard',
    ...JSON.parse(session.conversation_settings || '{}'),
  });
  const [settings, setSettings] = useState(initial);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState('');
  const [confirmCompact, setConfirmCompact] = useState(false);
  const meter = contextSnapshot(events, session);
  const supported = ['openai', 'anthropic'].includes(session.provider);
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<CSSProperties>({});
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const anchor = trigger.current?.getBoundingClientRect();
      if (!anchor) return;
      const width = Math.min(360, window.innerWidth - 24);
      const above = anchor.top > window.innerHeight / 2;
      setPosition({
        width,
        left: Math.max(12, Math.min(anchor.right - width, window.innerWidth - width - 12)),
        ...(above ? { bottom: window.innerHeight - anchor.top + 8 } : { top: anchor.bottom + 8 }),
        maxHeight: Math.min(
          560,
          Math.max(120, (above ? anchor.top : window.innerHeight - anchor.bottom) - 24),
        ),
      });
    };
    place();
    window.addEventListener('resize', place);
    window.addEventListener('scroll', place, true);
    return () => {
      window.removeEventListener('resize', place);
      window.removeEventListener('scroll', place, true);
    };
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !trigger.current?.contains(event.target) &&
        !panel.current?.contains(event.target)
      )
        setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        setOpen(false);
        trigger.current?.focus();
      }
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape, true);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape, true);
    };
  }, [open]);
  const tooltip =
    meter.percent === null ? 'Контекст: нет данных' : `Контекст: заполнено ${meter.percent}%`;
  return (
    <div className="conversation-controls">
      <button
        ref={trigger}
        type="button"
        className="context-ring-button"
        aria-label="Контекст и оформление ответа"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={tooltip}
        onClick={() => {
          if (!open) {
            setSettings(initial());
            setError('');
            setConfirmCompact(false);
          }
          setOpen(!open);
        }}
      >
        <svg
          className={`context-ring ${meter.percent === null ? 'context-ring-unknown' : ''}`}
          viewBox="0 0 20 20"
          width="20"
          height="20"
          aria-hidden="true"
        >
          <circle
            className="context-ring-track"
            cx="10"
            cy="10"
            r="7"
            fill="none"
            strokeWidth="2"
          />
          <circle
            className="context-ring-value"
            cx="10"
            cy="10"
            r="7"
            fill="none"
            strokeWidth="2"
            pathLength="100"
            strokeDasharray={`${meter.percent === null ? 20 : Math.max(0, Math.min(100, meter.percent))} 100`}
            transform="rotate(-90 10 10)"
          />
        </svg>
      </button>
      {open &&
        createPortal(
          <div
            ref={panel}
            style={position}
            className="conversation-panel"
            role="dialog"
            aria-label="Контекст и оформление ответа"
          >
            <strong>Контекст</strong>
            {meter.percent !== null && (
              <progress
                max={100}
                value={Math.min(100, meter.percent)}
                aria-label="Заполнение контекста"
              />
            )}
            <p>
              {meter.compacted
                ? 'Сжат, ждём замер'
                : meter.used === null
                  ? 'Нет данных'
                  : `${meter.used.toLocaleString('ru-RU')}${meter.window ? ` / ${meter.window.toLocaleString('ru-RU')}` : ''} токенов`}
            </p>
            <label>
              Автосжатие
              <select
                disabled={running || busy || !supported}
                value={settings.compact_tokens ?? ''}
                onChange={(e) =>
                  setSettings({
                    ...settings,
                    compact_tokens: e.target.value ? Number(e.target.value) : null,
                  })
                }
              >
                <option value="">По умолчанию</option>
                {[100000, 200000, 300000, 500000, 1000000].map((n) => (
                  <option key={n} value={n}>
                    {n.toLocaleString('ru-RU')} токенов
                  </option>
                ))}
              </select>
            </label>
            {session.provider === 'anthropic' && (
              <label>
                Сжимать при
                <select
                  disabled={running || busy}
                  value={settings.claude_compact_percent ?? ''}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      claude_compact_percent: e.target.value ? Number(e.target.value) : null,
                    })
                  }
                >
                  <option value="">По умолчанию</option>
                  {[50, 70, 80, 90, 95].map((n) => (
                    <option key={n} value={n}>
                      {n}% ориентира
                    </option>
                  ))}
                </select>
              </label>
            )}
            {!supported && <p className="small muted">Сжатие недоступно у провайдера</p>}
            {supported && (
              <div className="compact-action">
                <button
                  disabled={running || busy || !session.provider_session_id}
                  onClick={() => setConfirmCompact(true)}
                >
                  Сжать сейчас
                </button>
                {confirmCompact && (
                  <div role="alertdialog" aria-label="Сжать контекст">
                    <p>Заменить историю сводкой? Часть деталей потеряется.</p>
                    <button
                      disabled={running || busy}
                      onClick={async () => {
                        setBusy(true);
                        setError('');
                        try {
                          await client.sendMessage(session.id, '/compact', []);
                          setConfirmCompact(false);
                        } catch (e) {
                          setError(String(e));
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      Сжать
                    </button>
                    <button onClick={() => setConfirmCompact(false)}>Отмена</button>
                  </div>
                )}
              </div>
            )}
            <label>
              Оформление ответа
              <select
                disabled={running || busy}
                value={settings.response_style}
                onChange={(e) =>
                  setSettings({
                    ...settings,
                    response_style: e.target.value as ConversationSettings['response_style'],
                  })
                }
              >
                <option value="standard">Стандартное</option>
                <option value="concise">Кратко</option>
                <option value="detailed">С объяснениями</option>
                <option value="visual">Наглядно</option>
              </select>
            </label>
            <button
              disabled={busy || running}
              onClick={async () => {
                setBusy(true);
                setError('');
                try {
                  updated(await client.configureConversation(session.id, settings));
                } catch (e) {
                  setError(String(e));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Сохранить настройки чата
            </button>
            {error && <p role="alert">{error}</p>}
          </div>,
          document.body,
        )}
    </div>
  );
}
