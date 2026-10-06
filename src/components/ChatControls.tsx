import { useEffect, useState } from 'react';
import { ArrowRightLeft, Puzzle, Settings2 } from 'lucide-react';
import type { AgentConfig, ClientTransport, ModelInfo, Session } from '../contracts';
import { effortLabels, sessionConfig } from '../chat';
import { errorText } from '../locale';

export function ChatControls({
  client,
  session,
  busy,
  configure,
  settings,
  handoff,
}: {
  client: ClientTransport;
  session: Session;
  busy: boolean;
  configure: (config: AgentConfig) => void;
  settings: () => void;
  handoff: () => void;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [error, setError] = useState('');
  useEffect(() => {
    let alive = true;
    setModels([]);
    setError('');
    void client
      .accountModels(session.account_profile_id)
      .then((list) => alive && setModels(list))
      .catch((e) => alive && setError(errorText(e)));
    return () => {
      alive = false;
    };
  }, [client, session.account_profile_id]);
  const model = models.find((m) => m.id === session.model);
  const disabled = busy || session.status === 'running';
  return (
    <div className="chat-controls">
      <div className="chat-control-fields">
        <label>
          Модель
          <select
            aria-label="Модель в чате"
            disabled={disabled || !models.length}
            value={session.model}
            onChange={(e) =>
              configure({
                ...sessionConfig(session),
                model: e.target.value,
                reasoning_effort: null,
              })
            }
          >
            {models.length ? (
              models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))
            ) : (
              <option>{session.model}</option>
            )}
          </select>
        </label>
        <label>
          Рассуждение
          <select
            aria-label="Рассуждение в чате"
            disabled={disabled || !model?.reasoning_efforts?.length}
            value={session.reasoning_effort ?? ''}
            onChange={(e) =>
              configure({ ...sessionConfig(session), reasoning_effort: e.target.value || null })
            }
          >
            <option value="">
              {model?.reasoning_efforts?.length ? 'По умолчанию' : 'Недоступно'}
            </option>
            {model?.reasoning_efforts?.map((e) => (
              <option key={e} value={e}>
                {effortLabels[e] ?? e}
              </option>
            ))}
          </select>
        </label>
        <label>
          Доступ
          <select
            aria-label="Доступ в чате"
            value={session.permission_profile}
            disabled={disabled || !!session.parent_session_id}
            onChange={(e) =>
              configure({ ...sessionConfig(session), permission_profile: e.target.value })
            }
          >
            <option value="standard">Изменения проекта</option>
            <option value="read_only">Только чтение</option>
          </select>
        </label>
        <button
          className="secondary-button"
          title="Настроить плагины, MCP, навыки и роль"
          onClick={settings}
          disabled={disabled}
        >
          <Puzzle size={14} /> Инструменты
        </button>
        {!session.parent_session_id && (
          <button className="secondary-button" onClick={handoff} disabled={disabled}>
            <ArrowRightLeft size={14} /> Перейти
          </button>
        )}
        <button
          className="icon-button"
          aria-label="Настройки агента"
          title="Настройки агента"
          onClick={settings}
          disabled={disabled}
        >
          <Settings2 size={16} />
        </button>
      </div>
      {error && <p className="field-error">{error}</p>}
    </div>
  );
}
