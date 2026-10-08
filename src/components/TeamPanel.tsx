import { ChevronDown, ExternalLink, Network, Settings2, UsersRound } from 'lucide-react';
import { useEffect, useRef } from 'react';
import type { Session } from '../contracts';
import { counted, sessionTitle, statusLabels } from '../locale';
import { effortLabels } from '../chat';

export function TeamPanel({
  session,
  sessions,
  configure,
  select,
}: {
  session: Session;
  sessions: Session[];
  configure: (session: Session) => void;
  select: (session: Session) => void;
}) {
  const panel = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (
        event.target instanceof Element &&
        !event.target.closest('[role="dialog"]') &&
        !panel.current?.contains(event.target) &&
        panel.current
      )
        panel.current.open = false;
    };
    const escape = (event: KeyboardEvent) => {
      if (
        event.key === 'Escape' &&
        panel.current?.open &&
        !(event.target instanceof Element && event.target.closest('[role="dialog"]'))
      ) {
        panel.current.open = false;
        panel.current.querySelector('summary')?.focus();
      }
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [session.id]);
  const members = [
    session,
    ...sessions.filter((s) => s.parent_session_id === session.id && s.chat_mode !== 'task'),
  ];
  const tasks = sessions.filter(
    (s) => s.parent_session_id === session.id && s.chat_mode === 'task',
  );
  const running = session.status === 'running';
  const Icon = session.chat_mode === 'auto' ? Network : UsersRound;
  return (
    <details className="team-panel" key={session.id} ref={panel}>
      <summary>
        <Icon size={17} />
        <span>
          {session.chat_mode === 'auto' ? 'Авторазбиение' : 'Команда'} ·{' '}
          {counted(members.length, ['агент', 'агента', 'агентов'])}
        </span>
        <ChevronDown size={14} />
      </summary>
      <div className="team-panel-body">
        <p className="muted small">
          Участники выполняют свои части задачи. Основной агент проверяет результаты и готовит один
          общий ответ.
        </p>
        <div className="team-member-list">
          {members.map((member, index) => (
            <div className="team-member" key={member.id}>
              <div>
                <strong>
                  {member.role || (index === 0 ? 'Основной агент' : `Участник ${index + 1}`)}
                </strong>
                <span>
                  {member.provider === 'mock' ? 'Локальное демо' : member.model}
                  {member.reasoning_effort
                    ? ` · ${effortLabels[member.reasoning_effort] ?? member.reasoning_effort}`
                    : ''}
                </span>
                <small>
                  {index === 0
                    ? 'Проверяет и собирает итог'
                    : member.permission_profile === 'read_only'
                      ? 'Анализирует проект · только чтение'
                      : member.permission_profile === 'workspace_auto'
                        ? 'Изменяет проект · автоправки'
                        : 'Изменяет проект · с подтверждениями'}
                </small>
              </div>
              <button
                className="secondary-button"
                disabled={running}
                aria-label={`Настроить ${member.role || member.model}`}
                onClick={() => configure(member)}
              >
                <Settings2 size={15} /> Настроить
              </button>
            </div>
          ))}
        </div>
        <details className="team-task-details">
          <summary>Результаты участников · {tasks.length}</summary>
          {!tasks.length && (
            <p className="muted small">Подзадачи появятся после отправки сообщения.</p>
          )}
          {tasks.map((task) => (
            <button
              className="team-task-row"
              key={task.id}
              onClick={() => select(task)}
              title="Открыть результат в отдельном контексте"
            >
              <span className={`status-dot ${task.status}`} />
              <span>{sessionTitle(task.title)}</span>
              <small>{statusLabels[task.status]}</small>
              <ExternalLink size={14} />
            </button>
          ))}
        </details>
      </div>
    </details>
  );
}
