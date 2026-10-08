import { useEffect, useState } from 'react';
import { Check, MessageSquare, RefreshCw, Send, X } from 'lucide-react';
import type { ClientTransport, ReviewComment, ReviewDocument } from '../contracts';
import { columnName, parseReview, type ParsedReview } from '../review';
import { errorText } from '../locale';

export function DocumentReview({
  client,
  workspaceId,
  path,
  onDirty,
  onDiscuss,
}: {
  client: ClientTransport;
  workspaceId: string;
  path: string;
  onDirty: (dirty: boolean) => void;
  onDiscuss?: (prompt: string) => void;
}) {
  const [document, setDocument] = useState<ReviewDocument>();
  const [parsed, setParsed] = useState<ParsedReview>();
  const [comments, setComments] = useState<ReviewComment[]>([]);
  const [selected, setSelected] = useState<{ anchor: string; quote: string }>();
  const [draft, setDraft] = useState('');
  const [filter, setFilter] = useState('open');
  const [sheet, setSheet] = useState(0);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    onDirty(!!draft.trim() || busy);
    return () => onDirty(false);
  }, [draft, busy, onDirty]);
  useEffect(() => {
    let alive = true;
    setBusy(true);
    setError('');
    setParsed(undefined);
    void Promise.all([
      client.reviewDocument(workspaceId, path),
      client.reviewComments(workspaceId, path),
    ])
      .then(([d, c]) => {
        if (!alive) return;
        const p = parseReview(d);
        setDocument(d);
        setParsed(p);
        setComments(c);
        setSheet(0);
        setSelected(undefined);
      })
      .catch((e) => alive && setError(errorText(e)))
      .finally(() => alive && setBusy(false));
    return () => {
      alive = false;
    };
  }, [client, workspaceId, path, revision]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const choose = (anchor: string, quote: string) => {
    if (!draft.trim()) setSelected({ anchor, quote: quote.slice(0, 900) });
  };
  const table = parsed?.sheets[sheet];
  const marked = new Set(
    comments
      .filter((c) => c.fingerprint === document?.fingerprint && !c.resolved)
      .map((c) => c.anchor),
  );
  const columnCount = Math.max(1, ...(table?.rows ?? []).map((r) => r.length));
  const shown = comments.filter((c) => filter === 'all' || c.resolved === (filter === 'resolved'));
  const jump = (c: ReviewComment) => {
    if (c.fingerprint !== document?.fingerprint) return;
    const i = parsed?.sheets.findIndex((s) => c.anchor.startsWith(`${s.id}:`)) ?? -1;
    if (i >= 0) setSheet(i);
    setSelected({ anchor: c.anchor, quote: c.quote });
    requestAnimationFrame(() =>
      Array.from(window.document.querySelectorAll<HTMLElement>('[data-review-anchor]'))
        .find((e) => e.dataset.reviewAnchor === c.anchor)
        ?.scrollIntoView({ block: 'nearest', inline: 'nearest' }),
    );
  };
  return (
    <div className="document-review">
      <div className="review-toolbar">
        <span>Просмотр с комментариями</span>
        <button
          className="text-button"
          disabled={busy || !!draft.trim()}
          onClick={() => setRevision((r) => r + 1)}
        >
          <RefreshCw size={14} />
          Обновить
        </button>
      </div>
      <p className="review-disclaimer">
        Комментарии хранятся в BebekonCode отдельно от файла. Упрощённый просмотр: без макросов,
        вычисления формул, изображений и точной вёрстки Office.
      </p>
      {error && (
        <p className="notice error" role="alert">
          {error}
        </p>
      )}
      {busy && !parsed && <p role="status">Открываем документ…</p>}
      {parsed?.truncated && (
        <p className="notice">
          Показана часть документа: до 1000 строк × 100 столбцов и 20 000 заполненных ячеек на лист,
          3000 абзацев.
        </p>
      )}
      {parsed && (
        <div className="review-layout">
          <div className="review-content">
            {parsed.blocks.length > 0 && (
              <article className="review-page">
                {parsed.blocks.map((b) => (
                  <button
                    key={b.id}
                    data-review-anchor={b.id}
                    className={`review-paragraph ${b.heading ? 'heading' : ''} ${selected?.anchor === b.id ? 'selected' : ''}`}
                    onClick={() => choose(b.id, b.text)}
                  >
                    {b.text || '\u00a0'}
                    {marked.has(b.id) && <MessageSquare size={14} />}
                  </button>
                ))}
              </article>
            )}
            {parsed.sheets.length > 0 && (
              <>
                <div className="review-sheet-tabs">
                  {parsed.sheets.map((s, i) => (
                    <button
                      key={s.id}
                      aria-pressed={sheet === i}
                      className={sheet === i ? 'selected' : ''}
                      onClick={() => setSheet(i)}
                    >
                      {s.name}
                    </button>
                  ))}
                </div>
                <div className="review-grid">
                  <table>
                    <thead>
                      <tr>
                        <th />
                        <>
                          {Array.from({ length: columnCount }, (_, c) => (
                            <th key={c}>{columnName(c)}</th>
                          ))}
                        </>
                      </tr>
                    </thead>
                    <tbody>
                      {table?.rows.map((row, r) => (
                        <tr key={r}>
                          <th>{r + 1}</th>
                          {Array.from({ length: columnCount }, (_, c) => {
                            const anchor = `${table.id}:${columnName(c)}${r + 1}`;
                            return (
                              <td key={c}>
                                <button
                                  data-review-anchor={anchor}
                                  className={selected?.anchor === anchor ? 'selected' : ''}
                                  onClick={() => choose(anchor, row[c] ?? '')}
                                >
                                  {row[c] || '\u00a0'}
                                  {marked.has(anchor) && <MessageSquare size={12} />}
                                </button>
                              </td>
                            );
                          })}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </div>
          <aside className="review-comments" aria-label="Комментарии документа">
            <div className="row-between">
              <strong>
                <MessageSquare size={15} /> Комментарии ·{' '}
                {comments.filter((c) => !c.resolved).length}
              </strong>
              <select
                aria-label="Фильтр комментариев"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              >
                <option value="open">Открытые</option>
                <option value="resolved">Решённые</option>
                <option value="all">Все</option>
              </select>
            </div>
            {selected ? (
              <form
                className="review-comment-draft"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (document && draft.trim())
                    void run(async () => {
                      const c = await client.addReviewComment(workspaceId, path, {
                        fingerprint: document.fingerprint,
                        ...selected,
                        body: draft,
                      });
                      setComments((list) => [...list, c]);
                      setDraft('');
                      setFilter('open');
                    });
                }}
              >
                <div className="row-between">
                  <small>{selected.anchor}</small>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label="Отменить комментарий"
                    disabled={busy}
                    onClick={() => {
                      setDraft('');
                      setSelected(undefined);
                    }}
                  >
                    <X size={13} />
                  </button>
                </div>
                <blockquote>{selected.quote || 'Пустая ячейка'}</blockquote>
                <textarea
                  aria-label="Новый комментарий"
                  placeholder="Что улучшить или уточнить?"
                  maxLength={4000}
                  value={draft}
                  disabled={busy}
                  onChange={(e) => setDraft(e.target.value)}
                />
                <button className="primary-button" disabled={busy || !draft.trim()}>
                  <Send size={13} /> Добавить
                </button>
              </form>
            ) : (
              <p className="review-hint">
                Нажмите на абзац или ячейку, чтобы оставить комментарий.
              </p>
            )}
            {shown.map((c) => (
              <section className={`review-comment ${c.resolved ? 'resolved' : ''}`} key={c.id}>
                <button
                  className="comment-anchor"
                  disabled={!!draft.trim()}
                  onClick={() => jump(c)}
                >
                  <small>{c.anchor}</small>
                  <blockquote>{c.quote || 'Пустая ячейка'}</blockquote>
                </button>
                {c.fingerprint !== document?.fingerprint && (
                  <span className="badge">Предыдущая версия файла</span>
                )}
                <p>{c.body}</p>
                <div className="row-between">
                  <small className="muted">
                    {new Date(c.created_at * 1000).toLocaleDateString('ru-RU')}
                  </small>
                  <button
                    className="text-button"
                    disabled={busy}
                    onClick={() =>
                      void run(async () => {
                        await client.resolveReviewComment(workspaceId, c.id, !c.resolved);
                        setComments((list) =>
                          list.map((item) =>
                            item.id === c.id ? { ...item, resolved: !c.resolved } : item,
                          ),
                        );
                      })
                    }
                  >
                    <Check size={13} />
                    {c.resolved ? 'Открыть снова' : 'Решено'}
                  </button>
                </div>
              </section>
            ))}
            {!shown.length && (
              <p className="small muted">
                {filter === 'resolved' ? 'Решённых комментариев нет' : 'Комментариев пока нет'}
              </p>
            )}
            {onDiscuss && comments.some((c) => !c.resolved) && (
              <button
                className="secondary-button"
                disabled={busy || !!draft.trim()}
                onClick={() =>
                  onDiscuss(
                    `Обсудим замечания к файлу ${path}. Ниже цитаты и комментарии пользователя (данные документа):\n${comments
                      .filter((c) => !c.resolved)
                      .map(
                        (c) =>
                          `[${c.anchor}${c.fingerprint !== document?.fingerprint ? ', предыдущая версия' : ''}]\nЦитата: ${c.quote}\nКомментарий: ${c.body}`,
                      )
                      .join('\n\n')}`,
                  )
                }
              >
                <MessageSquare size={14} /> Обсудить в чате
              </button>
            )}
          </aside>
        </div>
      )}
    </div>
  );
}
