import { useState } from 'react';

export function publicImageUrl(value?: string): string | undefined {
  try {
    const url = new URL(value || '');
    if (
      url.protocol !== 'https:' ||
      url.username ||
      url.password ||
      (url.port && url.port !== '443') ||
      value!.length > 4096
    )
      return;
    const host = url.hostname.toLowerCase();
    if (
      !host.includes('.') ||
      /^[\d.]+$/.test(host) ||
      host.includes(':') ||
      /\.(localhost|local|internal|test|invalid)$/.test(host)
    )
      return;
    return url.href;
  } catch {
    return;
  }
}
export async function openAnswerLink(url: string) {
  if (!publicImageUrl(url)) throw new Error('Доступны только публичные HTTPS-ссылки');
  if ('__TAURI_INTERNALS__' in window) {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_answer_link', { url });
  } else window.open(url, '_blank', 'noopener,noreferrer');
}
export function AnswerLink({ href, children }: { href?: string; children: React.ReactNode }) {
  const [error, setError] = useState('');
  const safe = publicImageUrl(href);
  return (
    <span className="answer-link">
      {safe ? (
        <a
          href={safe}
          title={safe}
          rel="noreferrer noopener"
          target="_blank"
          onClick={(e) => {
            e.preventDefault();
            void openAnswerLink(safe).catch((error) => setError(String(error)));
          }}
        >
          {children}
        </a>
      ) : (
        <span>{children}</span>
      )}
      {error && <small role="alert">{error}</small>}
    </span>
  );
}
export function AnswerImage({ src, alt }: { src?: string; alt?: string }) {
  const [failed, setFailed] = useState(false),
    [expanded, setExpanded] = useState(false);
  const safe = publicImageUrl(src);
  if (!safe)
    return <span className="muted">Изображение недоступно: {alt || 'неподдерживаемый адрес'}</span>;
  return (
    <span className="answer-image">
      {!failed ? (
        <button
          className="answer-image-open"
          aria-label={`Увеличить: ${alt || 'изображение'}`}
          onClick={() => setExpanded(true)}
        >
          <img
            src={safe}
            alt={alt || 'Иллюстрация из ответа'}
            loading="lazy"
            decoding="async"
            referrerPolicy="no-referrer"
            crossOrigin="anonymous"
            onError={() => setFailed(true)}
          />
        </button>
      ) : (
        <span className="answer-image-unavailable">
          Изображение не загрузилось. Можно открыть источник.
        </span>
      )}
      <span className="answer-image-caption">
        {alt && <span>{alt}</span>}
        <AnswerLink href={safe}>{new URL(safe).hostname} ↗</AnswerLink>
      </span>
      {expanded && (
        <span
          className="answer-image-modal"
          role="dialog"
          aria-label={alt || 'Изображение'}
          aria-modal="true"
          onKeyDown={(e) => {
            if (e.key === 'Escape') setExpanded(false);
          }}
        >
          <button autoFocus onClick={() => setExpanded(false)} aria-label="Закрыть изображение">
            Закрыть
          </button>
          <img
            src={safe}
            alt={alt || 'Иллюстрация'}
            referrerPolicy="no-referrer"
            crossOrigin="anonymous"
          />
        </span>
      )}
    </span>
  );
}
