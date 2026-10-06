import { useEffect, useMemo, useRef } from 'react';
import { Check, ChevronRight, CircleStop, Sparkles, UserRound } from 'lucide-react';
import { buildTimeline } from '../timeline';
import type { AgentEvent, Session } from '../contracts';
export function Timeline({
  events,
  session,
  loadOlder,
}: {
  events: AgentEvent[];
  session: Session;
  loadOlder: () => void;
}) {
  const turns = useMemo(() => buildTimeline(events), [events]);
  const scroll = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  useEffect(() => {
    if (follow.current) scroll.current?.scrollTo({ top: scroll.current.scrollHeight });
  }, [events]);
  useEffect(() => {
    follow.current = true;
  }, [session.id]);
  return (
    <div
      className="timeline"
      ref={scroll}
      onScroll={() => {
        const element = scroll.current;
        if (element)
          follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
      }}
    >
      <div className="timeline-inner">
        {events.length >= 300 && (
          <button className="text-button" onClick={loadOlder}>
            Load previous activity
          </button>
        )}
        {turns.length === 0 && (
          <div className="session-empty">
            <div className="agent-glyph">
              <Sparkles size={24} />
            </div>
            <h2>What are we working on?</h2>
            <p>
              Send a task to try the local streaming simulator.
              <br />
              No files will be read or changed.
            </p>
          </div>
        )}
        {session.status === 'interrupted' && (
          <div className="notice">
            The previous run was interrupted when the core stopped. Your saved history is available.
            Send a new message to continue.
          </div>
        )}
        {turns.map((turn) => (
          <section className="turn" key={turn.id}>
            {turn.prompt && (
              <div className="timeline-entry">
                <div className="avatar">
                  <UserRound size={15} />
                </div>
                <div className="entry-body">
                  <div className="entry-label">You</div>
                  <div className="prose">{turn.prompt}</div>
                </div>
              </div>
            )}
            <div className="timeline-entry">
              <div className="avatar avatar-agent">
                <Sparkles size={15} />
              </div>
              <div className="entry-body">
                <div className="entry-label">
                  Local demo <span className="muted">· mock-stream-v1</span>
                </div>
                {turn.activities.map((activity, index) => (
                  <details className="tool-activity" key={`${turn.id}-${index}`}>
                    <summary>
                      <ChevronRight size={13} />
                      <Check size={13} />
                      {activity.label}
                      <span className="demo-label">DEMO</span>
                    </summary>
                    <p>{activity.detail}</p>
                  </details>
                ))}
                <div className="prose">
                  {turn.text}
                  {turn.status === 'running' && session.status === 'running' && (
                    <span className="stream-caret" />
                  )}
                </div>
                {turn.error && (
                  <div role="alert" className="notice error">
                    {turn.error}
                  </div>
                )}
                {(turn.status === 'stopped' ||
                  (session.status === 'interrupted' && turn.status === 'running')) && (
                  <div className="turn-status">
                    <CircleStop size={13} />{' '}
                    {session.status === 'interrupted' ? 'Interrupted' : 'Stopped by you'}
                  </div>
                )}
                {turn.status === 'completed' && (
                  <div className="turn-status">
                    <Check size={13} /> Turn completed
                  </div>
                )}
              </div>
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
