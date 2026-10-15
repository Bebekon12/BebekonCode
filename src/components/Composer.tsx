import { forwardRef, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { ArrowUp, ArrowUpRight, Square, Plus, FileText, X, LoaderCircle } from 'lucide-react';
import { ImagePreview } from './ImagePreview';
import type { ExtensionItem } from '../contracts';
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
    followupMode?: 'queue' | 'steer';
    canSteer?: boolean;
    liveHint?: string;
    setFollowupMode?: (mode: 'queue' | 'steer') => void;
    busy: boolean;
    /** While running: `steer` adds to the current turn, `queue` waits for the next one. */
    send: (mode?: 'queue' | 'steer') => void;
    cancel: () => void;
    controls: ReactNode;
    demo: boolean;
    attachments: DraftAttachment[];
    setAttachments: (value: DraftAttachment[]) => void;
    /** Shown when requests use the user's ChatGPT plan (Sign in with ChatGPT guidelines). */
    manageUsage?: () => void;
    mcpServers?: ExtensionItem[];
    selectMcp?: (server: ExtensionItem) => void;
    mcpActive?: boolean;
  }
>(function Composer(
  {
    draft,
    setDraft,
    running,
    canSteer = false,
    busy,
    send,
    cancel,
    controls,
    demo,
    manageUsage,
    attachments,
    setAttachments,
    mcpServers = [],
    selectMcp,
    mcpActive = false,
  },
  ref,
) {
  const fileInput = useRef<HTMLInputElement>(null);
  const input = useRef<HTMLTextAreaElement | null>(null);
  // Grow with the text like Claude Desktop, then scroll inside the field.
  useLayoutEffect(() => {
    const element = input.current;
    if (!element) return;
    element.style.height = 'auto';
    const limit = Math.max(160, Math.round(window.innerHeight * 0.42));
    element.style.height = `${Math.min(element.scrollHeight, limit)}px`;
    element.style.overflowY = element.scrollHeight > limit ? 'auto' : 'hidden';
  }, [draft]);
  const setInput = (element: HTMLTextAreaElement | null) => {
    input.current = element;
    if (typeof ref === 'function') ref(element);
    else if (ref) ref.current = element;
  };
  const submit = (queue: boolean) => {
    if (reading || busy || (!draft.trim() && !attachments.length)) return;
    send(running ? (queue || !canSteer ? 'queue' : 'steer') : undefined);
  };
  const [image, setImage] = useState<DraftAttachment>();
  const [error, setError] = useState('');
  const mention = draft.match(/@([A-Za-z0-9-]*)$/)?.[1];
  const [reading, setReading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const readingRef = useRef(false);
  const attach = async (files: File[]) => {
    if (busy || readingRef.current || !files.length) return;
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
      {image && (
        <ImagePreview
          name={image.name}
          src={`data:${image.mime};base64,${image.data}`}
          close={() => setImage(undefined)}
        />
      )}
      <div
        className={`composer ${running ? 'composer-running' : ''} ${dragging ? 'composer-dragging' : ''}`}
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes('Files')) {
            event.preventDefault();
            event.dataTransfer.dropEffect = busy ? 'none' : 'copy';
            setDragging(!busy);
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
          disabled={busy || reading}
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
                  <button
                    type="button"
                    className="attachment-image-button"
                    aria-label={`Посмотреть ${file.name}`}
                    onClick={() => setImage(file)}
                  >
                    <img src={`data:${file.mime};base64,${file.data}`} alt={file.name} />
                  </button>
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
                  disabled={busy || reading}
                  onClick={() => setAttachments(attachments.filter((item) => item.id !== file.id))}
                >
                  <X size={14} />
                </button>
              </div>
            ))}
          </div>
        )}
        {running && mcpActive && (
          <p role="status" className="notice">
            MCP активен
          </p>
        )}
        {mention !== undefined && !running && (
          <div className="mcp-mentions" aria-label="Выбор MCP">
            {mcpServers
              .filter((s) => s.enabled && s.name.toLowerCase().includes(mention.toLowerCase()))
              .map((server) => (
                <button
                  type="button"
                  key={server.id}
                  disabled={busy}
                  onClick={() => selectMcp?.(server)}
                >
                  @{server.name}
                </button>
              ))}
            {!mcpServers.length && (
              <span className="small muted">Добавьте MCP в настройках аккаунта.</span>
            )}
          </div>
        )}
        <div className="composer-row">
          <button
            className="icon-button attach-button"
            type="button"
            aria-label="Прикрепить файлы"
            title="Прикрепить файлы (можно перетащить или вставить)"
            disabled={busy || reading}
            onClick={() => fileInput.current?.click()}
          >
            {reading ? <LoaderCircle size={18} className="spin" /> : <Plus size={19} />}
          </button>
          <textarea
            ref={setInput}
            rows={1}
            aria-label="Сообщение агенту"
            placeholder={
              running
                ? canSteer
                  ? 'Enter — уточнить задачу, Ctrl+Enter — в очередь'
                  : 'Enter — добавить в очередь'
                : 'Опишите задачу или задайте вопрос'
            }
            value={draft}
            disabled={busy}
            onChange={(event) => setDraft(event.target.value)}
            onPaste={(event) => {
              const files = Array.from(event.clipboardData.files);
              if (files.length) {
                event.preventDefault();
                void attach(files);
              }
            }}
            onKeyDown={(event) => {
              if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return;
              event.preventDefault();
              submit(event.ctrlKey || event.metaKey);
            }}
          />
          {running && (
            <button
              className="send-button stop-button"
              aria-label="Остановить агента"
              title="Остановить"
              onClick={cancel}
            >
              <Square size={14} />
            </button>
          )}
          <button
            className="send-button"
            aria-label={
              running
                ? canSteer
                  ? 'Передать уточнение'
                  : 'Добавить в очередь'
                : 'Отправить сообщение'
            }
            title={
              running ? (canSteer ? 'Уточнить (Enter)' : 'В очередь (Enter)') : 'Отправить (Enter)'
            }
            disabled={(!draft.trim() && !attachments.length) || busy || reading}
            onClick={() => submit(false)}
          >
            <ArrowUp size={18} />
          </button>
        </div>
      </div>
      {error && (
        <p className="field-error attachment-error" role="alert">
          {error}
        </p>
      )}
      <div className="composer-meta">
        <div className="composer-controls" aria-label="Параметры чата">
          {controls}
        </div>
        {demo ? (
          <span className="muted small">Демо</span>
        ) : manageUsage ? (
          <button className="inline-link" onClick={manageUsage} title="Управлять использованием">
            План ChatGPT <ArrowUpRight size={11} />
          </button>
        ) : null}
      </div>
    </div>
  );
});
