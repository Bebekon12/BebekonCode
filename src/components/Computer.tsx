import { useEffect, useState } from 'react';
import { Monitor, RefreshCw, Square, ShieldCheck, Check, X } from 'lucide-react';
import type { ClientTransport, ComputerStatus, ComputerWindow, Session } from '../contracts';
import { Dialog } from './Dialog';
import { errorText } from '../locale';

export function useComputer(client: ClientTransport | null) {
  const [status, setStatus] = useState<ComputerStatus>();
  useEffect(() => {
    if (!client) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const value = await client.computerStatus();
        if (alive) setStatus(value);
      } catch {
        if (alive) setStatus(undefined);
      }
      if (alive) timer = setTimeout(() => void poll(), 700);
    };
    void poll();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [client]);
  return status;
}

export function ComputerIndicator({
  client,
  status,
  open,
}: {
  client: ClientTransport;
  status?: ComputerStatus;
  open: () => void;
}) {
  const [sensitive, setSensitive] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const pending = status?.pending;
  useEffect(() => {
    setSensitive(false);
    setError('');
  }, [pending?.id]);
  if (!status?.enabled && !status?.executing) return null;
  if (!status) return null;
  const decide = async (allow: boolean) => {
    if (!pending) return;
    setBusy(true);
    try {
      await client.approveComputer(pending.id, allow, sensitive);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <aside className="computer-indicator" aria-label="Управление компьютером">
      <div className="computer-indicator-heading">
        <button className="text-button" onClick={open}>
          <Monitor size={16} />
          <span>
            Компьютер · {status.windows.length} {status.windows.length === 1 ? 'окно' : 'окна'}
          </span>
        </button>
        <button
          className="danger-button"
          onClick={() => void client.stopComputer().catch((e) => setError(errorText(e)))}
        >
          <Square size={13} />
          Стоп
        </button>
      </div>
      {pending && (
        <div className="computer-confirmation" key={pending.id}>
          <strong>{pending.description}</strong>
          {pending.sensitive && (
            <label className="computer-sensitive">
              <input
                type="checkbox"
                checked={sensitive}
                onChange={(e) => setSensitive(e.target.checked)}
              />
              Разрешаю изменение данных этим действием
            </label>
          )}
          <div className="computer-confirm-actions">
            <button disabled={busy} onClick={() => void decide(false)}>
              <X size={15} />
              Отклонить
            </button>
            <button
              className="primary-button"
              disabled={busy || (pending.sensitive && !sensitive)}
              onClick={() => void decide(true)}
            >
              <Check size={15} />
              Разрешить один раз
            </button>
          </div>
        </div>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </aside>
  );
}

export function ComputerDialog({
  client,
  session,
  status,
  close,
}: {
  client: ClientTransport;
  session?: Session;
  status?: ComputerStatus;
  close: () => void;
}) {
  const [windows, setWindows] = useState<ComputerWindow[]>([]);
  const [selected, setSelected] = useState<string[]>(status?.windows.map((w) => w.id) ?? []);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [consent, setConsent] = useState(false);
  const [notice, setNotice] = useState('');
  const eligible =
    !!session &&
    session.provider === 'openai' &&
    session.chat_mode === 'single' &&
    !session.parent_session_id &&
    !session.role &&
    session.status !== 'running';
  const refresh = async () => {
    setBusy(true);
    setError('');
    try {
      setWindows(await client.computerWindows());
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    void refresh();
  }, [client]);
  const enable = async () => {
    if (!session) return;
    setBusy(true);
    setError('');
    try {
      await client.configureComputer(session.id, selected);
      close();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title="Smart Computer" close={close} wide>
      <div className="computer-panel">
        <div className="computer-intro">
          <Monitor size={28} />
          <div>
            <h3>Компьютер под вашим контролем</h3>
            <p>Локальные инструменты для Codex · ваша подписка ChatGPT</p>
          </div>
        </div>
        <div className="computer-window-heading">
          <strong>Доступные окна</strong>
          <button
            className="icon-button"
            aria-label="Обновить окна"
            title="Обновить окна"
            disabled={busy}
            onClick={() => void refresh()}
          >
            <RefreshCw size={17} />
          </button>
        </div>
        <div className="computer-window-list">
          {windows.map((w) => (
            <label key={w.id} className="computer-window">
              <input
                type="checkbox"
                checked={selected.includes(w.id)}
                disabled={busy || (!selected.includes(w.id) && selected.length >= 8)}
                onChange={(e) =>
                  setSelected((values) =>
                    e.target.checked ? [...values, w.id] : values.filter((id) => id !== w.id),
                  )
                }
              />
              <div>
                <strong>{w.title}</strong>
                <span>
                  {w.application} · {w.bounds.width} × {w.bounds.height} ·{' '}
                  {Math.round((w.dpi / 96) * 100)}%
                </span>
              </div>
            </label>
          ))}
          {!windows.length && (
            <p className="muted">
              {busy
                ? 'Получаю окна…'
                : 'Нет доступных окон. Защищённые и свёрнутые окна исключены.'}
            </p>
          )}
        </div>
        {!eligible && <p className="muted">Выберите остановленный обычный чат Codex.</p>}
        <label className="computer-consent">
          <input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} />
          <div>
            <strong>Разрешить этому чату работать с выбранными окнами</strong>
            <span>
              Каждое чтение и действие потребует подтверждения. Изображения выбранных окон могут
              передаваться Codex. Ctrl+Alt+Pause немедленно останавливает управление.
            </span>
          </div>
        </label>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="computer-panel-actions">
          <span>
            <ShieldCheck size={15} />
            Один агент · локальный Host
          </span>
          {status?.enabled && (
            <button
              className="danger-button"
              onClick={() => void client.stopComputer().catch((e) => setError(errorText(e)))}
            >
              <Square size={13} />
              Остановить
            </button>
          )}
          <button
            className="primary-button"
            disabled={busy || !eligible || !consent || !selected.length}
            onClick={() => void enable()}
          >
            Подключить к чату
          </button>
        </div>
        <details className="computer-metrics">
          <summary>Эффективность и журнал</summary>
          <div className="computer-metric-grid">
            {(
              [
                ['Вызовы', 'tools'],
                ['Кадры', 'screenshots'],
                ['Изображения', 'images_sent'],
                ['Действия', 'actions'],
                ['Проверено', 'verified'],
                ['Контекст, байт', 'context_bytes'],
                ['Изображения, байт', 'image_bytes'],
                ['Время, мс', 'latency_ms'],
              ] as const
            ).map(([title, key]) => (
              <div key={key}>
                <span>{title}</span>
                <strong>{status?.metrics[key] ?? 0}</strong>
              </div>
            ))}
          </div>
          {status?.audit
            .slice(-10)
            .reverse()
            .map((entry, i) => (
              <p key={i}>
                {entry.action ?? entry.tool} · {entry.execution} · {entry.verification} ·{' '}
                {entry.latency_ms} мс
              </p>
            ))}
        </details>
        <details className="computer-metrics">
          <summary>Unity Editor Adapter</summary>
          <p>
            Чтение Hierarchy и Inspector, кадры Scene/Game, правки с Undo. Установка добавит
            исходники в выбранный Unity-проект.
          </p>
          <button
            disabled={busy}
            onClick={() => {
              void (async () => {
                setError('');
                try {
                  const root = await client.chooseFolder();
                  if (!root) return;
                  setBusy(true);
                  await client.installUnityAdapter(root);
                  setNotice(
                    'Установлено. В Unity: Tools → Bebekon → Smart Computer → Enable for this Editor.',
                  );
                } catch (e) {
                  setError(errorText(e));
                } finally {
                  setBusy(false);
                }
              })();
            }}
          >
            Выбрать проект и установить
          </button>
          {notice && <p role="status">{notice}</p>}
        </details>
      </div>
    </Dialog>
  );
}
