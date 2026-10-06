import { forwardRef } from 'react';
import { ArrowUp, ArrowUpRight, ShieldCheck, Sparkles, Square } from 'lucide-react';

export const Composer = forwardRef<
  HTMLTextAreaElement,
  {
    draft: string;
    setDraft: (value: string) => void;
    running: boolean;
    busy: boolean;
    send: () => void;
    cancel: () => void;
    providerName: string;
    model: string;
    account: string;
    permissions: string;
    demo: boolean;
    /** Shown when requests use the user's ChatGPT plan (Sign in with ChatGPT guidelines). */
    manageUsage?: () => void;
  }
>(function Composer(
  {
    draft,
    setDraft,
    running,
    busy,
    send,
    cancel,
    providerName,
    model,
    account,
    permissions,
    demo,
    manageUsage,
  },
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
          <div className="composer-controls" aria-label="Параметры сессии">
            <span className="control-pill" title="Агент сессии">
              <Sparkles size={13} /> {providerName}
            </span>
            <span className="control-pill" title="Модель">
              {model}
            </span>
            <span className="control-pill account-pill" title="Аккаунт">
              {account}
            </span>
            <span className="permission-pill" title="Профиль разрешений">
              <ShieldCheck size={13} />
              {permissions}
            </span>
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
            : 'Провайдер и аккаунт сессии не меняются автоматически'}
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
