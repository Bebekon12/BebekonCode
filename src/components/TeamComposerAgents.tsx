import { Settings2, ShieldCheck, UsersRound } from 'lucide-react';
import type { Session } from '../contracts';
import { effortLabels } from '../chat';

export function TeamComposerAgents({
  session,
  sessions,
  busy,
  configure,
  rail = false,
}: {
  session: Session;
  sessions: Session[];
  busy: boolean;
  configure: (id: string) => void;
  /** Wide windows show the roster as a column beside the chat instead of above the composer. */
  rail?: boolean;
}) {
  const agents = [
    session,
    ...sessions.filter((s) => s.parent_session_id === session.id && s.chat_mode !== 'task'),
  ];
  return (
    <div className={`composer-team ${rail ? 'team-rail' : ''}`} aria-label="Все участники команды">
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
              className={`agent-access ${a.permission_profile === 'full_access' ? 'access-danger' : ''}`}
              title={
                a.permission_profile === 'read_only'
                  ? 'Только чтение'
                  : a.permission_profile === 'workspace_auto'
                    ? 'Авто в проекте · дополнительный доступ требует подтверждения'
                    : a.permission_profile === 'full_access'
                      ? 'Полный доступ · без песочницы и подтверждений'
                      : 'Доступ по правилам провайдера'
              }
            >
              <ShieldCheck size={12} />
              {a.permission_profile === 'read_only'
                ? 'Чтение'
                : a.permission_profile === 'workspace_auto'
                  ? 'Авто'
                  : a.permission_profile === 'full_access'
                    ? 'Полный'
                    : 'По правилам'}
            </span>
            <Settings2 size={13} />
          </button>
        ))}
      </div>
    </div>
  );
}
