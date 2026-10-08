import { useEffect, useRef, useState } from 'react';
import {
  ArrowUp,
  File,
  Folder,
  FolderOpen,
  FolderPlus,
  Plus,
  RefreshCw,
  Save,
  Terminal,
} from 'lucide-react';
import type { ClientTransport, FileEntry, Workspace } from '../contracts';
import { DocumentReview } from './DocumentReview';
import { Dialog } from './Dialog';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { displayPath, errorText } from '../locale';

export function FileManager({
  client,
  workspace,
  close,
  onDiscuss,
}: {
  client: ClientTransport;
  workspace: Workspace;
  close: () => void;
  onDiscuss?: (prompt: string) => void;
}) {
  const [review, setReview] = useState(false);
  const [reviewDirty, setReviewDirty] = useState(false);
  const [reviewKey, setReviewKey] = useState(0);
  const [directory, setDirectory] = useState('');
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [original, setOriginal] = useState('');
  const [content, setContent] = useState('');
  const [editable, setEditable] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [revision, setRevision] = useState(0);
  const [operation, setOperation] = useState<'file' | 'folder' | 'rename' | 'delete' | null>(null);
  const [name, setName] = useState('');
  const [decision, setDecision] = useState<(() => void) | null>(null);
  const loading = useRef(false);
  const dirty = reviewDirty || (editable && content !== original.replace(/\r\n/g, '\n'));
  const locked = busy || !!decision;

  useEffect(() => {
    if (!isTauri() || (!dirty && !busy)) return;
    let alive = true;
    let unlisten: (() => void) | undefined;
    const window = getCurrentWindow();
    void window
      .onCloseRequested((event) => {
        event.preventDefault();
        if (busy) setError('Дождитесь завершения операции с файлом.');
        else
          setDecision(() => () => {
            void window.destroy();
          });
      })
      .then((remove) => {
        if (alive) unlisten = remove;
        else remove();
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [dirty, busy]);

  useEffect(() => {
    let alive = true;
    loading.current = true;
    setBusy(true);
    setEntries([]);
    setError('');
    void client
      .listFiles(workspace.id, directory)
      .then((entries) => {
        if (alive) setEntries(entries);
      })
      .catch((error) => {
        if (alive) setError(errorText(error));
      })
      .finally(() => {
        if (alive) {
          loading.current = false;
          setBusy(false);
        }
      });
    return () => {
      alive = false;
    };
  }, [client, workspace.id, directory, revision]);

  function guard(action: () => void) {
    if (busy || loading.current) return;
    if (dirty) setDecision(() => action);
    else action();
  }
  function clearEditor() {
    setReview(false);
    setReviewDirty(false);
    setSelected(null);
    setEditable(false);
    setOriginal('');
    setContent('');
    setOperation(null);
  }
  async function perform(action: () => Promise<void>) {
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await action();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  function open(entry: FileEntry, source = false) {
    guard(() => {
      clearEditor();
      setNotice('');
      if (entry.directory) {
        setDirectory(entry.path);
        return;
      }
      setSelected(entry);
      if (!source && /\.(docx|xlsx|pptx|csv|tsv)$/i.test(entry.path)) {
        setReview(true);
        setReviewKey((k) => k + 1);
        return;
      }
      void perform(async () => {
        const text = await client.readFile(workspace.id, entry.path);
        setOriginal(text);
        setContent(text.replace(/\r\n/g, '\n'));
        setEditable(true);
      });
    });
  }
  async function save() {
    if (!selected || !editable || !dirty || locked) return;
    await perform(async () => {
      const saved = original.includes('\r\n') ? content.replace(/\r?\n/g, '\r\n') : content;
      await client.saveFile(workspace.id, selected.path, original, saved);
      setOriginal(saved);
      setNotice('Сохранено на диске.');
    });
  }
  async function mutate() {
    if (!operation || locked) return;
    await perform(async () => {
      if (operation === 'delete' && selected) await client.deletePath(workspace.id, selected.path);
      else if (operation === 'rename' && selected)
        await client.renamePath(workspace.id, selected.path, name);
      else if (operation === 'file' || operation === 'folder')
        await client.createPath(
          workspace.id,
          directory ? `${directory}/${name}` : name,
          operation === 'folder',
        );
      clearEditor();
      setRevision((value) => value + 1);
    });
  }
  return (
    <Dialog title={`${workspace.name} · Файлы проекта`} close={() => guard(close)} wide>
      <div className="file-manager">
        <code className="file-root" title={displayPath(workspace.root)}>
          {displayPath(workspace.root)}
        </code>
        {decision && (
          <div className="notice file-decision" role="alert">
            <span>
              Не сохранять {reviewDirty ? 'черновик комментария к файлу' : 'изменения в файле'} «
              {selected?.name}»?
            </span>
            <button autoFocus className="button" onClick={() => setDecision(null)}>
              Продолжить редактирование
            </button>
            <button
              className="button danger"
              onClick={() => {
                const action = decision;
                setDecision(null);
                action();
              }}
            >
              Не сохранять
            </button>
          </div>
        )}
        <div className="file-toolbar">
          <button
            className="button"
            disabled={locked}
            onClick={() =>
              guard(() => {
                clearEditor();
                setOperation('file');
                setName('');
              })
            }
          >
            <Plus size={15} /> Новый файл
          </button>
          <button
            className="button"
            disabled={locked}
            onClick={() =>
              guard(() => {
                clearEditor();
                setOperation('folder');
                setName('');
              })
            }
          >
            <FolderPlus size={15} /> Новая папка
          </button>
          <button
            className="button"
            disabled={locked}
            onClick={() => void perform(() => client.openProject(workspace.id, false))}
          >
            <FolderOpen size={15} /> Проводник
          </button>
          <button
            className="button"
            disabled={locked}
            onClick={() => void perform(() => client.openProject(workspace.id, true))}
          >
            <Terminal size={15} /> PowerShell
          </button>
        </div>
        {error && (
          <p className="notice" role="alert">
            {error}
          </p>
        )}
        {notice && (
          <p className="muted small" role="status">
            {notice}
          </p>
        )}
        {operation && (
          <form
            className="file-operation"
            onSubmit={(event) => {
              event.preventDefault();
              void mutate();
            }}
          >
            {operation === 'delete' ? (
              <p>
                Удалить <strong>{selected?.path}</strong> безвозвратно? Восстановить удаление
                нельзя. Удалять можно файлы и пустые папки.
              </p>
            ) : (
              <label>
                {operation === 'rename'
                  ? 'Новый путь относительно проекта'
                  : `Имя ${operation === 'folder' ? 'новой папки' : 'нового файла'} в папке «${directory || 'корень проекта'}»`}
                <input
                  aria-label="Имя файла"
                  autoFocus
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  disabled={locked}
                  required
                />
              </label>
            )}
            <button
              className={`button ${operation === 'delete' ? 'danger' : 'primary'}`}
              disabled={locked || (operation !== 'delete' && !name.trim())}
            >
              {operation === 'delete' ? 'Удалить безвозвратно' : 'Подтвердить'}
            </button>
            <button
              type="button"
              className="button"
              disabled={locked}
              onClick={() => setOperation(null)}
            >
              Отмена
            </button>
          </form>
        )}
        <div className="file-layout">
          <section className="file-browser" aria-label="Файловый менеджер проекта">
            <div className="file-navigation">
              <button
                className="icon-button"
                aria-label="На уровень выше"
                disabled={locked || !directory}
                onClick={() =>
                  guard(() => {
                    clearEditor();
                    setDirectory(directory.split('/').slice(0, -1).join('/'));
                  })
                }
              >
                <ArrowUp size={15} />
              </button>
              <code title={directory}>{directory || '/'}</code>
              <button
                className="icon-button"
                aria-label="Обновить список файлов"
                disabled={locked}
                onClick={() => setRevision((value) => value + 1)}
              >
                <RefreshCw size={15} />
              </button>
            </div>
            <div className="file-list">
              {entries.map((entry) => (
                <div
                  className={`file-row ${selected?.path === entry.path ? 'selected' : ''}`}
                  key={entry.path}
                >
                  <button
                    title={
                      entry.blocked
                        ? 'Служебные файлы Git, ссылка или неподдерживаемое имя'
                        : entry.path
                    }
                    disabled={locked || entry.blocked}
                    onClick={() => open(entry)}
                  >
                    {entry.directory ? <Folder size={15} /> : <File size={15} />}
                    <span>{entry.name}</span>
                  </button>
                  {!entry.blocked && (
                    <button
                      className="icon-button"
                      aria-label={`Manage ${entry.name}`}
                      disabled={locked}
                      onClick={() =>
                        guard(() => {
                          clearEditor();
                          setSelected(entry);
                          setOperation('rename');
                          setName(entry.path);
                        })
                      }
                    >
                      …
                    </button>
                  )}
                </div>
              ))}
              {!entries.length && !busy && (
                <p className="muted small">В папке нет доступных элементов.</p>
              )}
              {busy && (
                <p className="muted small" role="status">
                  Работа с локальными файлами…
                </p>
              )}
            </div>
          </section>
          <section className="file-editor" aria-label="Редактор файлов">
            <div className="file-editor-heading">
              <code title={selected?.path}>
                {selected?.path ?? 'Выберите файл'}
                {dirty ? ' *' : ''}
              </code>
              <button
                className="button primary"
                disabled={locked || !dirty || !editable || review}
                onClick={() => void save()}
              >
                <Save size={14} /> Сохранить
              </button>
            </div>
            {selected && (
              <div className="file-editor-actions">
                {!selected.directory && (
                  <button className="button" disabled={locked} onClick={() => open(selected)}>
                    Перечитать с диска
                  </button>
                )}
                <button
                  className="button"
                  disabled={locked}
                  onClick={() =>
                    guard(() => {
                      setEditable(false);
                      setReview(false);
                      setReviewDirty(false);
                      setOperation('rename');
                      setName(selected.path);
                    })
                  }
                >
                  Переименовать / переместить
                </button>
                <button
                  className="button danger"
                  disabled={locked}
                  onClick={() =>
                    guard(() => {
                      setEditable(false);
                      setReview(false);
                      setReviewDirty(false);
                      setOperation('delete');
                    })
                  }
                >
                  Удалить…
                </button>
              </div>
            )}
            {selected && /\.(docx|xlsx|pptx|csv|tsv|md|txt)$/i.test(selected.path) && (
              <div className="review-mode-switch">
                <button
                  className={review ? 'selected' : ''}
                  disabled={locked || review}
                  onClick={() =>
                    guard(() => {
                      setEditable(false);
                      setReview(true);
                      setReviewKey((k) => k + 1);
                    })
                  }
                >
                  Просмотр и комментарии
                </button>
                {!/\.(docx|xlsx|pptx)$/i.test(selected.path) && (
                  <button
                    className={!review ? 'selected' : ''}
                    disabled={locked || !review}
                    onClick={() => open(selected, true)}
                  >
                    Исходный текст
                  </button>
                )}
              </div>
            )}
            {review && selected ? (
              <DocumentReview
                key={`${selected.path}:${reviewKey}`}
                client={client}
                workspaceId={workspace.id}
                path={selected.path}
                onDirty={setReviewDirty}
                onDiscuss={onDiscuss}
              />
            ) : editable ? (
              <textarea
                aria-label="Содержимое файла"
                className="file-content"
                spellCheck={false}
                value={content}
                disabled={locked}
                onChange={(event) => setContent(event.target.value)}
                onKeyDown={(event) => {
                  if ((event.ctrlKey || event.metaKey) && event.key === 's') {
                    event.preventDefault();
                    void save();
                  }
                }}
              />
            ) : (
              <div className="file-editor-empty">
                <File size={28} />
                <p>Откройте текстовый файл, документ или таблицу.</p>
                <span className="muted small">
                  До 2 МиБ · Ctrl S — сохранить · изменения записываются на ваш диск
                </span>
              </div>
            )}
          </section>
        </div>
      </div>
    </Dialog>
  );
}
