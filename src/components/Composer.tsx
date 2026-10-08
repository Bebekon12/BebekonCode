import { forwardRef, useRef, useState, type ReactNode } from 'react';
import { ArrowUp, ArrowUpRight, Square, Paperclip, FileText, X, LoaderCircle } from 'lucide-react';
import {
  attachmentHint,
  attachmentSize,
  readAttachments,
  type DraftAttachment,
} from '../attachments';

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
    attachments: DraftAttachment[];
    setAttachments: (value: DraftAttachment[]) => void;
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
    controls,
    demo,
    manageUsage,
    attachments,
    setAttachments,
  },
  ref,
) {
  const fileInput = useRef<HTMLInputElement>(null);
  const [error, setError] = useState('');
  const [reading, setReading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const readingRef = useRef(false);
  const attach = async (files: File[]) => {
    if (running || busy || readingRef.current || !files.length) return;
    readingRef.current = true;
    setError('');
    setReading(true);
    try {
      setAttachments([...attachments, ...(await readAttachments(files, attachments))]);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      readingRef.current = false;
      setReading(false);
    }
  };
  return (
    <div className="composer-region">
      <div
        className={`composer ${running ? 'composer-running' : ''} ${dragging ? 'composer-dragging' : ''}`}
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes('Files')) {
            event.preventDefault();
            event.dataTransfer.dropEffect = running || busy ? 'none' : 'copy';
            setDragging(!running && !busy);
          }
        }}
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDragging(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          void attach(Array.from(event.dataTransfer.files));
        }}
      >
        <input
          className="sr-only"
          ref={fileInput}
          type="file"
          multiple
          tabIndex={-1}
          aria-label="Выбрать вложения"
          disabled={running || busy || reading}
          onChange={(event) => {
            void attach(Array.from(event.target.files ?? []));
            event.target.value = '';
          }}
        />
        {dragging && <div className="attachment-drop-hint">Отпустите файлы, чтобы прикрепить</div>}
        {attachments.length > 0 && (
          <div className="attachment-list" aria-label="Вложения сообщения">
            {attachments.map((file) => (
              <div className="attachment-card" key={file.id}>
                {file.mime.startsWith('image/') ? (
                  <img src={`data:${file.mime};base64,${file.data}`} alt={file.name} />
                ) : (
                  <FileText size={22} />
                )}
                <span>
                  <strong title={file.name}>{file.name}</strong>
                  <small>
                    {attachmentSize(file.size)} · {attachmentHint(file)}
                  </small>
                </span>
                <button
                  type="button"
                  className="icon-button"
                  aria-label={`Убрать ${file.name}`}
                  disabled={running || busy || reading}
                  onClick={() => setAttachments(attachments.filter((item) => item.id !== file.id))}
                >
                  <X size={14} />
                </button>
              </div>
            ))}
          </div>
        )}
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
          onPaste={(event) => {
            const files = Array.from(event.clipboardData.files);
            if (files.length) {
              event.preventDefault();
              void attach(files);
            }
          }}
          onKeyDown={(event) => {
            if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
              event.preventDefault();
              if (!reading) send();
            }
          }}
        />
        <div className="composer-toolbar">
          <button
            className="icon-button attach-button"
            type="button"
            aria-label="Прикрепить файлы"
            title="Прикрепить файлы · можно перетащить или вставить изображение"
            disabled={running || busy || reading}
            onClick={() => fileInput.current?.click()}
          >
            {reading ? <LoaderCircle size={18} className="spin" /> : <Paperclip size={18} />}
          </button>
          <div className="composer-controls" aria-label="Параметры чата">
            {controls}
          </div>
          {running ? (
            <button
              className="send-button stop-button"
              aria-label="Остановить агента"
              onClick={cancel}
            >
              <Square size={18} />
            </button>
          ) : (
            <button
              className="send-button"
              aria-label="Отправить сообщение"
              title="Отправить сообщение (Ctrl+Enter)"
              disabled={(!draft.trim() && !attachments.length) || busy || reading}
              onClick={send}
            >
              <ArrowUp size={21} />
            </button>
          )}
        </div>
      </div>
      {error && (
        <p className="field-error attachment-error" role="alert">
          {error}
        </p>
      )}
      <details className="attachment-help">
        <summary>Какие файлы можно прикрепить?</summary>
        <p>
          Фото: PNG, JPEG, GIF, WebP до 4 МиБ каждое и до 6 МиБ суммарно. DOCX, XLSX и PPTX:
          извлекаем текст без точного оформления. TXT, код и другие файлы передаются внутри проекта.
          PDF, видео, аудио и старые Office-файлы можно прикрепить, но их анализ зависит от
          доступных агенту инструментов. До 8 файлов, 20 МиБ на файл и 40 МиБ суммарно.
        </p>
      </details>
      <div className="composer-caption">
        <span>
          {demo
            ? 'Локальный симулятор · для ответов ИИ подключите аккаунт'
            : 'Контекст сжимается автоматически'}
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
