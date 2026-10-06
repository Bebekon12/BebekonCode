import { useEffect, useState } from 'react';
import {
  ArrowRight,
  FileCode2,
  FileMinus2,
  FilePen,
  FilePlus2,
  Folder,
  FolderOpen,
  GitBranch,
  HardDrive,
  Play,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Square,
  UserRound,
  Users,
  X,
} from 'lucide-react';
import type {
  AccountStatus,
  AccountProfile,
  ClientTransport,
  FileEntry,
  GitStatus,
  Workspace,
  Session,
} from '../contracts';
import {
  accountLabel,
  counted,
  displayPath,
  duration,
  errorText,
  formatStart,
  permissionLabel,
  statusLabels,
} from '../locale';
import { summarizeChanges } from '../git';
import { planLabel } from '../usage';
import { UsageBars } from './Accounts';

export function ContextPanel({
  client,
  workspace,
  session,
  account,
  accountStatus,
  providerName,
  openFiles,
  cancel,
  continueSession,
  close,
}: {
  client: ClientTransport | null;
  workspace?: Workspace;
  session?: Session;
  account?: AccountProfile;
  accountStatus?: AccountStatus;
  providerName?: string;
  openFiles: () => void;
  cancel: () => void;
  continueSession: () => void;
  close: () => void;
}) {
  const [git, setGit] = useState<GitStatus>();
  const [entries, setEntries] = useState<FileEntry[]>();
  const [diff, setDiff] = useState('');
  const [error, setError] = useState('');
  const [fileError, setFileError] = useState('');
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  const [expanded, setExpanded] = useState(false);
  const [actionError, setActionError] = useState('');
  const [now, setNow] = useState(() => Date.now() / 1000);
  const running = session?.status === 'running';
  useEffect(() => {
    setGit(undefined);
    setEntries(undefined);
    setDiff('');
    setError('');
    setFileError('');
    setExpanded(false);
    setActionError('');
    if (!client || !workspace) return;
    let alive = true;
    setBusy(true);
    void Promise.allSettled([
      client.gitStatus(workspace.id),
      client.listFiles(workspace.id, ''),
    ]).then(([status, files]) => {
      if (!alive) return;
      if (status.status === 'fulfilled') setGit(status.value);
      else setError(errorText(status.reason));
      if (files.status === 'fulfilled') setEntries(files.value);
      else setFileError(errorText(files.reason));
      setBusy(false);
    });
    return () => {
      alive = false;
    };
  }, [client, workspace?.id, revision]);
  useEffect(() => {
    setDiff('');
    if (!expanded || !client || !workspace) return;
    let alive = true;
    void client
      .gitDiff(workspace.id)
      .then((value) => {
        if (alive) setDiff(value);
      })
      .catch((error) => {
        if (alive) setError(errorText(error));
      });
    return () => {
      alive = false;
    };
  }, [expanded, client, workspace?.id, revision]);
  useEffect(() => {
    setNow(Date.now() / 1000);
    if (!running) return;
    // A coarse tick keeps elapsed time current without per-second rerenders.
    const timer = setInterval(() => setNow(Date.now() / 1000), 15_000);
    return () => clearInterval(timer);
  }, [running, session?.updated_at]);
  const files = entries?.filter((entry) => !entry.directory);
  const directories = entries?.filter((entry) => entry.directory);
  const changes = git && summarizeChanges(git.files);
  return (
    <aside className="context-panel" aria-label="Контекст проекта">
      <section className="context-card">
        <div className="card-heading">
          <FolderOpen className="cyan" size={22} />
          <h2>Рабочая область</h2>
          <button className="icon-button" aria-label="Закрыть панель контекста" onClick={close}>
            <X size={15} />
          </button>
        </div>
        <div className="workspace-card-project">
          <span className={`status-dot ${workspace ? 'completed' : 'idle'}`} />
          <div>
            <strong>{workspace?.name ?? 'Проект не выбран'}</strong>
            <code className="path" title={workspace && displayPath(workspace.root)}>
              {workspace ? displayPath(workspace.root) : 'Добавьте локальную папку, чтобы начать'}
            </code>
          </div>
        </div>
        {entries && (
          <div
            className="workspace-stats"
            title="Только корень папки проекта; служебные файлы исключены"
          >
            <div>
              <FileCode2 size={15} />
              <strong>{files?.length ?? 0}</strong>
              <small>файлов</small>
            </div>
            <div>
              <Folder size={15} />
              <strong>{directories?.length ?? 0}</strong>
              <small>папок</small>
            </div>
            <div>
              <HardDrive size={15} />
              <strong>{formatSize(files?.reduce((size, file) => size + file.size, 0) ?? 0)}</strong>
              <small>размер</small>
            </div>
          </div>
        )}
        <div className="workspace-card-actions">
          <button className="secondary-button" disabled={!client || !workspace} onClick={openFiles}>
            <FileCode2 size={13} /> Файлы
          </button>
          <button
            className="secondary-button"
            disabled={!client || !workspace}
            onClick={() => {
              if (client && workspace)
                void client
                  .openProject(workspace.id, false)
                  .catch((error) => setActionError(errorText(error)));
            }}
          >
            <FolderOpen size={13} /> В проводнике
          </button>
        </div>
        {fileError && <p className="muted small">{fileError}</p>}
        {actionError && <p className="notice">{actionError}</p>}
      </section>
      <section className="context-card">
        <div className="card-heading">
          <Play className="green" size={22} />
          <h2>Текущая сессия</h2>
          {session && (
            <span className={`step-pill ${session.status}`}>{statusLabels[session.status]}</span>
          )}
        </div>
        {session ? (
          <>
            <dl>
              <dt>Агент</dt>
              <dd className="agent-value">
                <Sparkles size={13} /> {providerName ?? session.provider}
              </dd>
              <dt>Модель</dt>
              <dd>{session.model}</dd>
              <dt>Аккаунт</dt>
              <dd>
                {accountLabel(account)}
                {accountStatus?.plan ? ` · ${planLabel(accountStatus.plan)}` : ''}
              </dd>
              <dt>Начало</dt>
              <dd>{formatStart(session.created_at)}</dd>
              <dt>Длительность</dt>
              <dd>
                {duration(
                  (running ? Math.max(now, session.updated_at) : session.updated_at) -
                    session.created_at,
                )}
              </dd>
            </dl>
            <div className="session-usage">
              <span className="session-usage-label">Использование</span>
              {accountStatus?.state === 'signed_in' ? (
                <UsageBars usage={accountStatus.usage} compact />
              ) : (
                <p className="muted small">
                  {session.provider === 'mock'
                    ? 'Локальный симулятор не расходует лимиты.'
                    : 'Нет данных от провайдера.'}
                </p>
              )}
            </div>
            <div className="session-card-actions">
              <button
                className="secondary-button danger-action"
                disabled={!running}
                onClick={cancel}
              >
                <Square size={12} /> Остановить
              </button>
              <button className="secondary-button" disabled={running} onClick={continueSession}>
                <Play size={13} /> Продолжить
              </button>
            </div>
          </>
        ) : (
          <p className="muted">Создайте сессию или выберите её слева.</p>
        )}
      </section>
      <section className="context-card">
        <div className="card-heading">
          <GitBranch className="blue" size={22} />
          <h2>Изменения</h2>
          {!!git?.files.length && <span className="step-pill warning-pill">Есть новые</span>}
          <button
            className="icon-button"
            aria-label="Обновить изменения Git"
            disabled={busy || !workspace}
            onClick={() => setRevision((value) => value + 1)}
          >
            <RefreshCw size={15} className={busy ? 'spin' : ''} />
          </button>
        </div>
        {busy && (
          <p className="muted" role="status">
            Чтение состояния Git…
          </p>
        )}
        {error && <p className="muted small">{error}</p>}
        {!workspace && <p className="muted">Выберите проект, чтобы посмотреть изменения.</p>}
        {git && changes && (
          <>
            <div className="branch">
              <GitBranch size={14} />
              {git.branch}
            </div>
            <ul className="change-counts">
              <li>
                <FilePen size={14} className="blue" />
                {counted(changes.modified, [
                  'изменённый файл',
                  'изменённых файла',
                  'изменённых файлов',
                ])}
              </li>
              <li>
                <FilePlus2 size={14} className="green" />
                {counted(changes.added, ['новый файл', 'новых файла', 'новых файлов'])}
              </li>
              <li>
                <FileMinus2 size={14} className="red" />
                {counted(changes.deleted, [
                  'удалённый файл',
                  'удалённых файла',
                  'удалённых файлов',
                ])}
              </li>
            </ul>
            <button
              className="secondary-button changes-button"
              aria-expanded={expanded}
              onClick={() => setExpanded((value) => !value)}
            >
              {expanded ? 'Скрыть изменения' : 'Открыть изменения'}
              <ArrowRight size={14} />
            </button>
            {expanded && (
              <>
                <div className="changed-files">
                  {git.files.map((file) => (
                    <div key={file.path}>
                      <span className="file-status">{file.status}</span>
                      <code title={file.path}>{file.path}</code>
                    </div>
                  ))}
                </div>
                {!git.files.length && (
                  <p className="muted small">В рабочем дереве нет изменений.</p>
                )}
                {!!git.files.length && (
                  <details className="diff-details">
                    <summary>Посмотреть diff</summary>
                    <pre className="diff">{diff || 'Нет diff для отслеживаемых файлов.'}</pre>
                  </details>
                )}
              </>
            )}
          </>
        )}
      </section>
      <section className="context-card">
        <div className="card-heading">
          <ShieldCheck className="blue" size={22} />
          <h2>Доступ и права</h2>
          <span className="step-pill idle">
            <UserRound size={11} /> Личная
          </span>
        </div>
        <dl>
          <dt>Владелец</dt>
          <dd>Вы, на этом компьютере</dd>
          <dt>Разрешения сессии</dt>
          <dd>{session ? permissionLabel(session.permission_profile) : '—'}</dd>
        </dl>
        <div className="access-sharing">
          <Users size={15} />
          <div>
            <strong>Совместный доступ</strong>
            <small>Недоступен: приложение работает без удалённого доступа и аккаунтов.</small>
          </div>
        </div>
      </section>
    </aside>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  const units = ['КБ', 'МБ', 'ГБ'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toLocaleString('ru-RU', { maximumFractionDigits: 1 })} ${units[unit]}`;
}
