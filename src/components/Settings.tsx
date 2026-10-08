import { useEffect, useState } from 'react';
import { RefreshCw, ShieldCheck, SlidersHorizontal, Cpu, Puzzle, Info } from 'lucide-react';
import { Dialog } from './Dialog';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type {
  AccountProfile,
  ClientTransport,
  ProviderInfo,
  ReleaseCheck,
  Settings as AppSettings,
} from '../contracts';
import type { AccountState } from '../accounts';
import { ProviderAccounts } from './Accounts';
import { UpdatePanel } from './UpdatePanel';
import product from '../../product.json';
import pkg from '../../package.json';
import { errorText } from '../locale';
export function Settings({
  client,
  appearance,
  initialTab,
  settings,
  providers,
  accounts,
  accountState,
  onAccountsChanged,
  runningSessions,
  setUpdateBusy,
  updateSettings,
  updateProviders,
  updateRelease,
  close,
}: {
  client: ClientTransport;
  appearance: {
    theme: 'dark' | 'light';
    textSize: 'comfortable' | 'large';
    setTheme: (theme: 'dark' | 'light') => void;
    setTextSize: (size: 'comfortable' | 'large') => void;
  };
  initialTab?: string;
  settings: AppSettings;
  providers: ProviderInfo[];
  accounts: AccountProfile[];
  accountState: AccountState;
  onAccountsChanged: () => void;
  runningSessions: number;
  setUpdateBusy: (value: boolean) => void;
  updateSettings: (value: AppSettings) => void;
  updateProviders: (value: ProviderInfo[]) => void;
  release: ReleaseCheck | null;
  updateRelease: (value: ReleaseCheck) => void;
  close: () => void;
}) {
  const [tab, setTab] = useState(initialTab ?? 'Основные');
  const [providerId, setProviderId] = useState(
    accounts.find((a) => a.provider !== 'mock')?.provider ?? 'openai',
  );
  const [busy, setBusy] = useState(false);
  const [installing, setInstallingState] = useState(false);
  const setInstalling = (value: boolean) => {
    setInstallingState(value);
    setUpdateBusy(value);
  };
  useEffect(() => {
    if (!installing || !isTauri()) return;
    let alive = true;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested((event) => event.preventDefault())
      .then((remove) => {
        if (alive) unlisten = remove;
        else remove();
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [installing]);
  const [error, setError] = useState('');
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title="Настройки"
      close={() => {
        if (!installing) close();
      }}
      wide
      closeDisabled={installing}
    >
      <div className="settings-layout">
        <nav aria-label="Разделы настроек">
          {['Основные', 'Провайдеры', 'Разрешения', 'Возможности', 'О программе и обновления'].map(
            (category) => (
              <button
                key={category}
                aria-label={category}
                className={tab === category ? 'selected' : ''}
                disabled={installing}
                onClick={() => setTab(category)}
              >
                {category === 'Основные' ? (
                  <SlidersHorizontal size={17} />
                ) : category === 'Провайдеры' ? (
                  <Cpu size={17} />
                ) : category === 'Разрешения' ? (
                  <ShieldCheck size={17} />
                ) : category === 'Возможности' ? (
                  <Puzzle size={17} />
                ) : (
                  <Info size={17} />
                )}
                <span>{category === 'О программе и обновления' ? 'О программе' : category}</span>
              </button>
            ),
          )}
        </nav>
        <div className="settings-content">
          {tab === 'Основные' && (
            <>
              <h3>Ваша локальная рабочая область</h3>
              <p className="muted">Проекты и история сессий хранятся на этом компьютере.</p>
              <div className="setting-row">
                <div>
                  <strong>Оформление</strong>
                  <p>Спокойные цвета и локальный шрифт Inter.</p>
                </div>
                <select
                  aria-label="Тема оформления"
                  value={appearance.theme}
                  onChange={(event) =>
                    appearance.setTheme(event.target.value === 'light' ? 'light' : 'dark')
                  }
                >
                  <option value="dark">Тёмная</option>
                  <option value="light">Светлая</option>
                </select>
              </div>
              <div className="setting-row">
                <div>
                  <strong>Размер текста</strong>
                  <p>Текст переписки и элементы интерфейса.</p>
                </div>
                <select
                  aria-label="Размер текста"
                  value={appearance.textSize}
                  onChange={(event) =>
                    appearance.setTextSize(event.target.value === 'large' ? 'large' : 'comfortable')
                  }
                >
                  <option value="comfortable">Комфортный</option>
                  <option value="large">Крупный</option>
                </select>
              </div>
              <div className="setting-row">
                <div>
                  <strong>Проверять обновления при запуске</strong>
                  <p>
                    Проверка новых версий при запуске. Установка внутри приложения по подтверждению.
                  </p>
                </div>
                <input
                  aria-label="Проверять обновления при запуске"
                  type="checkbox"
                  checked={settings.check_updates_on_start}
                  disabled={busy}
                  onChange={(event) => {
                    const next = { check_updates_on_start: event.target.checked };
                    void run(async () => {
                      await client.saveSettings(next);
                      updateSettings(next);
                    });
                  }}
                />
              </div>
              <div className="setting-row">
                <div>
                  <strong>Удалённый доступ и телеметрия</strong>
                  <p>В этой версии недоступны. Сетевого сервера и аналитики нет.</p>
                </div>
                <ShieldCheck size={19} />
              </div>
            </>
          )}
          {tab === 'Провайдеры' && (
            <>
              <div className="row-between">
                <h3>Провайдеры агентов</h3>
                <button
                  className="secondary-button"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => updateProviders(await client.refreshProviders()))
                  }
                >
                  <RefreshCw size={14} /> Обновить
                </button>
              </div>
              <div className="provider-tabs" aria-label="Выбрать провайдера">
                {providers.map((p) => (
                  <button
                    key={p.id}
                    aria-pressed={providerId === p.id}
                    className={providerId === p.id ? 'selected' : ''}
                    onClick={() => setProviderId(p.id)}
                  >
                    <span className={`provider-mark ${p.id}`}>
                      {p.id === 'anthropic' ? '✳' : p.id === 'openai' ? '◎' : '◇'}
                    </span>
                    <strong>
                      {p.id === 'anthropic' ? 'Claude' : p.id === 'openai' ? 'ChatGPT' : 'Демо'}
                    </strong>
                    <small>{p.available ? 'Готов' : 'Нужна установка'}</small>
                  </button>
                ))}
              </div>
              {providers
                .filter((p) => p.id === providerId)
                .map((provider) => (
                  <div className="provider-card" key={provider.id}>
                    <div className="row-between">
                      <strong>{provider.name}</strong>
                      <span className={`badge ${provider.available ? 'success' : ''}`}>
                        {provider.available
                          ? 'Готов'
                          : provider.detected_path
                            ? 'Найден · не подключён'
                            : 'Не найден'}
                      </span>
                    </div>
                    <details className="provider-details small muted">
                      <summary>О подключении и возможностях</summary>
                      <p>{provider.detail}</p>
                      {provider.detected_path && (
                        <code className="path">{provider.detected_path}</code>
                      )}
                      <p>
                        Вход выполняется через официальные средства провайдера. Аккаунты независимы.
                      </p>
                    </details>
                    {provider.id === 'openai' && !provider.available && (
                      <div className="setup-hint">
                        <strong>Установка Codex CLI</strong>
                        <p>Выполните в терминале официальную команду и нажмите «Обновить»:</p>
                        <code>npm install -g @openai/codex</code>
                      </div>
                    )}
                    {provider.id === 'anthropic' && !provider.available && (
                      <div className="setup-hint">
                        <strong>Установка Claude Code для Windows</strong>
                        <p>
                          Установите официальный нативный CLI по документации Anthropic. Для уже
                          установленного CLI выполните:
                        </p>
                        <code>claude update</code>
                      </div>
                    )}
                    {['openai', 'anthropic'].includes(provider.id) && (
                      <ProviderAccounts
                        client={client}
                        provider={provider}
                        accounts={accounts.filter((account) => account.provider === provider.id)}
                        state={accountState}
                        onChanged={onAccountsChanged}
                      />
                    )}
                  </div>
                ))}
              <p className="small muted">
                Аккаунты независимы: сессия всегда работает на выбранном при создании аккаунте, а
                при исчерпании лимита приложение не переключается на другой автоматически.
              </p>
            </>
          )}
          {tab === 'Разрешения' && (
            <>
              <h3>Профили разрешений</h3>
              <p className="muted">
                Песочница и подтверждения провайдера всегда включены. BebekonCode добавляет свой
                слой правил, но не отключает защиту Codex или Claude Code.
              </p>
              <div className="provider-card">
                <strong>Стандартный</strong>
                <p>
                  Чтение и запись в проекте, просмотр Git разрешены. Запись вне проекта, команды,
                  сеть, удаление, коммиты и отправка изменений требуют подтверждения.
                </p>
              </div>
              <div className="provider-card">
                <strong>Только чтение</strong>
                <p>
                  Чтение файлов проекта и просмотр Git разрешены. Запись и выполнение команд
                  запрещены.
                </p>
              </div>
              <div className="notice">
                Codex: «Стандартный» — песочница с записью в проекте, «Только чтение» — песочница
                только для чтения. Когда Codex просит выйти за рамки, в ленте появляется запрос с
                командой и папкой: разрешить один раз, на сессию или отклонить.
              </div>
            </>
          )}
          {tab === 'Возможности' && (
            <>
              <h3>Возможности провайдеров</h3>
              <p className="muted">Возможности зависят от официального CLI выбранного аккаунта.</p>
              <div className="provider-card">
                <strong>Делегирование между агентами</strong>
                <p>
                  Команда: до трёх участников, два параллельных анализа, один раунд обсуждения и
                  общий итог. Авторазбиение: до четырёх подзадач. Модели и аккаунты выбираете вы.
                </p>
              </div>
              <div className="provider-card">
                <strong>Генерация изображений</strong>
                <p>
                  Провайдер генерации изображений не настроен. Потребуется отдельное подключение
                  официального API.
                </p>
              </div>
            </>
          )}
          {tab === 'О программе и обновления' && (
            <>
              <div className="about-brand">
                <img src="/brand/snowman.png" alt="" />
                <div>
                  <h3>{product.name}</h3>
                  <p>Версия {pkg.version} · приложение для Windows</p>
                </div>
              </div>
              <p className="muted">{product.description}</p>
              <div className="notice">
                Подключены официальные Codex app-server и Claude Code CLI. Для Claude в Windows
                доступны файлы проекта; оболочка и расширения пока недоступны.
              </div>
              <UpdatePanel
                client={client}
                setInstalling={setInstalling}
                runningSessions={runningSessions}
                updateRelease={updateRelease}
              />
            </>
          )}
          {error && (
            <div role="alert" className="notice error">
              {error}
            </div>
          )}
        </div>
      </div>
    </Dialog>
  );
}
