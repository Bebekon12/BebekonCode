import { useState } from 'react';
import { MessageSquare, Network, Layers, Plus, X, ChevronDown, Settings2 } from 'lucide-react';
import type { AgentConfig, ClientTransport, CreateChat, Snapshot } from '../contracts';
import type { AccountState } from '../accounts';
import { defaultAgent, modeLabels } from '../chat';
import { AgentPicker } from './AgentPicker';
import { Dialog } from './Dialog';

export function NewSession({
  client,
  data,
  initialProject,
  initialMode = 'single',
  openAccounts,
  close,
  create,
  busy,
}: {
  client: ClientTransport;
  data: Snapshot;
  accountState: AccountState;
  initialProject: string;
  initialMode?: CreateChat['mode'];
  initialProvider?: string;
  openAccounts: () => void;
  close: () => void;
  create: (input: CreateChat) => void;
  busy: boolean;
}) {
  const [expanded, setExpanded] = useState(0);
  const [mode, setMode] = useState<CreateChat['mode']>(initialMode);
  const [project, setProject] = useState(initialProject === 'chat-scratch' ? '' : initialProject);
  const [agents, setAgents] = useState<AgentConfig[]>(() =>
    initialMode === 'team'
      ? [
          defaultAgent(data),
          { ...defaultAgent(data), role: 'Рецензент', permission_profile: 'read_only' },
        ]
      : [defaultAgent(data)],
  );
  const valid = agents.every(
    (a) =>
      a.model &&
      data.accounts.some(
        (account) =>
          account.id === a.account_profile_id &&
          (account.provider === 'mock' || account.auth_status === 'signed_in'),
      ),
  );
  const changeMode = (next: CreateChat['mode']) => {
    setExpanded(0);
    setMode(next);
    if (next === 'single') setAgents((current) => current.slice(0, 1));
    else if (next === 'team' && agents.length === 1)
      setAgents((current) => [
        ...current,
        { ...defaultAgent(data), role: 'Рецензент', permission_profile: 'read_only' },
      ]);
  };
  return (
    <Dialog title="Новый чат" close={close} wide>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) create({ workspace_id: project || null, mode, agents });
        }}
      >
        <div className="chat-mode-grid" role="group" aria-label="Режим чата">
          {(
            [
              ['single', MessageSquare, 'Один агент. Модель и доступ можно менять в чате.'],
              ['team', Network, 'Участники выполняют задачу. Вы получаете один общий итог.'],
              ['auto', Layers, 'ИИ разделяет задачу на подзадачи и собирает общий результат.'],
            ] as const
          ).map(([id, Icon, description]) => (
            <button
              type="button"
              key={id}
              className={`chat-mode ${mode === id ? 'selected' : ''}`}
              aria-pressed={mode === id}
              onClick={() => changeMode(id)}
            >
              <Icon size={20} />
              <strong>{modeLabels[id]}</strong>
              <span>{description}</span>
            </button>
          ))}
        </div>
        <label className="field">
          Проект · необязательно
          <select aria-label="Проект" value={project} onChange={(e) => setProject(e.target.value)}>
            <option value="">Без проекта · обычный разговор</option>
            {data.workspaces
              .filter((w) => w.id !== 'chat-scratch')
              .map((w) => (
                <option key={w.id} value={w.id}>
                  {w.name}
                </option>
              ))}
          </select>
        </label>
        <div className="chat-agent-list">
          {agents.map((agent, index) => (
            <section className="chat-agent-card" key={index}>
              <div className="chat-agent-heading">
                <button
                  type="button"
                  className="agent-card-toggle"
                  aria-expanded={expanded === index}
                  onClick={() => setExpanded(expanded === index ? -1 : index)}
                >
                  <span className={`provider-mark ${agent.provider}`}>
                    {agent.provider === 'anthropic' ? '✳' : agent.provider === 'openai' ? '◎' : '◇'}
                  </span>
                  <span>
                    <strong>{index === 0 ? 'Основной агент' : `Участник ${index + 1}`}</strong>
                    <small>
                      {agent.model === 'mock-stream-v1' ? 'Демо' : agent.model || 'Выберите модель'}{' '}
                      · {agent.role || 'Без роли'}
                    </small>
                  </span>
                  <Settings2 size={15} />
                  <ChevronDown size={14} />
                </button>
                {index > 0 && (
                  <button
                    type="button"
                    className="icon-button"
                    aria-label={`Удалить участника ${index + 1}`}
                    onClick={() => setAgents((current) => current.filter((_, i) => i !== index))}
                  >
                    <X size={14} />
                  </button>
                )}
              </div>
              <div hidden={expanded !== index}>
                <AgentPicker
                  client={client}
                  data={data}
                  value={agent}
                  worker={index > 0}
                  change={(value) =>
                    setAgents((current) => current.map((a, i) => (i === index ? value : a)))
                  }
                />
              </div>
            </section>
          ))}
        </div>
        {mode !== 'single' && (
          <p className="small muted">
            До двух подзадач параллельно. Участники читают проект; основной агент проверяет
            результаты и вносит изменения.{' '}
            {mode === 'auto'
              ? 'Каждый запрос создаёт до четырёх контекстов.'
              : 'Команда проводит один раунд взаимного обсуждения.'}
          </p>
        )}
        {mode !== 'single' && agents.length < 3 && (
          <button
            type="button"
            className="secondary-button"
            onClick={() => {
              setExpanded(agents.length);
              setAgents((current) => [
                ...current,
                { ...defaultAgent(data), permission_profile: 'read_only', role: 'Исследователь' },
              ]);
            }}
          >
            <Plus size={14} /> Добавить участника
          </button>
        )}
        <div className="dialog-footer">
          <button type="button" className="text-button" onClick={openAccounts}>
            Подключить аккаунт
          </button>
          <button type="button" className="secondary-button" onClick={close}>
            Отмена
          </button>
          <button
            className="primary-button"
            disabled={busy || !valid || (mode === 'team' && agents.length < 2)}
          >
            Создать чат
          </button>
        </div>
      </form>
    </Dialog>
  );
}
