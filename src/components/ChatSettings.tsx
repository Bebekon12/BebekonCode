import { useState } from 'react';
import type { AgentConfig, ClientTransport, Session, Snapshot } from '../contracts';
import { sessionConfig } from '../chat';
import { AgentPicker } from './AgentPicker';
import { Dialog } from './Dialog';

export function ChatSettings({
  client,
  data,
  session,
  handoff,
  close,
  save,
  busy,
  initialToolsOpen = false,
}: {
  client: ClientTransport;
  data: Snapshot;
  session: Session;
  handoff: boolean;
  close: () => void;
  save: (config: AgentConfig, transition: boolean) => void;
  busy: boolean;
  initialToolsOpen?: boolean;
}) {
  const [value, setValue] = useState(() => sessionConfig(session));
  const changedAccount =
    value.provider !== session.provider || value.account_profile_id !== session.account_profile_id;
  const valid =
    value.model &&
    data.accounts.some(
      (a) =>
        a.id === value.account_profile_id &&
        (a.provider === 'mock' || a.auth_status === 'signed_in'),
    );
  return (
    <Dialog title={handoff ? 'Перейти с контекстом' : 'Настройки агента'} close={close}>
      <p className="muted dialog-description">
        {handoff
          ? 'История передаётся выбранному агенту локально, даже при исчерпанном лимите.'
          : session.chat_mode === 'task'
            ? 'Подзадача: настройки меняются в составе команды.'
            : 'Применится со следующего сообщения.'}
      </p>
      <AgentPicker
        client={client}
        data={data}
        value={value}
        change={setValue}
        lockedAccount={!handoff}
        participant={!!session.parent_session_id}
        worker={data.sessions.some(
          (s) => s.id === session.parent_session_id && s.chat_mode === 'auto',
        )}
        initialToolsOpen={initialToolsOpen}
      />
      <div className="dialog-footer">
        <button className="secondary-button" onClick={close}>
          Отмена
        </button>
        <button
          className="primary-button"
          disabled={busy || !valid || session.status === 'running' || session.chat_mode === 'task'}
          onClick={() => save(value, handoff || changedAccount)}
        >
          {handoff ? 'Перейти с контекстом' : 'Сохранить'}
        </button>
      </div>
    </Dialog>
  );
}
