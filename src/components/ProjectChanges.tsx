import { useEffect, useState } from 'react';
import { GitBranch, RefreshCw } from 'lucide-react';
import type { ClientTransport, GitStatus, Workspace } from '../contracts';
import { errorText } from '../locale';
import { Dialog } from './Dialog';

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
  return (
    <Dialog title="Изменения проекта" close={close} wide>
      <div className="changes-heading">
        <span>
          <GitBranch size={17} /> {workspace.name}
          {status ? ` · ${status.branch}` : ''}
        </span>
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
      {status && (
        <>
          <div className="changed-files">
            {status.files.map((file) => (
              <div key={file.path}>
                <span className="file-status">{file.status}</span>
                <code>{file.path}</code>
              </div>
            ))}
          </div>
          {!status.files.length && <p className="muted">В рабочем дереве нет изменений.</p>}
          {!!status.files.length && (
            <pre className="diff changes-diff">
              {diff || 'Для новых и бинарных файлов текстовый diff недоступен.'}
            </pre>
          )}
        </>
      )}
    </Dialog>
  );
}
