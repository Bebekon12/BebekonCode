import { useEffect, useState } from 'react';
import {
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  Cpu,
  Puzzle,
  Info,
  Activity,
  Search,
  ArrowUpRight,
} from 'lucide-react';
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
import { McpSettings } from './McpSettings';
import product from '../../product.json';
import pkg from '../../package.json';
import { errorText } from '../locale';
import { planLabel, resetLabel, windowLabel } from '../usage';
const categories = [
  {
    id: 'MCP',
    label: 'MCP-серверы',
    icon: Puzzle,
    description: 'Локальные серверы, отдельные аккаунты и подтверждения вызовов.',
  },
  {
    id: 'Основные',
    label: 'Основные',
    icon: SlidersHorizontal,
    description: 'Оформление, чтение и поведение приложения.',
  },
  {
    id: 'Провайдеры',
    label: 'Аккаунты и провайдеры',
    icon: Cpu,
    description: 'Подключайте CLI и управляйте отдельными аккаунтами.',
  },
  {
    id: 'Лимиты',
    label: 'Лимиты и использование',
    icon: Activity,
    description: 'Доступный остаток подписок и переход к лимитам провайдера.',
  },
  {
    id: 'Разрешения',
    label: 'Доступ и безопасность',
    icon: ShieldCheck,
    description: 'Как работают песочница, подтверждения и доступ к проекту.',
  },
  {
    id: 'Возможности',
    label: 'Возможности',
    icon: Puzzle,
    description: 'Командная работа, инструменты и доступные расширения.',
  },
  {
    id: 'О программе и обновления',
    label: 'О программе',
    icon: Info,
    description: 'Версия приложения и проверенные обновления для Windows.',
  },
];
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
  const [search, setSearch] = useState('');
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
      <div className="settings-layout settings-redesign">
        <nav aria-label="Разделы настроек">
          <label className="settings-search">
            <Search size={15} />
            <input
              aria-label="Найти раздел настроек"
              placeholder="Найти раздел…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </label>
          {categories
            .filter((category) =>
              `${category.id} ${category.label} ${category.description}`
                .toLowerCase()
                .includes(search.toLowerCase()),
            )
            .map((category) => (
              <button
                key={category.id}
                aria-label={category.id}
                aria-current={tab === category.id ? 'page' : undefined}
                className={tab === category.id ? 'selected' : ''}
                disabled={installing}
                onClick={() => setTab(category.id)}
              >
                <category.icon size={18} />
                <span>{category.label}</span>
              </button>
            ))}
          {search &&
            !categories.some((category) =>
              `${category.id} ${category.label} ${category.description}`
                .toLowerCase()
                .includes(search.toLowerCase()),
            ) && <p className="small muted">Раздел не найден.</p>}
          <div className="settings-local-note">
            <ShieldCheck size={16} />
            <span>
              Локальная история
              <br />
              <small>На этом компьютере</small>
            </span>
          </div>
        </nav>
        <div className="settings-content">
          <header className="settings-section-heading">
            <span className="settings-eyebrow">BebekonCode · настройки</span>
            <h3>{categories.find((category) => category.id === tab)?.label}</h3>
            <p>{categories.find((category) => category.id === tab)?.description}</p>
          </header>
          {tab === 'Основные' && (
            <>
              <h4 className="settings-group-title">Внешний вид</h4>
              <div className="setting-row">
                <div>
                  <strong>Оформление</strong>
                  <p>Выберите удобную тему для работы.</p>
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
              <h4 className="settings-group-title">Обновления и хранение</h4>
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
                  <strong>Проекты и история</strong>
                  <p>Хранятся на этом компьютере. Телеметрия и удалённый сервер отключены.</p>
                </div>
                <ShieldCheck size={19} />
              </div>
            </>
          )}
          {tab === 'Лимиты' && (
            <>
              <div className="row-between settings-limit-toolbar">
                <span className="small muted">Обновление по запросу и событиям CLI</span>
                <button
                  className="secondary-button"
                  disabled={busy || !accounts.some((a) => a.provider !== 'mock')}
                  onClick={() =>
                    void run(async () => {
                      await Promise.allSettled(
                        accounts
                          .filter((a) => a.provider !== 'mock')
                          .map((a) => accountState.refresh(a.id)),
                      );
                    })
                  }
                >
                  <RefreshCw size={14} className={busy ? 'spin' : ''} /> Обновить лимиты
                </button>
              </div>
              <div className="settings-limit-grid">
                {accounts
                  .filter((a) => a.provider !== 'mock')
                  .map((account) => {
                    const status = accountState.statuses[account.id];
                    return (
                      <article className="settings-limit-card" key={account.id}>
                        <div className="row-between">
                          <strong>{account.label}</strong>
                          <span className="badge">
                            {status?.plan
                              ? planLabel(status.plan)
                              : account.provider === 'openai'
                                ? 'ChatGPT'
                                : 'Claude'}
                          </span>
                        </div>
                        {status?.state === 'signed_in' ? (
                          <>
                            {status.usage.map((window, index) => (
                              <div className="settings-usage-window" key={index}>
                                <div className="row-between">
                                  <span>{window.label ?? windowLabel(window.window_minutes)}</span>
                                  <strong>
                                    {Math.round(Math.max(0, 100 - window.used_percent))}% осталось
                                  </strong>
                                </div>
                                <progress
                                  max={100}
                                  value={Math.max(0, 100 - window.used_percent)}
                                  aria-label={`${account.label}: ${window.label ?? windowLabel(window.window_minutes)}, осталось`}
                                />
                                <small className="muted">{resetLabel(window.resets_at)}</small>
                              </div>
                            ))}
                            {!status.usage.length && (
                              <p className="muted">
                                Лимиты недоступны: провайдер не передал проценты.
                              </p>
                            )}
                            {status.usage_error && (
                              <p className="notice error">{status.usage_error}</p>
                            )}
                            {account.provider === 'anthropic' && (
                              <p className="small muted">
                                Claude SDK · экспериментальный API, требуется Node.js 18+.
                              </p>
                            )}
                            {status.limit_reached && (
                              <p className="notice error">
                                Провайдер сообщил об исчерпании лимита.
                              </p>
                            )}
                            {status.credits && <p className="small">Кредиты: {status.credits}</p>}
                            {status.usage_detail && (
                              <details>
                                <summary>Статус CLI</summary>
                                <pre className="settings-cli-status">{status.usage_detail}</pre>
                              </details>
                            )}
                          </>
                        ) : (
                          <p className="muted">
                            {!status
                              ? 'Проверка аккаунта…'
                              : status.state === 'signed_out'
                                ? 'Войдите в аккаунт в разделе «Аккаунты и провайдеры».'
                                : (status.message ?? 'Статус сейчас недоступен.')}
                          </p>
                        )}
                        {accountState.errors[account.id] && (
                          <p className="notice error" role="alert">
                            {accountState.errors[account.id]}
                          </p>
                        )}
                        {status?.usage.length ? (
                          <small className="muted">
                            Проверено {new Date(status.checked_at * 1000).toLocaleString('ru-RU')}
                          </small>
                        ) : null}
                        <button
                          className="secondary-button"
                          disabled={busy}
                          onClick={() => void run(() => client.openUsage(account.provider))}
                        >
                          Открыть мои лимиты <ArrowUpRight size={14} />
                        </button>
                      </article>
                    );
                  })}
              </div>
              {!accounts.some((a) => a.provider !== 'mock') && (
                <div className="settings-empty">
                  <Activity size={28} />
                  <h4>Подключите аккаунт</h4>
                  <p className="muted">Лимиты появятся после входа в ChatGPT или Claude.</p>
                  <button className="secondary-button" onClick={() => setTab('Провайдеры')}>
                    Перейти к аккаунтам
                  </button>
                </div>
              )}
            </>
          )}
          {tab === 'Провайдеры' && (
            <>
              <div className="row-between">
                <span className="small muted">Выберите провайдера, чтобы управлять аккаунтами</span>
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
              <p className="muted">
                Профиль выбирается для отдельного чата или участника команды. По умолчанию работают
                песочница и подтверждения провайдера.
              </p>
              <div className="provider-card">
                <strong>По правилам провайдера</strong>
                <p>
                  Claude спрашивает перед изменением файлов. Codex выполняет разрешённые команды и
                  правки в песочнице автоматически; отдельное подтверждение каждой правки Codex в
                  этой версии недоступно.
                </p>
                <p>
                  Команды shell и MCP у Claude на Windows работают без песочницы ОС и требуют
                  подтверждения каждого вызова, включая режим «Авто в проекте».
                </p>
              </div>
              <div className="provider-card">
                <strong>Только чтение</strong>
                <p>
                  Файлы защищены от записи. Codex может выполнять команды в песочнице только для
                  чтения. Повторная проверка участников команды всегда проходит в этом режиме.
                </p>
              </div>
              <div className="notice">
                «Авто в проекте»: Claude принимает правки проекта автоматически, Codex выполняет
                команды и правки в песочнице. Для Codex на Windows сначала настройте песочницу в
                разделе «Аккаунты». Дополнительный доступ требует подтверждения. Разрешение на
                контекст относится к запросу одного агента, а не ко всей команде.
              </div>
              <div className="provider-card">
                <strong>Полный доступ · только Codex</strong>
                <p>
                  Включается явно в отдельном чате после видимого предупреждения. Codex получает
                  доступ без песочницы и подтверждений команд. Режим недоступен для повторной
                  проверки и автоматических рабочих сессий. Учётные данные остаются запрещены.
                </p>
              </div>
            </>
          )}
          {tab === 'MCP' && <McpSettings client={client} accounts={accounts} />}
          {tab === 'Возможности' && (
            <>
              <div className="provider-card">
                <strong>Делегирование между агентами</strong>
                <p>
                  Команда: до трёх участников, общий итог от координатора. Раунд взаимной проверки
                  на чтение проходит при двух и более участниках; одного участника проверяет
                  координатор. При подключённых ChatGPT и Claude по умолчанию Codex координирует и
                  запускает проверки, Claude анализирует и правит код. Авторазбиение: до четырёх
                  подзадач на чтение. Модели, аккаунты и доступ выбираете вы.
                </p>
              </div>
              <div className="provider-card">
                <strong>Навыки Codex и вложения</strong>
                <p>
                  В разделе «Плагины → Установка Codex» можно подготовить запрос официальному
                  skill-installer. Для плагинов OpenAI API установки пока недоступен. Прикреплённые
                  изображения открываются по нажатию до отправки и в истории чата.
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
