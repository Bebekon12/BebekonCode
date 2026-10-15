import { ChatMarkdown } from './ChatMarkdown';
import { RunChanges } from './RunChanges';
import { UsageDiagnostics } from './UsageDiagnostics';
import { ThinkingIndicator } from './ThinkingIndicator';
import { ImagePreview } from './ImagePreview';
import { LiveStatus, TodoCard, ToolGroup } from './TurnParts';
import { effortLabels } from '../chat';
import { attachmentHint } from '../attachments';
import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import {
  Ban,
  Check,
  ShieldQuestion,
  ChevronDown,
  FolderOpen,
  ListChevronsDownUp,
  ListChevronsUpDown,
  ListFilter,
  Search,
  Sparkles,
  Terminal,
  Wrench,
  X,
} from 'lucide-react';
import { buildTimeline, filterTimeline, type TimelineFilter, type TimelineTurn } from '../timeline';
import type { AgentEvent, ApprovalDecision, Session, SessionStatus } from '../contracts';
import type { TimelineApproval } from '../timeline';
import { demoActivity, demoResponse, duration, errorText } from '../locale';

type StepState = SessionStatus | 'ready';
/** Turns mounted at once; earlier ones mount in pages of this size while scrolling up. */
const renderPage = 40;
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
  resolveApproval,
  retry,
  chooseAnotherAccount,
  manageUsage,
  loadImage,
}: {
  events: AgentEvent[];
  session: Session;
  providerName: string;
  /** Older pages of the stored history are still being loaded. */
  historyLoading?: boolean;
  openFiles: () => void;
  openTerminal: () => void;
  /** Stopping lives in the composer; kept for callers. */
  cancel?: () => void;
  resolveApproval: (approvalId: string, decision: ApprovalDecision) => Promise<void>;
  retry: (prompt: string) => void;
  chooseAnotherAccount: () => void;
  manageUsage?: () => void;
  loadImage?: (path: string) => Promise<string>;
}) {
  const demo = session.provider === 'mock';
  const team =
    !session.parent_session_id && (session.chat_mode === 'team' || session.chat_mode === 'auto');
  // Finished turns keep their object identity, so memoized steps skip re-rendering while a new
  // answer streams in. A turn changes only when its own events change.
  const turnCache = useRef(new Map<string, { key: string; turn: TimelineTurn }>());
  const turns = useMemo(() => {
    const signatures = new Map<string, string>();
    const counts = new Map<string, number>();
    for (const event of events) {
      const count = (counts.get(event.run_id) ?? 0) + 1;
      counts.set(event.run_id, count);
      signatures.set(event.run_id, `${count}:${event.sequence}:${demo}`);
    }
    const cache = turnCache.current;
    const next = buildTimeline(events).map((built) => {
      const key = signatures.get(built.id) ?? '';
      const cached = cache.get(built.id);
      if (cached?.key === key) return cached.turn;
      const turn = demo
        ? {
            ...built,
            text: demoResponse(built.text),
            error: built.error && demoActivity(built.error),
            activities: built.activities.map((activity) => ({
              ...activity,
              label: demoActivity(activity.label),
              detail: demoActivity(activity.detail),
            })),
          }
        : built;
      cache.set(built.id, { key, turn });
      return turn;
    });
    for (const id of cache.keys()) if (!signatures.has(id)) cache.delete(id);
    return next;
  }, [events, demo]);
  const [filter, setFilter] = useState<TimelineFilter>('all');
  const [image, setImage] = useState<{ name: string; src?: string; error?: string }>();
  const imageRequest = useRef(0);
  const viewImage = async (file: { name: string; path: string }) => {
    const request = ++imageRequest.current;
    setImage({ name: file.name });
    try {
      if (!loadImage) throw new Error('Просмотр вложения сейчас недоступен.');
      const src = await loadImage(file.path);
      if (request === imageRequest.current) setImage({ name: file.name, src });
    } catch (error) {
      if (request === imageRequest.current) setImage({ name: file.name, error: errorText(error) });
    }
  };
  const [showDetails, setShowDetails] = useState(false);
  const [query, setQuery] = useState('');
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const visibleTurns = useMemo(() => filterTimeline(turns, filter, query), [turns, filter, query]);
  // Only the latest turns are mounted; scrolling to the top mounts earlier ones in pages.
  const [shown, setShown] = useState(renderPage);
  const growAnchor = useRef<number | null>(null);
  const renderedTurns = visibleTurns.slice(-shown);
  const hiddenTurns = visibleTurns.length - renderedTurns.length;
  const showEarlier = (count = renderPage) => {
    const element = scroll.current;
    if (element && growAnchor.current === null) growAnchor.current = element.scrollHeight;
    setShown((value) => value + count);
  };
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
  // Earlier turns were mounted above the reader: keep the same text in view.
  useLayoutEffect(() => {
    const element = scroll.current;
    if (!element || growAnchor.current === null) return;
    element.scrollTop += element.scrollHeight - growAnchor.current;
    growAnchor.current = null;
    lastHeight.current = element.scrollHeight;
  }, [shown]);
  useEffect(() => {
    follow.current = true;
    firstSequence.current = undefined;
    growAnchor.current = null;
    setShown(renderPage);
    setCollapsed(new Set());
    setActiveTurn('');
    imageRequest.current++;
    setImage(undefined);
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
    const index = visibleTurns.findIndex((turn) => turn.id === id);
    if (index < 0) {
      setFilter('all');
      setQuery('');
      const all = turns.findIndex((turn) => turn.id === id);
      if (all >= 0) setShown((value) => Math.max(value, turns.length - all));
    } else if (index < hiddenTurns) {
      setShown(visibleTurns.length - index);
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
          className="icon-button history-search-toggle"
          aria-label="Поиск и фильтры истории"
          title="Поиск по чату"
          aria-expanded={showDetails}
          onClick={() => {
            setShowDetails(!showDetails);
            setFilter('all');
          }}
        >
          {showDetails ? <X size={15} /> : <Search size={15} />}
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
          if (element) {
            follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
            if (element.scrollTop < 400 && hiddenTurns > 0 && growAnchor.current === null)
              showEarlier();
          }
          trackActive();
        }}
      >
        <ol className="stepper">
          {historyLoading && (
            <li className="stepper-more" role="status">
              <ThinkingIndicator label="Загружаем раннюю историю чата…" />
            </li>
          )}
          {hiddenTurns > 0 && (
            <li className="stepper-more">
              <button className="text-button" onClick={() => showEarlier()}>
                Показать более ранние сообщения · {hiddenTurns}
              </button>
            </li>
          )}
          {overview && !turns.length && (
            <li className="chat-empty">
              <h2>Над чем поработаем?</h2>
              {demo && (
                <p className="small muted">Демо-режим: подключите аккаунт для ответов ИИ.</p>
              )}
            </li>
          )}
          {turns.length > 0 && visibleTurns.length === 0 && (
            <li className="stepper-empty">Подходящих событий нет.</li>
          )}
          {renderedTurns.map((turn) => (
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
              waiting={waitingApproval}
              viewImage={(file) => void viewImage(file)}
            />
          ))}
          {overview && interrupted && (
            <li className="step step-tail">
              <p className="muted small">
                Предыдущий запуск прерван. История сохранена — отправьте сообщение, чтобы
                продолжить.
              </p>
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
      {image && (
        <ImagePreview
          {...image}
          close={() => {
            imageRequest.current++;
            setImage(undefined);
          }}
        />
      )}
    </>
  );
}

type TurnStepProps = Parameters<typeof TurnStepView>[0];
// Callbacks are recreated on every parent render but act on the same session and turn, so only
// data props decide whether a step re-renders. Finished turns keep identity (see turnCache).
const TurnStep = memo(
  TurnStepView,
  (a: TurnStepProps, b: TurnStepProps) =>
    a.turn === b.turn &&
    a.filter === b.filter &&
    a.demo === b.demo &&
    a.team === b.team &&
    a.providerName === b.providerName &&
    a.model === b.model &&
    a.interrupted === b.interrupted &&
    a.live === b.live &&
    a.open === b.open &&
    a.last === b.last &&
    a.waiting === b.waiting &&
    !a.manageUsage === !b.manageUsage,
);

function TurnStepView({
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
  waiting,
  viewImage,
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
  waiting: boolean;
  viewImage: (file: { name: string; path: string }) => void;
}) {
  const state: StepState = turn.status === 'running' && interrupted ? 'interrupted' : turn.status;
  const running = turn.status === 'running' && live;
  const showTools = filter !== 'agent';
  const showText = filter !== 'tools';
  const configuration = `${providerName} · ${model} · ${turn.reasoningEffort ? (effortLabels[turn.reasoningEffort] ?? turn.reasoningEffort) : 'по умолчанию'}${turn.fastMode ? ' · Fast' : ''}`;
  // Team turns keep their final answer as one block; single chats interleave text and tools.
  const parts = team
    ? turn.parts.filter((part) => part.kind !== 'text')
    : turn.parts.length
      ? turn.parts
      : turn.text
        ? [{ kind: 'text' as const, text: turn.text }]
        : [];
  const lastText = parts.reduce((index, part, i) => (part.kind === 'text' ? i : index), -1);
  const runningTool = running
    ? turn.parts
        .flatMap((part) => (part.kind === 'tools' ? part.items : []))
        .filter((item) => item.state === 'running')
        .at(-1)
    : undefined;
  const footer =
    turn.finishedAt && turn.status !== 'running'
      ? `${state === 'stopped' ? 'Остановлено вами' : state === 'failed' ? 'Ошибка' : 'Ответ завершён'} · ${duration(turn.finishedAt - turn.startedAt)}`
      : '';
  return (
    <li className={`turn step step-${state}`} data-turn-id={turn.id}>
      {filter === 'all' && (turn.prompt || turn.attachments.length > 0) && (
        <div className="step-message user-message">
          <div className="prose">{turn.prompt}</div>
          {turn.attachments.length > 0 && (
            <div className="sent-attachments">
              {turn.attachments.map((file) =>
                file.mime.startsWith('image/') ? (
                  <button
                    className="sent-image-button"
                    key={file.path}
                    onClick={() => viewImage(file)}
                    aria-label={`Посмотреть ${file.name}`}
                    title="Посмотреть изображение"
                  >
                    {file.name}
                  </button>
                ) : (
                  <span key={file.path} title={`${attachmentHint(file)}\n${file.path}`}>
                    {file.name}
                  </span>
                ),
              )}
            </div>
          )}
          <button
            className="icon-button turn-toggle"
            aria-expanded={open}
            aria-label={open ? 'Свернуть ответ' : 'Развернуть ответ'}
            title={open ? 'Свернуть ответ' : 'Развернуть ответ'}
            onClick={toggle}
          >
            <ChevronDown size={14} />
          </button>
        </div>
      )}
      {filter === 'all' &&
        turn.followups.map((message, index) => (
          <div className="step-message user-message followup-message" key={index}>
            <div className="entry-label">Уточнение</div>
            <div className="prose">{message.prompt}</div>
            {message.files.map((file) => (
              <span className="small" key={file.path}>
                {file.name}{' '}
              </span>
            ))}
          </div>
        ))}
      {open && (
        <div className="turn-body">
          {parts.map((part, index) => {
            if (part.kind === 'tools')
              return showTools ? (
                <ToolGroup key={index} items={part.items} expanded={filter === 'tools'} />
              ) : null;
            if (part.kind === 'todos')
              return showTools ? <TodoCard key={index} items={part.items} /> : null;
            return showText && part.text.trim() ? (
              <div className="step-message agent" key={index} title={configuration}>
                <div className="prose">
                  <ChatMarkdown
                    text={demo ? demoResponse(part.text) : part.text}
                    streaming={running && index === parts.length - 1}
                    copy={index === lastText && !running}
                  />
                </div>
              </div>
            ) : null;
          })}
          {showText && (turn.progress.trim() || turn.teamMessages.length > 0) && (
            <details className="progress-details">
              <summary>
                <Sparkles size={14} /> {team ? 'Обсуждение команды' : 'Ход работы'}
              </summary>
              {turn.progress.trim() && (
                <div className="progress-copy">
                  <ChatMarkdown text={turn.progress} streaming={running} />
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
                      {message.model}
                      {message.reasoningEffort
                        ? ` · ${effortLabels[message.reasoningEffort] ?? message.reasoningEffort}`
                        : ''}
                    </small>
                  )}
                  <ChatMarkdown text={message.text} streaming={running} />
                </div>
              ))}
            </details>
          )}
          {team && showText && turn.text && (
            <div className="step-message agent" title={configuration}>
              <div className="entry-label">Ответ команды</div>
              <div className="prose">
                <ChatMarkdown text={turn.text} streaming={running} />
              </div>
            </div>
          )}
        </div>
      )}
      {turn.approvals
        .filter((approval) => filter !== 'agent' || !approval.decision)
        .map((approval) => (
          <ApprovalCard
            key={approval.id}
            approval={approval}
            actionable={running}
            resolve={(decision) => resolveApproval(approval.id, decision)}
          />
        ))}
      {running && last && (
        <LiveStatus
          item={runningTool}
          label={
            waiting
              ? 'Ждёт вашего решения'
              : team
                ? turn.activities.some((activity) => activity.label === 'Сборка результата')
                  ? 'Собирает общий ответ…'
                  : 'Команда работает…'
                : undefined
          }
        />
      )}
      {turn.changes && turn.status !== 'running' && filter !== 'tools' && (
        <RunChanges summary={turn.changes} latest={last} />
      )}
      {turn.error && turn.errorKind === 'usage_limit' ? (
        <div role="alert" className="limit-notice">
          <strong>Лимит аккаунта исчерпан</strong>
          <p>{turn.error}</p>
          {last && (
            <div className="limit-actions">
              {manageUsage && (
                <button className="primary-button" onClick={manageUsage}>
                  Лимиты
                </button>
              )}
              <button className="secondary-button" onClick={() => retry(turn.prompt)}>
                Повторить
              </button>
              <button className="secondary-button" onClick={chooseAnotherAccount}>
                Другой аккаунт
              </button>
            </div>
          )}
        </div>
      ) : (
        turn.error && (
          <div role="alert" className="notice error">
            {demo ? demoActivity(turn.error) : turn.error}
          </div>
        )
      )}
      {open && (footer || turn.usage.length > 0 || turn.diagnostics.length > 0) && (
        <div className="turn-footer">
          {footer && <span className="step-completed-label">{footer}</span>}
          {filter !== 'tools' && <UsageDiagnostics turn={turn} />}
        </div>
      )}
    </li>
  );
}

const decisionLabels: Record<string, string> = {
  allow_once: 'Разрешено один раз',
  allow_session: 'Разрешено в этом чате',
  deny: 'Отклонено',
  expired: 'Закрыт без ответа',
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
      {approval.reason && <p className="approval-meta">{approval.reason}</p>}
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
              Разрешить в чате
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
        <p className="notice">Неподдерживаемый запрос. Остановите задачу и обновите CLI.</p>
      )}
      {pending && !actionable && <p className="muted small">Запрос больше не активен.</p>}
      {error && <p className="notice error">{error}</p>}
    </div>
  );
}
