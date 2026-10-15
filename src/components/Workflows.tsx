import { useEffect, useState } from 'react';
import type {
  ClientTransport,
  Session,
  QueuedTask,
  Checkpoint,
  CheckpointDiff,
  FileRule,
} from '../contracts';
import { Dialog } from './Dialog';
import { errorText } from '../locale';

const states: Record<string, string> = {
  queued: 'В очереди',
  running: 'Выполняется',
  paused: 'Приостановлено',
  completed: 'Готово',
  cancelled: 'Отменено',
  failed: 'Ошибка',
};
export function Workflows({
  client,
  session,
  close,
  branched,
}: {
  client: ClientTransport;
  session: Session;
  close: () => void;
  branched: (s: Session) => Promise<void>;
}) {
  const [tab, setTab] = useState('queue');
  const [queue, setQueue] = useState<QueuedTask[]>([]);
  const [points, setPoints] = useState<Checkpoint[]>([]);
  const [rules, setRules] = useState<FileRule[]>([]);
  const [prompt, setPrompt] = useState('');
  const [path, setPath] = useState('');
  const [tool, setTool] = useState('Edit');
  const [selected, setSelected] = useState('');
  const [diff, setDiff] = useState<CheckpointDiff[]>([]);
  const [confirm, setConfirm] = useState<CheckpointDiff>();
  const [isolated, setIsolated] = useState(session.workspace_id !== 'chat-scratch');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const idle = session.status !== 'running';
  const refresh = async () => {
    const [q, p] = await Promise.all([client.taskQueue(), client.checkpoints(session.id)]);
    setQueue(q);
    setPoints(p);
    if (session.provider === 'anthropic') setRules(await client.projectFileRules(session.id));
  };
  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError('');
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    let alive = true;
    let unsubscribe: (() => void) | undefined;
    void refresh().catch((e) => alive && setError(errorText(e)));
    void client
      .subscribe(
        (e) => {
          if (
            [
              'queue_changed',
              'turn_started',
              'turn_completed',
              'session_stopped',
              'provider_error',
            ].includes(e.payload.type) &&
            alive
          )
            void refresh().catch((e) => alive && setError(errorText(e)));
        },
        () => {
          if (alive) void refresh().catch((e) => alive && setError(errorText(e)));
        },
      )
      .then((u) => {
        if (alive) unsubscribe = u;
        else u();
      });
    return () => {
      alive = false;
      unsubscribe?.();
    };
    // Management only; no model request is triggered when this dialog opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, session.id]);
  return (
    <Dialog title="Задачи и версии" close={close} wide>
      <div className="workflow-tabs" role="group" aria-label="Разделы задач">
        {(
          [
            ['queue', 'Очередь'],
            ['branch', 'Ветка чата'],
            ['checkpoints', 'Контрольные точки'],
            ['rules', 'Правила Claude'],
          ] as const
        ).map(([id, label]) => (
          <button
            className="secondary-button"
            key={id}
            aria-pressed={tab === id}
            onClick={() => {
              setTab(id);
              setError('');
            }}
          >
            {label}
          </button>
        ))}
      </div>
      {error && (
        <p role="alert" className="notice error">
          {error}
        </p>
      )}
      {tab === 'queue' && (
        <>
          <p className="muted">
            Задачи выполняются последовательно с выбранным аккаунтом и настройками чата. Ошибка или
            остановка приостанавливает остаток очереди. После перезапуска требуется ваше
            продолжение.
          </p>
          <label className="field">
            Задание для этого чата
            <textarea
              aria-label="Задание в очередь"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
            />
          </label>
          <button
            className="primary-button"
            disabled={busy || !prompt.trim()}
            onClick={() =>
              void act(async () => {
                await client.enqueueTask(session.id, prompt);
                setPrompt('');
              })
            }
          >
            Добавить и выполнять по очереди
          </button>
          <button className="text-button" onClick={() => void act(refresh)}>
            Обновить очередь
          </button>
          <ul className="workflow-list">
            {queue.map((t) => (
              <li key={t.id}>
                <strong>{states[t.state] ?? t.state}</strong>
                <p>{t.prompt}</p>
                {t.session_id !== session.id && <small>Другой чат</small>}
                {t.error && <p className="muted">{t.error}</p>}
                {t.state === 'paused' && (
                  <button
                    disabled={busy}
                    className="secondary-button"
                    onClick={() => void act(() => client.changeQueuedTask(t.id, true))}
                  >
                    Продолжить с текущими настройками
                  </button>
                )}
                {['paused', 'queued', 'running'].includes(t.state) && (
                  <button
                    disabled={busy}
                    className="text-button"
                    onClick={() => void act(() => client.changeQueuedTask(t.id, false))}
                  >
                    Отменить задачу
                  </button>
                )}
              </li>
            ))}
          </ul>
          {!queue.length && <p className="muted">Очередь пуста.</p>}
        </>
      )}
      {tab === 'branch' && (
        <>
          <p>
            Независимый чат с текущей историей и тем же аккаунтом. Для Claude и Codex используется
            штатное ветвление сессии, без запроса модели. Для остальных провайдеров — локальная
            передача публичного контекста.
          </p>
          <label className="toggle">
            <input
              type="checkbox"
              checked={isolated}
              onChange={(e) => setIsolated(e.target.checked)}
            />
            Создать отдельный Git worktree с текущими файлами
          </label>
          <p className="muted">
            Незакоммиченные файлы переносятся; игнорируемые файлы, учётные данные и зависимости не
            копируются. Нужен Git-репозиторий с коммитом. Изменения не сливаются автоматически. Без
            worktree записи в общую папку сериализуются.
          </p>
          <button
            className="primary-button"
            disabled={busy || !idle}
            onClick={() =>
              void act(async () => {
                const s = await client.branchChat(session.id, isolated);
                await branched(s);
              })
            }
          >
            Создать ветку
          </button>
        </>
      )}
      {tab === 'checkpoints' && (
        <>
          <p className="muted">
            Локальные контрольные точки текстовых файлов Git-проекта. Создаются перед сообщениями с
            правом записи; не меняют историю разговора или Git. Двоичные, большие, игнорируемые и
            защищённые файлы не покрываются. Новые файлы не удаляются при восстановлении.
          </p>
          <button
            className="secondary-button"
            disabled={busy || !idle}
            onClick={() =>
              void act(() => client.createCheckpoint(session.id, 'Ручная контрольная точка'))
            }
          >
            Сохранить контрольную точку
          </button>
          <ul className="workflow-list">
            {points.map((p) => (
              <li key={p.id}>
                <strong>{p.label}</strong>
                <span>
                  {' '}
                  · {p.file_count} файлов · {new Date(p.created_at * 1000).toLocaleString()}
                  {p.limited ? ' · неполная' : ''}
                </span>
                <button
                  className="text-button"
                  disabled={busy}
                  onClick={() =>
                    void act(async () => {
                      setSelected(p.id);
                      setDiff(await client.checkpointDiff(session.id, p.id));
                    })
                  }
                >
                  Сравнить
                </button>
                <button
                  className="text-button"
                  disabled={busy}
                  onClick={() =>
                    void act(async () => {
                      await client.deleteCheckpoint(session.id, p.id);
                      if (selected === p.id) {
                        setSelected('');
                        setDiff([]);
                      }
                    })
                  }
                >
                  Удалить точку
                </button>
              </li>
            ))}
          </ul>
          {selected && !diff.length && <p>Изменений в сохранённых файлах нет.</p>}
          {diff.map((file) => (
            <details className="checkpoint-file" key={file.path}>
              <summary>{file.path}</summary>
              <div className="checkpoint-comparison">
                <div>
                  <strong>Сохранённая версия</strong>
                  <pre>{file.before ?? 'Файл отсутствует'}</pre>
                </div>
                <div>
                  <strong>Сейчас</strong>
                  <pre>{file.after ?? 'Файл отсутствует'}</pre>
                </div>
              </div>
              <button
                className="secondary-button"
                disabled={busy || !idle}
                onClick={() => setConfirm(file)}
              >
                Восстановить этот файл
              </button>
            </details>
          ))}
          {confirm && (
            <div role="alertdialog" aria-label="Подтверждение восстановления" className="notice">
              <p>
                Заменить текущий файл {confirm.path} сохранённой версией? Перед заменой будет
                сохранена дополнительная контрольная точка.
              </p>
              <button className="secondary-button" onClick={() => setConfirm(undefined)}>
                Отмена
              </button>
              <button
                className="primary-button"
                disabled={busy}
                onClick={() =>
                  void act(async () => {
                    await client.restoreCheckpointFile(
                      session.id,
                      selected,
                      confirm.path,
                      confirm.expected,
                    );
                    setConfirm(undefined);
                    setDiff(await client.checkpointDiff(session.id, selected));
                  })
                }
              >
                Подтвердить восстановление
              </button>
            </div>
          )}
        </>
      )}
      {tab === 'rules' &&
        (session.provider !== 'anthropic' ? (
          <p>В этом разделе доступны постоянные правила файлов для Claude.</p>
        ) : (
          <>
            <p className="muted">
              Точные пути относительно проекта. Правила сохраняются для этого аккаунта и проекта и
              передаются в штатные permissions.allow Claude. Применяются со следующего запуска в
              Manual. Запреты провайдера и защита учётных данных остаются. Shell и MCP здесь не
              разрешаются.
            </p>
            <div className="field-grid">
              <label className="field">
                Инструмент
                <select
                  aria-label="Инструмент правила"
                  value={tool}
                  onChange={(e) => setTool(e.target.value)}
                >
                  <option>Edit</option>
                  <option>Write</option>
                </select>
              </label>
              <label className="field">
                Файл
                <input
                  aria-label="Файл правила"
                  value={path}
                  onChange={(e) => setPath(e.target.value)}
                  placeholder="src/main.rs"
                />
              </label>
            </div>
            <button
              className="primary-button"
              disabled={busy || !idle || !path.trim()}
              onClick={() =>
                void act(async () => {
                  await client.saveProjectFileRules(session.id, [
                    ...rules,
                    { root: session.working_directory, path: path.trim(), tool },
                  ]);
                  setPath('');
                })
              }
            >
              Всегда разрешать этот файл
            </button>
            <ul className="workflow-list">
              {rules.map((rule, index) => (
                <li key={`${rule.tool}:${rule.path}`}>
                  <code>
                    {rule.tool} · {rule.path}
                  </code>
                  <button
                    className="text-button"
                    disabled={busy || !idle}
                    onClick={() =>
                      void act(() =>
                        client.saveProjectFileRules(
                          session.id,
                          rules.filter((_, i) => i !== index),
                        ),
                      )
                    }
                  >
                    Отозвать правило
                  </button>
                </li>
              ))}
            </ul>
          </>
        ))}
    </Dialog>
  );
}
