import { useEffect, useRef, useState } from 'react';
import { ArrowUpRight, Check, Download, RefreshCw, ShieldCheck } from 'lucide-react';
import type { AppUpdate, ClientTransport, ReleaseCheck, UpdateProgress } from '../contracts';
import { errorText, formatDate } from '../locale';
import pkg from '../../package.json';

type State =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'current'; checkedAt: number }
  | { kind: 'available'; update: AppUpdate }
  | { kind: 'manual'; release: ReleaseCheck; reason: string }
  | { kind: 'installing'; update: AppUpdate; progress: UpdateProgress | null };

const megabytes = (bytes: number) =>
  (bytes / 1024 / 1024).toLocaleString('ru-RU', { maximumFractionDigits: 1 });

/**
 * In-app updates through the official Tauri updater. Packages are installed only after their
 * signature matches the public key built into the app; without a signed manifest the panel
 * falls back to a link to the release page.
 */
export function UpdatePanel({
  client,
  runningSessions,
  updateRelease,
  setInstalling,
}: {
  client: ClientTransport;
  runningSessions: number;
  setInstalling: (value: boolean) => void;
  updateRelease: (value: ReleaseCheck) => void;
}) {
  const [state, setState] = useState<State>({ kind: 'idle' });
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState('');
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const checkNow = async () => {
    setError('');
    setConfirm(false);
    setState({ kind: 'checking' });
    try {
      const update = await client.checkUpdate();
      if (!mounted.current) return;
      setState(
        update
          ? { kind: 'available', update }
          : { kind: 'current', checkedAt: Math.floor(Date.now() / 1000) },
      );
    } catch (reason) {
      // Releases without a signed update manifest (before 0.3.2) can only be installed by hand.
      try {
        const release = await client.checkReleases(true);
        if (!mounted.current) return;
        updateRelease(release);
        setState({ kind: 'manual', release, reason: errorText(reason) });
      } catch (fallback) {
        if (!mounted.current) return;
        setState({ kind: 'idle' });
        setError(errorText(fallback));
      }
    }
  };

  const install = async (update: AppUpdate) => {
    if (runningSessions > 0) {
      setError('Остановите активные сессии перед установкой обновления.');
      return;
    }
    setInstalling(true);
    setError('');
    setConfirm(false);
    setState({ kind: 'installing', update, progress: null });
    try {
      await client.installUpdate(update.version, (progress) =>
        setState({ kind: 'installing', update, progress }),
      );
    } catch (reason) {
      setState({ kind: 'idle' });
      setError(
        `Обновление не установлено: ${errorText(reason)} Проверьте обновления ещё раз, чтобы повторить.`,
      );
    } finally {
      setInstalling(false);
    }
  };

  const busy = state.kind === 'checking' || state.kind === 'installing';
  return (
    <div className="update-panel">
      <div className="update-actions">
        <button className="primary-button" disabled={busy} onClick={() => void checkNow()}>
          <RefreshCw size={15} className={state.kind === 'checking' ? 'spin' : ''} />{' '}
          {state.kind === 'checking' ? 'Проверка…' : 'Проверить обновления'}
        </button>
        <button
          className="secondary-button"
          disabled={busy}
          onClick={() => void client.openReleases().catch((reason) => setError(errorText(reason)))}
        >
          История версий <ArrowUpRight size={14} />
        </button>
      </div>

      {state.kind === 'current' && (
        <div className="release-result">
          <div className="context-title">
            <Check size={17} /> Установлена актуальная версия · {pkg.version}
          </div>
          <p className="small muted">Проверено: {formatDate(state.checkedAt)}</p>
        </div>
      )}

      {(state.kind === 'available' || state.kind === 'installing') && (
        <div className="release-result">
          <div className="context-title">
            <Download size={17} /> Доступна версия {state.update.version}
          </div>
          <p className="small muted">
            Установлена {state.update.currentVersion}
            {state.update.date
              ? ` · выпуск от ${new Date(state.update.date).toLocaleDateString('ru-RU')}`
              : ''}
          </p>
          <pre className="release-notes">
            {state.update.notes || 'Описание изменений отсутствует.'}
          </pre>
          {state.kind === 'installing' ? (
            <div className="update-progress" role="status">
              <span
                className="usage-track"
                role="progressbar"
                aria-label="Загрузка обновления"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={
                  state.progress?.total
                    ? Math.min(
                        100,
                        Math.floor((state.progress.downloaded / state.progress.total) * 100),
                      )
                    : undefined
                }
              >
                <span
                  style={{
                    width: `${
                      state.progress?.total
                        ? Math.min(100, (state.progress.downloaded / state.progress.total) * 100)
                        : 8
                    }%`,
                  }}
                />
              </span>
              <span className="small muted">
                {state.progress?.phase === 'installing'
                  ? 'Установка и перезапуск…'
                  : state.progress?.phase === 'verifying'
                    ? 'Проверка подписи…'
                    : state.progress?.total
                      ? `Загрузка ${megabytes(state.progress.downloaded)} из ${megabytes(state.progress.total)} МБ`
                      : 'Загрузка и проверка подписи…'}
                {' · после установки приложение откроется снова'}
              </span>
            </div>
          ) : confirm ? (
            <div className="file-decision" role="alertdialog" aria-label="Установка обновления">
              <p>
                BebekonCode закроется, установит версию {state.update.version} и откроется снова.{' '}
                Проекты, сессии и аккаунты сохранятся.
              </p>
              <div>
                <button className="secondary-button" onClick={() => setConfirm(false)}>
                  Отмена
                </button>
                <button
                  className="primary-button"
                  disabled={runningSessions > 0}
                  onClick={() => void install(state.update)}
                >
                  Установить и перезапустить
                </button>
              </div>
            </div>
          ) : (
            <button className="primary-button" onClick={() => setConfirm(true)}>
              <Download size={15} /> Установить и перезапустить
            </button>
          )}
          {runningSessions > 0 && (
            <p className="notice">
              Остановите активные сессии ({runningSessions}) перед установкой.
            </p>
          )}
        </div>
      )}

      {state.kind === 'manual' && (
        <div className="release-result">
          <div className="context-title">
            {state.release.available ? <Download size={17} /> : <Check size={17} />}{' '}
            {state.release.available
              ? `Доступна версия ${state.release.latest_version}`
              : state.release.latest_version
                ? `Установлена актуальная версия · ${pkg.version}`
                : 'Стабильных выпусков пока нет'}
          </div>
          {state.release.available && (
            <>
              <p className="small muted">
                Не удалось подготовить установку внутри приложения. Вы можете скачать установщик со
                страницы выпуска.
              </p>
              <button
                className="secondary-button"
                onClick={() =>
                  void client.openReleases().catch((reason) => setError(errorText(reason)))
                }
              >
                Открыть выпуск и скачать <ArrowUpRight size={14} />
              </button>
            </>
          )}
          <p className="small muted">Подробность: {state.reason}</p>
        </div>
      )}

      <p className="small muted update-footnote">
        <ShieldCheck size={13} /> Обновления устанавливаются только после проверки цифровой подписи
        пакета. Проверка загружает сведения о выпуске; установщик скачивается только после вашего
        подтверждения.
      </p>
      {error && (
        <div role="alert" className="notice error">
          {error}
        </div>
      )}
    </div>
  );
}
