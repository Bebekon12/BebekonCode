import { isValidElement, memo, useState, type ReactNode } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Check, Copy } from 'lucide-react';
import { richBlock, richBlockKinds } from './RichBlocks';

function CopyButton({ text, label }: { text: string; label: string }) {
  const [done, setDone] = useState(false);
  const [error, setError] = useState(false);
  return (
    <button
      className="text-button copy-button"
      aria-label={label}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text);
          setDone(true);
          setError(false);
        } catch {
          setError(true);
        }
      }}
    >
      {done ? <Check size={13} /> : <Copy size={13} />}{' '}
      {error ? 'Не удалось скопировать' : done ? 'Скопировано' : 'Копировать'}
    </button>
  );
}
function plain(children: ReactNode): string {
  if (typeof children === 'string') return children;
  if (Array.isArray(children)) return children.map(plain).join('');
  if (children && typeof children === 'object' && 'props' in children)
    return plain((children.props as { children: ReactNode }).children);
  return '';
}
function language(children: ReactNode): string {
  const child = Array.isArray(children) ? children[0] : children;
  if (!isValidElement(child)) return '';
  const className = (child.props as { className?: unknown }).className;
  return typeof className === 'string' ? className : '';
}
// Markdown parsing is the costliest part of a long chat; finished answers render once.
export const ChatMarkdown = memo(function ChatMarkdown({
  text,
  streaming,
}: {
  text: string;
  streaming: boolean;
}) {
  return (
    <div className="chat-markdown">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          pre: ({ children }) => {
            const kind = /language-bebekon-(\w+)/.exec(language(children))?.[1];
            if (kind && (richBlockKinds as readonly string[]).includes(kind)) {
              const block = richBlock(kind, plain(children));
              if (block) return block;
              if (streaming) return <p className="muted small">Готовим визуализацию…</p>;
            }
            return (
              <div className="code-review-block">
                <div className="code-block-toolbar">
                  <span>Код</span>
                  <CopyButton text={plain(children)} label="Копировать код" />
                </div>
                <pre>{children}</pre>
              </div>
            );
          },
          table: ({ children }) => (
            <div className="markdown-table">
              <table>{children}</table>
            </div>
          ),
          img: ({ alt }) => <span className="muted">[Изображение: {alt || 'без подписи'}]</span>,
          a: ({ href, children }) => (
            <span className="message-link" title={href}>
              {children} {href && <CopyButton text={href} label="Копировать ссылку" />}
            </span>
          ),
        }}
      >
        {text}
      </Markdown>
      {streaming ? (
        <span className="stream-caret" />
      ) : (
        <CopyButton text={text} label="Копировать ответ" />
      )}
    </div>
  );
});
