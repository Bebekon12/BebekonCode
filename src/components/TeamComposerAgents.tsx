import { Settings2, ShieldCheck, UsersRound } from 'lucide-react';
import type { Session } from '../contracts';
import { effortLabels } from '../chat';

export function TeamComposerAgents({
  session,
  sessions,
  busy,
  configure,
}: {
  session: Session;
  sessions: Session[];
  busy: boolean;
  configure: (id: string) => void;
}) {
  const agents = [
    session,
    ...sessions.filter((s) => s.parent_session_id === session.id && s.chat_mode !== 'task'),
  ];
  return (
    <div className="composer-team" aria-label="Все участники команды">
      <span className="composer-team-label">
        <UsersRound size={14} /> Команда
      </span>
      <div className="composer-team-members">
        {agents.map((a, i) => (
          <button
            className="composer-agent"
            key={a.id}
            disabled={busy}
            onClick={() => configure(a.id)}
            aria-label={`Настроить ${a.role || `участника ${i + 1}`}: ${a.model}`}
            title="Модель, обдумывание, доступ и инструменты"
          >
            <span className={`provider-mark ${a.provider}`}>
              {a.provider === 'anthropic' ? '✳' : a.provider === 'openai' ? '◎' : '◇'}
            </span>
            <span className="composer-agent-copy">
              <strong>{a.model === 'mock-stream-v1' ? 'Демо' : a.model}</strong>
              <small>
                {i === 0 ? 'Собирает итог' : a.role || `Участник ${i + 1}`} ·{' '}
                {a.reasoning_effort
                  ? (effortLabels[a.reasoning_effort] ?? a.reasoning_effort)
                  : 'Авто'}
              </small>
            </span>
            <span
              className="agent-access"
              title={
                a.permission_profile === 'read_only'
                  ? 'Только чтение'
                  : 'Изменения проекта с подтверждениями'
              }
            >
              <ShieldCheck size={12} />
              {a.permission_profile === 'read_only' ? 'Чтение' : 'Изменения'}
            </span>
            <Settings2 size={13} />
          </button>
        ))}
      </div>
    </div>
  );
}
