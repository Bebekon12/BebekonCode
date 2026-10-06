import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
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
  account,
  permissions,
  loadOlder,
  openFiles,
  openTerminal,
  cancel,
  resolveApproval,
  retry,
  chooseAnotherAccount,
  manageUsage,
}: {
  events: AgentEvent[];
  session: Session;
  providerName: string;
  account: string;
  permissions: string;
  loadOlder: () => void;
  openFiles: () => void;
  openTerminal: () => void;
  cancel: () => void;
  resolveApproval: (approvalId: string, decision: ApprovalDecision) => Promise<void>;
  retry: (prompt: string) => void;
  chooseAnotherAccount: () => void;
  manageUsage?: () => void;
}) {
  const demo = session.provider === 'mock';
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
  const [query, setQuery] = useState('');
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const visibleTurns = useMemo(() => filterTimeline(turns, filter, query), [turns, filter, query]);
  const allCollapsed = turns.length > 0 && turns.every((turn) => collapsed.has(turn.id));
  const scroll = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  useEffect(() => {
    if (follow.current) scroll.current?.scrollTo({ top: scroll.current.scrollHeight });
  }, [events]);
  useEffect(() => {
    follow.current = true;
    setCollapsed(new Set());
  }, [session.id]);
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
      <div
        className="timeline"
        ref={scroll}
        onScroll={() => {
          const element = scroll.current;
          if (element)
            follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
        }}
      >
        <ol className="stepper">
          {events.length >= 300 && (
            <li className="stepper-more">
              <button className="text-button" onClick={loadOlder}>
                Загрузить предыдущие события
              </button>
            </li>
          )}
          {overview && (
            <Step
              state="ready"
              title="Сессия запущена"
              subtitle={
                demo ? 'Локальный симулятор готов к работе' : 'Агент подключён и готов к работе'
              }
              time={session.created_at}
            >
              <div className="step-agent-row">
                <span className="step-agent-avatar">
                  <Sparkles size={14} />
                </span>
                <strong>{providerName}</strong>
                <span className="step-badge">Агент</span>
                <span className="step-agent-meta">
                  {session.model} · {account} · Разрешения: {permissions}
                </span>
              </div>
            </Step>
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
              providerName={providerName}
              model={session.model}
              interrupted={interrupted}
              live={running}
              open={!collapsed.has(turn.id)}
              toggle={() => toggle(turn.id)}
              resolveApproval={resolveApproval}
              retry={retry}
              chooseAnotherAccount={chooseAnotherAccount}
              manageUsage={manageUsage}
              last={turn.id === turns[turns.length - 1]?.id}
            />
          ))}
          {overview && (
            <li className={`step step-tail ${running ? 'live' : ''}`}>
              <span className="step-marker" aria-hidden="true" />
              <div className="step-head">
                <div className="step-title">
                  <strong>
                    {running
                      ? 'Агент работает…'
                      : turns.length
                        ? 'Сессия продолжается…'
                        : 'Над чем будем работать?'}
                  </strong>
                  <small>
                    {running && waitingApproval
                      ? 'Ожидает вашего решения по запросу выше'
                      : running
                        ? 'Ответ появляется в реальном времени'
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
  children,
}: {
  state: StepState;
  title: string;
  subtitle: string;
  time: number;
  open?: boolean;
  toggle?: () => void;
  children?: ReactNode;
}) {
  const Icon = stepIcons[state];
  return (
    <li className={`step step-${state}`}>
      <span className="step-marker" aria-hidden="true">
        <Icon size={15} className={state === 'running' ? 'spin' : undefined} />
      </span>
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
      {open && children && <div className="step-body">{children}</div>}
    </li>
  );
}

function TurnStep({
  turn,
  filter,
  demo,
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
}: {
  turn: TimelineTurn;
  filter: TimelineFilter;
  demo: boolean;
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
}) {
  const state: StepState = turn.status === 'running' && interrupted ? 'interrupted' : turn.status;
  const elapsed = turn.finishedAt ? ` · ${duration(turn.finishedAt - turn.startedAt)}` : '';
  const subtitle: Record<StepState, string> = {
    ready: '',
    idle: 'Ожидает',
    running: 'Агент отвечает…',
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
    >
      {filter === 'all' && turn.prompt && (
        <div className="step-message">
          <span className="step-message-avatar">
            <UserRound size={13} />
          </span>
          <div>
            <div className="entry-label">Вы</div>
            <div className="prose">{turn.prompt}</div>
          </div>
        </div>
      )}
      {filter !== 'agent' && turn.activities.length > 0 && (
        <ul className="activity-table" aria-label="Действия инструментов">
          {turn.activities.map((activity, index) => (
            <li key={`${turn.id}-${index}`}>
              <Wrench size={14} className="activity-icon" />
              <span className="activity-label">{activity.label}</span>
              <span className="activity-detail" title={activity.detail}>
                {activity.detail}
              </span>
              {demo && <span className="demo-label">ДЕМО</span>}
              <time>{clock(activity.timestamp)}</time>
            </li>
          ))}
        </ul>
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
      {filter !== 'tools' && (turn.text || turn.status === 'running') && (
        <div className="step-message agent">
          <span className="step-message-avatar">
            <Sparkles size={13} />
          </span>
          <div>
            <div className="entry-label">
              {providerName} <span className="muted">· {model}</span>
            </div>
            <div className="prose">
              {turn.text}
              {turn.status === 'running' && live && <span className="stream-caret" />}
            </div>
          </div>
        </div>
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
  allow_session: 'Разрешено до конца сессии',
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
      {pending && actionable && (
        <div className="approval-actions">
          <button className="secondary-button" disabled={busy} onClick={() => answer('deny')}>
            Отклонить
          </button>
          <button
            className="secondary-button"
            disabled={busy}
            onClick={() => answer('allow_session')}
          >
            Разрешить на сессию
          </button>
          <button className="primary-button" disabled={busy} onClick={() => answer('allow_once')}>
            Разрешить один раз
          </button>
        </div>
      )}
      {pending && !actionable && <p className="muted small">Запрос больше не активен.</p>}
      {error && <p className="notice error">{error}</p>}
    </div>
  );
}
