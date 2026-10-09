import { ChatMarkdown } from './ChatMarkdown';
import { RunChanges, type ChangeSource } from './RunChanges';
import { ThinkingIndicator } from './ThinkingIndicator';
import { effortLabels } from '../chat';
import { attachmentHint } from '../attachments';
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import {
  AlertTriangle,
  Ban,
  Check,
  ShieldQuestion,
  ChevronDown,
  CircleStop,
  FolderOpen,
  ListChevronsDownUp,
  ListChevronsUpDown,
  ListFilter,
  LoaderCircle,
  MessageSquareText,
  Play,
  Search,
  Sparkles,
  Terminal,
  UserRound,
  Wrench,
} from 'lucide-react';
import { buildTimeline, filterTimeline, type TimelineFilter, type TimelineTurn } from '../timeline';
import type { AgentEvent, ApprovalDecision, Session, SessionStatus } from '../contracts';
import type { TimelineApproval } from '../timeline';
import { clock, demoActivity, demoResponse, duration } from '../locale';

type StepState = SessionStatus | 'ready';
const stepIcons: Record<StepState, typeof Check> = {
  ready: Play,
  idle: MessageSquareText,
  running: LoaderCircle,
  completed: Check,
  stopped: CircleStop,
  failed: AlertTriangle,
  interrupted: AlertTriangle,
};
const pillLabels: Record<StepState, string> = {
  ready: 'Готово',
  idle: 'Ожидает',
  running: 'Выполняется',
  completed: 'Завершено',
  stopped: 'Остановлено',
  failed: 'Ошибка',
  interrupted: 'Прервано',
};
const filters: { id: TimelineFilter; label: string; icon: typeof Check }[] = [
  { id: 'all', label: 'Все события', icon: ListFilter },
  { id: 'agent', label: 'Действия агента', icon: Sparkles },
  { id: 'tools', label: 'Инструменты', icon: Wrench },
];

export function Timeline({
  events,
  session,
  providerName,
  historyLoading = false,
  openFiles,
  openTerminal,
  cancel,
  resolveApproval,
  retry,
  chooseAnotherAccount,
  manageUsage,
  changes,
}: {
  events: AgentEvent[];
  session: Session;
  providerName: string;
  /** Older pages of the stored history are still being loaded. */
  historyLoading?: boolean;
  openFiles: () => void;
  openTerminal: () => void;
  cancel: () => void;
  resolveApproval: (approvalId: string, decision: ApprovalDecision) => Promise<void>;
  retry: (prompt: string) => void;
  chooseAnotherAccount: () => void;
  manageUsage?: () => void;
  /** Project change summary under the latest finished answer; absent without a Git project. */
  changes?: ChangeSource;
}) {
  const demo = session.provider === 'mock';
  const team =
    !session.parent_session_id && (session.chat_mode === 'team' || session.chat_mode === 'auto');
  const turns = useMemo(
    () =>
      buildTimeline(events).map((turn) =>
        demo
          ? {
              ...turn,
              text: demoResponse(turn.text),
              error: turn.error && demoActivity(turn.error),
              activities: turn.activities.map((activity) => ({
                ...activity,
                label: demoActivity(activity.label),
                detail: demoActivity(activity.detail),
              })),
            }
          : turn,
      ),
    [events, demo],
  );
  const [filter, setFilter] = useState<TimelineFilter>('all');
  const [showDetails, setShowDetails] = useState(false);
  const [query, setQuery] = useState('');
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const visibleTurns = useMemo(() => filterTimeline(turns, filter, query), [turns, filter, query]);
  const allCollapsed = turns.length > 0 && turns.every((turn) => collapsed.has(turn.id));
  const scroll = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const lastHeight = useRef(0);
  const firstSequence = useRef<number | undefined>(undefined);
  const [activeTurn, setActiveTurn] = useState('');
  useLayoutEffect(() => {
    const element = scroll.current;
    if (!element) return;
    const first = events[0]?.sequence;
    // Older history arrived above the reader: keep the same text in view instead of jumping.
    const prepended =
      first !== undefined && firstSequence.current !== undefined && first < firstSequence.current;
    if (follow.current) element.scrollTo({ top: events.length ? element.scrollHeight : 0 });
    else if (prepended) element.scrollTop += element.scrollHeight - lastHeight.current;
    firstSequence.current = first;
    lastHeight.current = element.scrollHeight;
  }, [events]);
  useEffect(() => {
    follow.current = true;
    firstSequence.current = undefined;
    setCollapsed(new Set());
    setActiveTurn('');
  }, [session.id]);
  const questions = turns.filter((turn) => turn.prompt.trim());
  // Marks the question whose step is at the top of the viewport.
  const trackActive = () => {
    const element = scroll.current;
    if (!element) return;
    const top = element.getBoundingClientRect().top + 24;
    let current = '';
    for (const step of element.querySelectorAll<HTMLElement>('[data-turn-id]')) {
      if (step.getBoundingClientRect().top <= top) current = step.dataset.turnId ?? current;
      else break;
    }
    setActiveTurn(current || questions[0]?.id || '');
  };
  const jumpTo = (id: string) => {
    follow.current = false;
    // A collapsed or filtered-out question must become visible before scrolling to it.
    setCollapsed((current) => {
      if (!current.has(id)) return current;
      const next = new Set(current);
      next.delete(id);
      return next;
    });
    if (!visibleTurns.some((turn) => turn.id === id)) {
      setFilter('all');
      setQuery('');
    }
    requestAnimationFrame(() => {
      scroll.current
        ?.querySelector(`[data-turn-id="${CSS.escape(id)}"]`)
        ?.scrollIntoView({ block: 'start', behavior: 'smooth' });
      setActiveTurn(id);
    });
  };
  const toggle = (id: string) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const running = session.status === 'running';
  const interrupted = session.status === 'interrupted';
  const waitingApproval = turns.some((turn) =>
    turn.approvals.some((approval) => !approval.decision),
  );
  const overview = filter === 'all' && !query.trim();
  return (
    <>
      <div className="chat-history-switch">
        <button
          className="text-button"
          aria-label="Поиск и фильтры истории"
          aria-expanded={showDetails}
          onClick={() => {
            setShowDetails(!showDetails);
            setFilter('all');
          }}
        >
          {' '}
          <Search size={14} /> {showDetails ? 'Скрыть фильтры' : 'История'}{' '}
          <ChevronDown size={12} />
        </button>
      </div>
      {showDetails && (
        <div className="event-toolbar" role="toolbar" aria-label="Фильтры событий">
          {filters.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={filter === id ? 'active' : ''}
              aria-pressed={filter === id}
              onClick={() => setFilter(id)}
            >
              <Icon size={14} /> {label}
            </button>
          ))}
          <span className="toolbar-divider" aria-hidden="true" />
          <button onClick={openFiles}>
            <FolderOpen size={14} /> Файлы
          </button>
          <button onClick={openTerminal}>
            <Terminal size={14} /> Терминал
          </button>
          <label className="event-search">
            <Search size={14} />
            <input
              aria-label="Поиск по событиям"
              placeholder="Поиск по событиям…"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <button
            className="icon-button toolbar-square"
            aria-label={allCollapsed ? 'Развернуть все этапы' : 'Свернуть все этапы'}
            title={allCollapsed ? 'Развернуть все этапы' : 'Свернуть все этапы'}
            disabled={!turns.length}
            onClick={() =>
              setCollapsed(allCollapsed ? new Set() : new Set(turns.map((turn) => turn.id)))
            }
          >
            {allCollapsed ? <ListChevronsUpDown size={16} /> : <ListChevronsDownUp size={16} />}
          </button>
        </div>
      )}
      <div
        className="timeline chat-timeline"
        ref={scroll}
        onScroll={() => {
          const element = scroll.current;
          if (element)
            follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
          trackActive();
        }}
      >
        <ol className="stepper">
          {historyLoading && (
            <li className="stepper-more" role="status">
              <ThinkingIndicator label="Загружаем раннюю историю чата…" />
            </li>
          )}
          {overview && !turns.length && (
            <li className="chat-empty">
              <h2>Над чем поработаем?</h2>
              <p>
                {team
                  ? 'Опишите задачу. Команда подготовит один общий ответ; результаты участников доступны в меню команды.'
                  : 'Опишите задачу или задайте вопрос. Настройки агента — рядом с полем ввода.'}
              </p>
              {demo && (
                <p className="small muted">
                  Сейчас включён локальный симулятор. Для ответов ИИ подключите аккаунт.
                </p>
              )}
            </li>
          )}
          {turns.length > 0 && visibleTurns.length === 0 && (
            <li className="stepper-empty">Подходящих событий нет.</li>
          )}
          {visibleTurns.map((turn) => (
            <TurnStep
              key={turn.id}
              turn={turn}
              filter={filter}
              demo={demo}
              team={team}
              providerName={
                turn.provider
                  ? ({ openai: 'OpenAI / Codex', mock: 'Локальное демо', anthropic: 'Claude Code' }[
                      turn.provider
                    ] ?? turn.provider)
                  : providerName
              }
              model={turn.model ?? session.model}
              interrupted={interrupted}
              live={running}
              open={!collapsed.has(turn.id)}
              toggle={() => toggle(turn.id)}
              resolveApproval={resolveApproval}
              retry={retry}
              chooseAnotherAccount={chooseAnotherAccount}
              manageUsage={manageUsage}
              last={turn.id === turns[turns.length - 1]?.id}
              changes={running ? undefined : changes}
            />
          ))}
          {overview && (running || interrupted) && (
            <li className={`step step-tail ${running ? 'live' : ''}`}>
              <span className="step-marker" aria-hidden="true" />
              <div className="step-head">
                <div className="step-title">
                  <strong>
                    {running ? (
                      <ThinkingIndicator
                        label={
                          waitingApproval
                            ? 'Ждёт подтверждения…'
                            : team
                              ? 'Команда работает…'
                              : 'Агент работает…'
                        }
                      />
                    ) : interrupted ? (
                      'Сессия продолжается…'
                    ) : (
                      'Над чем будем работать?'
                    )}
                  </strong>
                  <small>
                    {running && waitingApproval
                      ? 'Ожидает вашего решения по запросу выше'
                      : running
                        ? team
                          ? 'Участники выполняют задачу; здесь появится общий итог.'
                          : 'Ответ появляется в реальном времени'
                        : interrupted
                          ? 'Предыдущий запуск прерван при остановке ядра. История сохранена — отправьте новое сообщение, чтобы продолжить.'
                          : turns.length
                            ? 'Продолжайте общение с агентом'
                            : demo
                              ? 'Отправьте задачу, чтобы попробовать симулятор. Файлы не будут прочитаны или изменены.'
                              : 'Опишите задачу в поле ниже'}
                  </small>
                </div>
                {running && (
                  <button className="secondary-button step-stop" onClick={cancel}>
                    <CircleStop size={13} /> Остановить
                  </button>
                )}
              </div>
            </li>
          )}
        </ol>
      </div>
      {questions.length > 1 && (
        <nav className="question-nav" aria-label="Переход к вопросам">
          {questions.map((turn, index) => (
            <button
              key={turn.id}
              className={turn.id === activeTurn ? 'active' : ''}
              aria-current={turn.id === activeTurn ? 'true' : undefined}
              aria-label={`Вопрос ${index + 1}: ${turn.prompt.slice(0, 120)}`}
              onClick={() => jumpTo(turn.id)}
            >
              <span className="question-nav-label">{turn.prompt}</span>
              <i aria-hidden="true" />
            </button>
          ))}
        </nav>
      )}
    </>
  );
}

function Step({
  state,
  title,
  subtitle,
  time,
  open = true,
  toggle,
  anchor,
  children,
}: {
  state: StepState;
  title: string;
  subtitle: string;
  time: number;
  open?: boolean;
  toggle?: () => void;
  /** Turn id used by the question navigation to find this step. */
  anchor?: string;
  children?: ReactNode;
}) {
  const Icon = stepIcons[state];
  return (
    <li className={`step step-${state}`} data-turn-id={anchor}>
      <span className="step-marker" aria-hidden="true">
        <Icon size={15} className={state === 'running' ? 'spin' : undefined} />
      </span>
      {children && <div className="step-body">{children}</div>}
      <div className="step-head">
        <div className="step-title">
          <strong title={title}>{title}</strong>
          <small>{subtitle}</small>
        </div>
        <time dateTime={new Date(time * 1000).toISOString()}>{clock(time)}</time>
        <span className={`step-pill ${state}`}>
          {(state === 'completed' || state === 'ready') && <Check size={11} />}
          {pillLabels[state]}
        </span>
        {toggle ? (
          <button
            className={`icon-button step-toggle ${open ? 'open' : ''}`}
            aria-expanded={open}
            aria-label={open ? 'Свернуть этап' : 'Развернуть этап'}
            onClick={toggle}
          >
            <ChevronDown size={15} />
          </button>
        ) : (
          <span className="step-toggle-space" />
        )}
      </div>
    </li>
  );
}

function TurnStep({
  turn,
  filter,
  demo,
  team,
  providerName,
  model,
  interrupted,
  live,
  open,
  toggle,
  resolveApproval,
  retry,
  chooseAnotherAccount,
  manageUsage,
  last,
  changes,
}: {
  turn: TimelineTurn;
  filter: TimelineFilter;
  demo: boolean;
  team: boolean;
  providerName: string;
  model: string;
  interrupted: boolean;
  live: boolean;
  open: boolean;
  toggle: () => void;
  resolveApproval: (approvalId: string, decision: ApprovalDecision) => Promise<void>;
  retry: (prompt: string) => void;
  chooseAnotherAccount: () => void;
  manageUsage?: () => void;
  last: boolean;
  changes?: ChangeSource;
}) {
  const state: StepState = turn.status === 'running' && interrupted ? 'interrupted' : turn.status;
  const elapsed = turn.finishedAt ? ` · ${duration(turn.finishedAt - turn.startedAt)}` : '';
  const subtitle: Record<StepState, string> = {
    ready: '',
    idle: 'Ожидает',
    running: team ? 'Команда готовит общий ответ…' : 'Агент отвечает…',
    completed: `Ответ завершён${elapsed}`,
    stopped: `Остановлено вами${elapsed}`,
    failed: `Ошибка провайдера${elapsed}`,
    interrupted: 'Прервано при остановке ядра',
  };
  return (
    <Step
      state={state}
      title={turn.prompt || 'Запрос'}
      subtitle={subtitle[state]}
      time={turn.startedAt}
      open={open}
      toggle={toggle}
      anchor={turn.id}
    >
      {filter === 'all' && turn.prompt && (
        <div className="step-message">
          <span className="step-message-avatar">
            <UserRound size={13} />
          </span>
          <div>
            <div className="entry-label">Вы</div>
            <div className="prose">{turn.prompt}</div>
            {turn.attachments.length > 0 && (
              <div className="sent-attachments">
                {turn.attachments.map((file) => (
                  <span key={file.path} title={`${attachmentHint(file)}\n${file.path}`}>
                    {file.name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
      {open && filter !== 'agent' && turn.activities.length > 0 && (
        <details
          className="chat-activity-details"
          open={filter === 'tools' || (live && turn.status === 'running') ? true : undefined}
        >
          <summary>
            {team ? 'Работа команды' : 'Действия агента'} · {turn.activities.length}
          </summary>
          <ul className="activity-table" aria-label="Действия инструментов">
            {turn.activities.map((activity, index) => (
              <li key={`${turn.id}-${index}`}>
                <Wrench size={14} className="activity-icon" />
                <span className="activity-label">{activity.label}</span>
                <details className="activity-detail">
                  <summary>{activity.detail.slice(0, 100)}</summary>
                  <pre>{activity.detail}</pre>
                </details>
                {demo && <span className="demo-label">ДЕМО</span>}
                <time>{clock(activity.timestamp)}</time>
              </li>
            ))}
          </ul>
        </details>
      )}
      {open && filter !== 'tools' && (turn.progress.trim() || turn.teamMessages.length > 0) && (
        <details className="progress-details" open>
          <summary>
            <Sparkles size={14} /> {team ? 'Обсуждение команды' : 'Ход работы'}
          </summary>
          {turn.progress.trim() && (
            <div className="progress-copy">
              <ChatMarkdown text={turn.progress} streaming={live && turn.status === 'running'} />
            </div>
          )}
          {turn.teamMessages.map((message) => (
            <div
              className="team-transcript-message"
              key={`${message.sessionId}:${message.stageId}`}
            >
              <strong>{message.title}</strong>
              {message.model && (
                <small>
                  {message.model} · Обдумывание:{' '}
                  {message.reasoningEffort
                    ? `${effortLabels[message.reasoningEffort] ?? message.reasoningEffort} (запрошено)`
                    : 'настройка CLI'}
                </small>
              )}
              <ChatMarkdown text={message.text} streaming={live && turn.status === 'running'} />
            </div>
          ))}
        </details>
      )}
      {filter !== 'agent' &&
        turn.approvals.map((approval) => (
          <ApprovalCard
            key={approval.id}
            approval={approval}
            actionable={live && turn.status === 'running'}
            resolve={(decision) => resolveApproval(approval.id, decision)}
          />
        ))}
      {open && team && !turn.text && turn.status === 'running' && live && filter !== 'tools' && (
        <div className="team-progress" role="status">
          <ThinkingIndicator
            label={
              turn.activities.some((activity) => activity.label === 'Сборка результата')
                ? 'Собираем общий ответ…'
                : turn.activities.some((activity) => activity.label === 'Обсуждение в команде')
                  ? 'Участники проверяют результаты…'
                  : 'Команда выполняет задачу…'
            }
          />
        </div>
      )}
      {open && filter !== 'tools' && (turn.text || (!team && turn.status === 'running')) && (
        <div className="step-message agent">
          <span className="step-message-avatar">
            <Sparkles size={13} />
          </span>
          <div>
            <div className="entry-label">
              {team ? 'Ответ команды' : providerName}{' '}
              {!demo && (
                <span className="muted">
                  · {model} ·{' '}
                  {turn.reasoningEffort
                    ? `${effortLabels[turn.reasoningEffort] ?? turn.reasoningEffort} (запрошено)`
                    : 'Уровень по умолчанию CLI'}
                </span>
              )}
            </div>
            <div className="prose">
              {turn.text ? (
                <ChatMarkdown text={turn.text} streaming={turn.status === 'running' && live} />
              ) : (
                <ThinkingIndicator />
              )}
            </div>
          </div>
        </div>
      )}
      {last && changes && turn.status !== 'running' && filter !== 'tools' && (
        <RunChanges source={changes} refreshKey={`${turn.id}:${turn.finishedAt ?? ''}`} />
      )}
      {turn.error && turn.errorKind === 'usage_limit' ? (
        <div role="alert" className="limit-notice">
          <strong>Аккаунт достиг текущего лимита провайдера</strong>
          <p>{turn.error}</p>
          <p className="muted small">
            BebekonCode не переключает аккаунты автоматически. Повторите позже или начните новую
            сессию с другим аккаунтом.
          </p>
          {last && (
            <div className="limit-actions">
              {manageUsage && (
                <button className="primary-button" onClick={manageUsage}>
                  Управлять использованием
                </button>
              )}
              <button className="secondary-button" onClick={() => retry(turn.prompt)}>
                Повторить
              </button>
              <button className="secondary-button" onClick={chooseAnotherAccount}>
                Выбрать другой аккаунт вручную
              </button>
            </div>
          )}
        </div>
      ) : (
        turn.error && (
          <div role="alert" className="notice error">
            {turn.error}
          </div>
        )
      )}
    </Step>
  );
}

const decisionLabels: Record<string, string> = {
  allow_once: 'Разрешено один раз',
  allow_session: 'Разрешено для этого действия в контексте агента',
  deny: 'Отклонено',
  expired: 'Запрос закрыт без ответа',
};

function ApprovalCard({
  approval,
  actionable,
  resolve,
}: {
  approval: TimelineApproval;
  actionable: boolean;
  resolve: (decision: ApprovalDecision) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const answer = (decision: ApprovalDecision) => {
    setBusy(true);
    setError('');
    void resolve(decision)
      .catch((reason: unknown) =>
        setError(reason instanceof Error ? reason.message : String(reason)),
      )
      .finally(() => setBusy(false));
  };
  const pending = !approval.decision;
  const offered = (decision: ApprovalDecision) =>
    approval.available_decisions == null || approval.available_decisions.includes(decision);
  return (
    <div
      className={`approval-card ${pending ? 'pending' : 'resolved'}`}
      role={pending ? 'alertdialog' : undefined}
      aria-label={approval.title}
    >
      <div className="approval-title">
        {pending ? (
          <ShieldQuestion size={16} />
        ) : approval.decision === 'deny' || approval.decision === 'expired' ? (
          <Ban size={15} />
        ) : (
          <Check size={15} />
        )}
        <strong>{approval.title}</strong>
        {!pending && <span className="muted">{decisionLabels[approval.decision ?? '']}</span>}
      </div>
      <pre className="approval-detail">{approval.detail}</pre>
      {approval.cwd && (
        <p className="approval-meta">
          Папка: <code>{approval.cwd}</code>
        </p>
      )}
      {approval.reason && <p className="approval-meta">Причина: {approval.reason}</p>}
      {pending && (
        <p className="approval-meta">
          Подтверждение относится к этому запросу агента. Разрешение на контекст не включает
          остальные команды и других участников команды. Авторежим работает в границах песочницы.
        </p>
      )}
      {pending && actionable && (
        <div className="approval-actions">
          {offered('deny') && (
            <button className="secondary-button" disabled={busy} onClick={() => answer('deny')}>
              Отклонить
            </button>
          )}
          {offered('allow_session') && (
            <button
              className="secondary-button"
              disabled={busy}
              onClick={() => answer('allow_session')}
            >
              Разрешить в этом контексте
            </button>
          )}
          {offered('allow_once') && (
            <button className="primary-button" disabled={busy} onClick={() => answer('allow_once')}>
              Разрешить один раз
            </button>
          )}
        </div>
      )}
      {pending && approval.available_decisions?.length === 0 && (
        <p className="notice">
          CLI предлагает неподдерживаемое решение. Остановите задачу и обновите Codex CLI.
        </p>
      )}
      {pending && !actionable && <p className="muted small">Запрос больше не активен.</p>}
      {error && <p className="notice error">{error}</p>}
    </div>
  );
}
