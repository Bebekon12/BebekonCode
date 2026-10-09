import { useEffect, useState } from 'react';
import { ArrowRightLeft, Puzzle } from 'lucide-react';
import type { AgentConfig, ClientTransport, ModelInfo, Session } from '../contracts';
import { sessionConfig } from '../chat';
import { errorText } from '../locale';
import { ModelPicker } from './ModelPicker';
import { AccessPicker } from './AccessPicker';

export function ChatControls({
  client,
  session,
  busy,
  configure,
  settings,
  handoff,
  team = false,
}: {
  client: ClientTransport;
  session: Session;
  busy: boolean;
  configure: (config: AgentConfig) => void;
  settings: () => void;
  handoff: () => void;
  team?: boolean;
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
  const disabled = busy || session.status === 'running';
  return (
    <div className="chat-controls">
      <div className="chat-control-fields">
        {!team && (
          <ModelPicker
            models={models}
            model={session.model}
            effort={session.reasoning_effort}
            disabled={disabled}
            change={(model, reasoning_effort) =>
              configure({ ...sessionConfig(session), model, reasoning_effort })
            }
          />
        )}
        <AccessPicker
          value={session.permission_profile}
          provider={session.provider}
          disabled={disabled}
          change={(permission_profile) =>
            configure({ ...sessionConfig(session), permission_profile })
          }
        />
        <button
          className="secondary-button"
          title="Настроить плагины, MCP, навыки и роль"
          aria-label="Инструменты"
          onClick={settings}
          disabled={disabled}
        >
          <Puzzle size={16} />
        </button>
        {!session.parent_session_id && (
          <button
            className="secondary-button"
            title="Передать чат другому агенту"
            aria-label="Перейти"
            onClick={handoff}
            disabled={disabled}
          >
            <ArrowRightLeft size={16} />
          </button>
        )}
      </div>
      {error && <p className="field-error">{error}</p>}
    </div>
  );
}
