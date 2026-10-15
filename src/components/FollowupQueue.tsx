import { useEffect, useState } from 'react';
import type { ClientTransport, QueuedTask } from '../contracts';

export function FollowupQueue({
  client,
  sessionId,
  revision,
}: {
  client: ClientTransport;
  sessionId: string;
  revision: string;
}) {
  const [tasks, setTasks] = useState<QueuedTask[]>([]),
    [error, setError] = useState('');
  const [editing, setEditing] = useState<string>(),
    [text, setText] = useState(''),
    [busy, setBusy] = useState(false);
  const refresh = async () =>
    setTasks(
      (await client.taskQueue()).filter(
        (t) =>
          t.session_id === sessionId && ['queued', 'paused', 'running', 'failed'].includes(t.state),
      ),
    );
  useEffect(() => {
    let active = true;
    const load = () =>
      client
        .taskQueue()
        .then((rows) => {
          if (active)
            setTasks(
              rows.filter(
                (t) =>
                  t.session_id === sessionId &&
                  ['queued', 'paused', 'running', 'failed'].includes(t.state),
              ),
            );
        })
        .catch((e) => {
          if (active) setError(String(e));
        });
    void load();
    const timer = setInterval(load, 2500);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [client, sessionId, revision]);
  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError('');
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  if (!tasks.length && !error) return null;
  return (
    <div className="followup-queue" aria-label="Следующие сообщения">
      {tasks.map((task) => (
        <div key={task.id} className="followup-card">
          <span className="small muted">
            {task.state === 'queued'
              ? 'После текущей задачи'
              : task.state === 'running'
                ? 'Выполняется'
                : task.state === 'paused'
                  ? 'На паузе'
                  : 'Не отправлено'}
          </span>
          {editing === task.id ? (
            <textarea
              aria-label="Изменить сообщение очереди"
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          ) : (
            <p>{task.prompt}</p>
          )}
          {task.attachments && task.attachments !== '[]' && <small>С прикреплёнными файлами</small>}
          {task.error && <p className="small">{task.error}</p>}
          <div className="followup-actions">
            {['queued', 'paused'].includes(task.state) && (
              <>
                <button
                  disabled={busy}
                  onClick={() => {
                    if (editing === task.id) {
                      void act(async () => {
                        await client.editQueuedTask(task.id, text, 0);
                        setEditing(undefined);
                      });
                    } else {
                      setEditing(task.id);
                      setText(task.prompt);
                    }
                  }}
                >
                  {editing === task.id ? 'Сохранить' : 'Изменить'}
                </button>
                <button
                  disabled={busy}
                  aria-label="Выше в очереди"
                  onClick={() => void act(() => client.editQueuedTask(task.id, task.prompt, -1))}
                >
                  ↑
                </button>
                <button
                  disabled={busy}
                  aria-label="Ниже в очереди"
                  onClick={() => void act(() => client.editQueuedTask(task.id, task.prompt, 1))}
                >
                  ↓
                </button>
              </>
            )}
            {task.state === 'paused' && (
              <button
                disabled={busy}
                onClick={() => void act(() => client.changeQueuedTask(task.id, true))}
              >
                Продолжить
              </button>
            )}
            {['queued', 'paused', 'running', 'failed'].includes(task.state) && (
              <button
                disabled={busy}
                onClick={() => void act(() => client.changeQueuedTask(task.id, false))}
              >
                {task.state === 'running' ? 'Остановить' : 'Убрать'}
              </button>
            )}
          </div>
        </div>
      ))}
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
