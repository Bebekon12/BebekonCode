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
import { Dialog } from './Dialog';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function FileManager({
  client,
  workspace,
  close,
}: {
  client: ClientTransport;
  workspace: Workspace;
  close: () => void;
}) {
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
  const dirty = editable && content !== original.replace(/\r\n/g, '\n');
  const locked = busy || !!decision;

  useEffect(() => {
    if (!isTauri() || (!dirty && !busy)) return;
    let alive = true;
    let unlisten: (() => void) | undefined;
    const window = getCurrentWindow();
    void window
      .onCloseRequested((event) => {
        event.preventDefault();
        if (busy) setError('Wait for the current file operation to finish.');
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
        if (alive) setError(String(error));
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
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }
  function open(entry: FileEntry) {
    guard(() => {
      clearEditor();
      setNotice('');
      if (entry.directory) {
        setDirectory(entry.path);
        return;
      }
      setSelected(entry);
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
      setNotice('Saved to disk.');
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
    <Dialog title={`${workspace.name} · Project files`} close={() => guard(close)} wide>
      <div className="file-manager">
        <code className="file-root" title={workspace.root}>
          {workspace.root}
        </code>
        {decision && (
          <div className="notice file-decision" role="alert">
            <span>Discard unsaved changes to {selected?.name}?</span>
            <button autoFocus className="button" onClick={() => setDecision(null)}>
              Keep editing
            </button>
            <button
              className="button danger"
              onClick={() => {
                const action = decision;
                setDecision(null);
                action();
              }}
            >
              Discard
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
            <Plus size={15} /> New file
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
            <FolderPlus size={15} /> New folder
          </button>
          <button
            className="button"
            disabled={locked}
            onClick={() => void perform(() => client.openProject(workspace.id, false))}
          >
            <FolderOpen size={15} /> Explorer
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
                Delete <strong>{selected?.path}</strong> permanently? This cannot be undone. Only
                files and empty folders can be deleted.
              </p>
            ) : (
              <label>
                {operation === 'rename'
                  ? 'New project-relative path'
                  : `New ${operation} name in ${directory || 'project root'}`}
                <input
                  aria-label="Filename"
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
              {operation === 'delete' ? 'Delete permanently' : 'Confirm'}
            </button>
            <button
              type="button"
              className="button"
              disabled={locked}
              onClick={() => setOperation(null)}
            >
              Cancel
            </button>
          </form>
        )}
        <div className="file-layout">
          <section className="file-browser" aria-label="Project file browser">
            <div className="file-navigation">
              <button
                className="icon-button"
                aria-label="Parent folder"
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
                aria-label="Refresh files"
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
                        ? 'Protected Git internals, link or unsupported name'
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
                <p className="muted small">No accessible entries in this folder.</p>
              )}
              {busy && (
                <p className="muted small" role="status">
                  Working with local files…
                </p>
              )}
            </div>
          </section>
          <section className="file-editor" aria-label="File editor">
            <div className="file-editor-heading">
              <code title={selected?.path}>
                {selected?.path ?? 'Select a file'}
                {dirty ? ' *' : ''}
              </code>
              <button
                className="button primary"
                disabled={locked || !dirty}
                onClick={() => void save()}
              >
                <Save size={14} /> Save
              </button>
            </div>
            {selected && (
              <div className="file-editor-actions">
                {!selected.directory && (
                  <button className="button" disabled={locked} onClick={() => open(selected)}>
                    Reload
                  </button>
                )}
                <button
                  className="button"
                  disabled={locked}
                  onClick={() =>
                    guard(() => {
                      setEditable(false);
                      setOperation('rename');
                      setName(selected.path);
                    })
                  }
                >
                  Rename / move
                </button>
                <button
                  className="button danger"
                  disabled={locked}
                  onClick={() =>
                    guard(() => {
                      setEditable(false);
                      setOperation('delete');
                    })
                  }
                >
                  Delete…
                </button>
              </div>
            )}
            {editable ? (
              <textarea
                aria-label="File content"
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
                <p>Open a UTF-8 text file to edit it.</p>
                <span className="muted small">
                  Up to 2 MiB · Ctrl S to save · changes are written to your local disk
                </span>
              </div>
            )}
          </section>
        </div>
      </div>
    </Dialog>
  );
}
