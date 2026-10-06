import { forwardRef, type ReactNode } from 'react';
import { ArrowUp, ArrowUpRight, Square } from 'lucide-react';

export const Composer = forwardRef<
  HTMLTextAreaElement,
  {
    draft: string;
    setDraft: (value: string) => void;
    running: boolean;
    busy: boolean;
    send: () => void;
    cancel: () => void;
    controls: ReactNode;
    demo: boolean;
    /** Shown when requests use the user's ChatGPT plan (Sign in with ChatGPT guidelines). */
    manageUsage?: () => void;
  }
>(function Composer(
  { draft, setDraft, running, busy, send, cancel, controls, demo, manageUsage },
  ref,
) {
  return (
    <div className="composer-region">
      <div className={`composer ${running ? 'composer-running' : ''}`}>
        <textarea
          ref={ref}
          aria-label="Сообщение агенту"
          placeholder={
            running
              ? 'Агент работает. Дождитесь ответа или остановите его.'
              : 'Опишите задачу, задайте вопрос или предложите идею…'
          }
          value={draft}
          maxLength={16000}
          disabled={running}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
              event.preventDefault();
              send();
            }
          }}
        />
        <div className="composer-toolbar">
          <div className="composer-controls" aria-label="Параметры чата">
            {controls}
          </div>
          {running ? (
            <button
              className="send-button stop-button"
              aria-label="Остановить агента"
              onClick={cancel}
            >
              <Square size={14} />
            </button>
          ) : (
            <button
              className="send-button"
              aria-label="Отправить сообщение"
              disabled={!draft.trim() || busy}
              onClick={send}
            >
              <ArrowUp size={17} />
            </button>
          )}
        </div>
      </div>
      <div className="composer-caption">
        <span>
          {demo
            ? 'Локальный симулятор · файлы проекта не читает и не меняет'
            : 'Контекст сжимается Codex автоматически · смена провайдера через «Перейти»'}
          {manageUsage && (
            <>
              {' · '}
              <span className="plan-usage-note">Используется план ChatGPT</span>{' '}
              <button className="inline-link" onClick={manageUsage}>
                Управлять использованием <ArrowUpRight size={11} />
              </button>
            </>
          )}
        </span>
        <span>
          <kbd>Ctrl ↵</kbd> отправить
        </span>
      </div>
    </div>
  );
});
