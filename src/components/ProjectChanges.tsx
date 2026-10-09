import { useEffect, useMemo, useState } from 'react';
import { FilePlus2, FileText, GitBranch, RefreshCw } from 'lucide-react';
import type { ClientTransport, GitStatus, Workspace } from '../contracts';
import { diffTotals, parseDiff } from '../diffstat';
import { errorText } from '../locale';
import { Dialog } from './Dialog';
import { DiffCounts } from './RunChanges';

export function ProjectChanges({
  client,
  workspace,
  close,
}: {
  client: ClientTransport;
  workspace: Workspace;
  close: () => void;
}) {
  const [status, setStatus] = useState<GitStatus>();
  const [diff, setDiff] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(true);
  const [revision, setRevision] = useState(0);
  const [selected, setSelected] = useState('');
  useEffect(() => {
    let alive = true;
    setBusy(true);
    setError('');
    setStatus(undefined);
    setDiff('');
    void Promise.allSettled([client.gitStatus(workspace.id), client.gitDiff(workspace.id)]).then(
      ([status, diff]) => {
        if (!alive) return;
        if (status.status === 'fulfilled') setStatus(status.value);
        if (diff.status === 'fulfilled') setDiff(diff.value);
        const errors = [status, diff].flatMap((result) =>
          result.status === 'rejected' ? [errorText(result.reason)] : [],
        );
        setError([...new Set(errors)].join('\n'));
        setBusy(false);
      },
    );
    return () => {
      alive = false;
    };
  }, [client, workspace.id, revision]);
  const files = useMemo(() => parseDiff(diff), [diff]);
  const totals = diffTotals(files);
  const untracked = (status?.files ?? [])
    .filter(
      (file) => file.status === '??' && !files.some((diffFile) => diffFile.path === file.path),
    )
    .map((file) => file.path);
  const currentUntracked = untracked.includes(selected) ? selected : undefined;
  const current = files.find((file) => file.path === selected) ?? files[0];
  return (
    <Dialog title="Изменения проекта" close={close} wide>
      <div className="changes-heading">
        <span>
          <GitBranch size={17} /> {workspace.name}
          {status ? ` · ${status.branch}` : ''}
        </span>
        {status && (files.length > 0 || untracked.length > 0) && (
          <span className="changes-totals">
            {files.length + untracked.length} файлов
            <DiffCounts
              added={totals.added}
              removed={totals.removed}
              approximate={totals.overlapping}
            />
          </span>
        )}
        <button
          className="secondary-button"
          disabled={busy}
          onClick={() => setRevision((value) => value + 1)}
        >
          <RefreshCw size={15} /> Обновить
        </button>
      </div>
      {busy && (
        <p role="status" className="muted">
          Чтение изменений…
        </p>
      )}
      {error && (
        <p role="alert" className="notice error">
          {error}
        </p>
      )}
      {status && !files.length && !untracked.length && (
        <p className="muted">В рабочем дереве нет изменений.</p>
      )}
      {status && (files.length > 0 || untracked.length > 0) && (
        <div className="changes-review">
          <nav className="changes-files" aria-label="Изменённые файлы">
            {files.map((file) => (
              <button
                key={file.path}
                className={!currentUntracked && file.path === current?.path ? 'selected' : ''}
                aria-current={!currentUntracked && file.path === current?.path ? 'true' : undefined}
                onClick={() => setSelected(file.path)}
                title={file.path}
              >
                <FileText size={14} />
                <span>{file.path}</span>
                {file.binary ? (
                  <small className="muted">bin</small>
                ) : (
                  <DiffCounts added={file.added} removed={file.removed} />
                )}
              </button>
            ))}
            {untracked.map((path) => (
              <button
                key={path}
                className={currentUntracked === path ? 'selected' : ''}
                aria-current={currentUntracked === path ? 'true' : undefined}
                onClick={() => setSelected(path)}
                title={path}
              >
                <FilePlus2 size={14} />
                <span>{path}</span>
                <small className="diff-added">новый</small>
              </button>
            ))}
          </nav>
          <div className="changes-diff-view" aria-label="Изменения файла">
            {currentUntracked ? (
              <p className="muted changes-placeholder">
                Новый файл ещё не добавлен в Git, поэтому построчный diff недоступен. Откройте его в
                файловом менеджере проекта.
              </p>
            ) : current?.binary ? (
              <p className="muted changes-placeholder">
                Двоичный файл: построчный diff недоступен.
              </p>
            ) : current ? (
              <pre className="diff-lines">
                {current.lines.map((line, index) => (
                  <span key={index} className={lineClass(line)}>
                    {line || ' '}
                    {'\n'}
                  </span>
                ))}
              </pre>
            ) : null}
          </div>
        </div>
      )}
    </Dialog>
  );
}

function lineClass(line: string) {
  if (line.startsWith('@@')) return 'diff-hunk';
  if (/^(diff --git|index |--- |\+\+\+ |new file|deleted file|similarity|rename )/.test(line))
    return 'diff-meta';
  if (line.startsWith('+')) return 'diff-line-added';
  if (line.startsWith('-')) return 'diff-line-removed';
  return undefined;
}
