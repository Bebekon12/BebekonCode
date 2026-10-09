import { useEffect, useState } from 'react';
import { Puzzle, Server, Sparkles, Search, RefreshCw, ArrowUpRight, Store } from 'lucide-react';
import type { ClientTransport, Extensions, Session, Snapshot, PluginCatalog } from '../contracts';
import { accountLabel, errorText } from '../locale';
import { Dialog } from './Dialog';
import { PluginManager } from './PluginManager';

export function ExtensionsDialog({
  client,
  data,
  session,
  configure,
  close,
  prepareInstall,
}: {
  client: ClientTransport;
  data: Snapshot;
  session?: Session;
  configure: (session: Session) => void;
  close: () => void;
  prepareInstall: (accountId: string, skill: string) => Promise<void>;
}) {
  const [accountId, setAccountId] = useState(
    session?.account_profile_id ??
      data.accounts.find((a) => a.provider !== 'mock' && a.auth_status === 'signed_in')?.id ??
      data.accounts.find((a) => a.provider !== 'mock')?.id ??
      '',
  );
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('all');
  const [inventory, setInventory] = useState<Extensions>();
  const [error, setError] = useState('');
  const [tab, setTab] = useState<'catalog' | 'connected' | 'install'>('catalog');
  const [skill, setSkill] = useState('');
  const [installAccount, setInstallAccount] = useState(
    data.accounts.find((a) => a.provider === 'openai' && a.auth_status === 'signed_in')?.id ?? '',
  );
  const [preparing, setPreparing] = useState(false);
  const [catalog, setCatalog] = useState<PluginCatalog>();
  const [catalogError, setCatalogError] = useState('');
  const [loading, setLoading] = useState(false);
  const [provider, setProvider] = useState('all');
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    setCatalogError('');
    void client
      .pluginCatalog()
      .then((value) => {
        if (alive) setCatalog(value);
      })
      .catch((reason) => {
        if (alive) setCatalogError(errorText(reason));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [client, revision]);
  useEffect(() => {
    let alive = true;
    setInventory(undefined);
    setError('');
    if (accountId)
      void client
        .accountExtensions(accountId)
        .then((result) => alive && setInventory(result))
        .catch((error) => alive && setError(errorText(error)));
    return () => {
      alive = false;
    };
  }, [client, accountId, tab]);
  const agents = session
    ? [
        session,
        ...data.sessions.filter(
          (s) => s.parent_session_id === session.id && s.chat_mode !== 'task',
        ),
      ].filter((s) => s.account_profile_id === accountId)
    : [];
  return (
    <Dialog title="Плагины и инструменты" close={close} wide workspace>
      <div className="extensions-workspace">
        <aside className="extensions-navigation">
          <div className="catalog-navigation" aria-label="Разделы плагинов">
            <button
              className={tab === 'catalog' ? 'selected' : ''}
              aria-pressed={tab === 'catalog'}
              onClick={() => setTab('catalog')}
            >
              <Store size={17} /> Каталог
            </button>
            <button
              className={tab === 'connected' ? 'selected' : ''}
              aria-pressed={tab === 'connected'}
              onClick={() => setTab('connected')}
            >
              <Puzzle size={17} /> Подключённые
            </button>
            <button
              className={tab === 'install' ? 'selected' : ''}
              aria-pressed={tab === 'install'}
              onClick={() => setTab('install')}
            >
              <Sparkles size={17} /> Установка Codex
            </button>
          </div>
          <label className="field plugin-account-select">
            Аккаунт
            <select
              aria-label="Аккаунт инструментов"
              value={accountId}
              onChange={(e) => setAccountId(e.target.value)}
            >
              <option value="">Без аккаунта</option>
              {data.accounts
                .filter((a) => a.provider !== 'mock' || a.id === session?.account_profile_id)
                .map((a) => (
                  <option key={a.id} value={a.id}>
                    {accountLabel(a)}
                  </option>
                ))}
            </select>
          </label>
          <span className="settings-local-note">
            <Puzzle size={15} /> Инструменты аккаунта
          </span>
        </aside>
        <div className="extensions-content">
          <header className="settings-section-heading">
            <h3>
              {tab === 'catalog'
                ? 'Плагины'
                : tab === 'connected'
                  ? 'Подключённые инструменты'
                  : 'Навыки Codex'}
            </h3>
          </header>
          {tab === 'catalog' ? (
            data.accounts.find((a) => a.id === accountId)?.provider === 'openai' ? (
              <PluginManager
                key={accountId}
                client={client}
                accountId={accountId}
                running={data.sessions.some(
                  (s) => s.account_profile_id === accountId && s.status === 'running',
                )}
              />
            ) : (
              <>
                <p className="small muted">
                  Для установки выберите аккаунт Codex. Плагины Claude в этом адаптере пока
                  недоступны.
                </p>
                <div className="catalog-search-row">
                  <label className="model-search">
                    <Search size={17} />
                    <input
                      aria-label="Поиск в каталоге"
                      placeholder="Найти плагин…"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                  </label>
                  <select
                    aria-label="Провайдер каталога"
                    value={provider}
                    onChange={(e) => setProvider(e.target.value)}
                  >
                    <option value="all">Все провайдеры</option>
                    <option value="openai">OpenAI / Codex</option>
                    <option value="anthropic">Claude</option>
                  </select>
                  <button
                    className="icon-button"
                    aria-label="Обновить каталог"
                    disabled={loading}
                    onClick={() => setRevision(revision + 1)}
                  >
                    <RefreshCw size={17} className={loading ? 'spin' : ''} />
                  </button>
                </div>
                {loading && (
                  <p role="status" className="muted">
                    Загружаю открытые каталоги…
                  </p>
                )}
                {catalogError || catalog?.errors.length ? (
                  <p role="alert" className="notice error">
                    {catalogError || catalog?.errors.join(' ')}
                  </p>
                ) : null}
                <div className="plugin-store-grid">
                  {catalog?.entries
                    .filter(
                      (item) =>
                        (provider === 'all' || provider === item.provider) &&
                        `${item.name} ${item.description} ${item.category}`
                          .toLowerCase()
                          .includes(query.toLowerCase()),
                    )
                    .map((item) => (
                      <article className="plugin-store-card" key={`${item.provider}:${item.name}`}>
                        <div className="plugin-store-heading">
                          <span className="extension-item-icon">
                            <Puzzle size={22} />
                          </span>
                          <div>
                            <h3>{item.name}</h3>
                            <small>
                              {item.provider === 'openai' ? 'OpenAI / Codex' : 'Claude'} ·{' '}
                              {item.category}
                            </small>
                          </div>
                        </div>
                        <p title={item.description}>{item.description}</p>
                        <div className="plugin-store-footer">
                          <span className="small muted">Только просмотр</span>
                          <button
                            className="text-button"
                            aria-label={`Источник ${item.name}`}
                            onClick={() =>
                              void client
                                .openCatalogSource(item.provider)
                                .catch((e) => setCatalogError(errorText(e)))
                            }
                          >
                            Источник <ArrowUpRight size={14} />
                          </button>
                        </div>
                      </article>
                    ))}
                </div>
                {catalog &&
                  !loading &&
                  !catalog.entries.some(
                    (item) =>
                      (provider === 'all' || provider === item.provider) &&
                      `${item.name} ${item.description} ${item.category}`
                        .toLowerCase()
                        .includes(query.toLowerCase()),
                  ) && (
                    <p className="extension-empty">Плагины не найдены. Попробуйте другой запрос.</p>
                  )}
                {catalog && (
                  <p className="small muted">
                    Предложений: {catalog.entries.length} · обновлено{' '}
                    {new Date(catalog.checked_at * 1000).toLocaleString('ru-RU')}
                  </p>
                )}
              </>
            )
          ) : tab === 'install' ? (
            <section className="skill-install-panel">
              <h3>Установить навык для Codex</h3>
              <p className="muted">
                Официальный skill-installer устанавливает навыки из каталога openai/skills. Запрос
                откроется в отдельном чате выбранного аккаунта. Проверьте его и отправьте; установка
                расходует лимит аккаунта и может потребовать подтверждения Codex.
              </p>
              <label className="field">
                Аккаунт Codex
                <select
                  aria-label="Аккаунт установки Codex"
                  value={installAccount}
                  disabled={preparing}
                  onChange={(e) => setInstallAccount(e.target.value)}
                >
                  <option value="">Выберите аккаунт</option>
                  {data.accounts
                    .filter((a) => a.provider === 'openai' && a.auth_status === 'signed_in')
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {accountLabel(a)}
                      </option>
                    ))}
                </select>
              </label>
              <label className="field">
                Название навыка
                <input
                  aria-label="Название устанавливаемого навыка"
                  placeholder="Например, gh-fix-ci или pdf"
                  value={skill}
                  disabled={preparing}
                  maxLength={64}
                  onChange={(e) => setSkill(e.target.value)}
                />
              </label>
              <p className="small muted">
                Для списка доступных навыков оставьте название пустым. Готовность установки
                проверяйте по ответу установщика и в разделе «Подключённые».
              </p>
              {skill && !/^[a-z0-9][a-z0-9-]{0,63}$/.test(skill) && (
                <p className="field-error">Используйте латинские строчные буквы, цифры и дефис.</p>
              )}
              <button
                className="primary-button"
                disabled={
                  preparing ||
                  !installAccount ||
                  (!!skill && !/^[a-z0-9][a-z0-9-]{0,63}$/.test(skill))
                }
                onClick={async () => {
                  setPreparing(true);
                  setError('');
                  try {
                    await prepareInstall(installAccount, skill);
                  } catch (reason) {
                    setError(errorText(reason));
                  } finally {
                    setPreparing(false);
                  }
                }}
              >
                {preparing
                  ? 'Подготавливаю чат…'
                  : skill
                    ? 'Подготовить установку'
                    : 'Посмотреть доступные навыки'}
              </button>
              {error && (
                <p className="notice error" role="alert">
                  {error}
                </p>
              )}
            </section>
          ) : (
            <>
              <p className="muted dialog-description">
                Инструменты подключены к аккаунту. Для каждого агента можно выбрать свой набор.
              </p>
              {!accountId && (
                <p className="muted">Сначала подключите аккаунт в разделе «Аккаунты».</p>
              )}
              {error && (
                <p className="notice error" role="alert">
                  {error}
                </p>
              )}
              {accountId && !inventory && !error && (
                <p className="muted" role="status">
                  Загрузка инструментов…
                </p>
              )}
              {inventory?.errors.map((error, index) => (
                <p className="notice error" key={index}>
                  {error}
                </p>
              ))}
              <div className="extension-toolbar">
                <label className="model-search">
                  <Search size={17} />
                  <input
                    aria-label="Поиск инструментов"
                    placeholder="Поиск плагинов, MCP и навыков…"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                  />
                </label>
                <div className="extension-tabs">
                  {[
                    ['all', 'Все'],
                    ['plugins', 'Плагины'],
                    ['mcp_servers', 'MCP'],
                    ['skills', 'Навыки'],
                  ].map(([id, label]) => (
                    <button
                      key={id}
                      className={category === id ? 'selected' : ''}
                      aria-pressed={category === id}
                      onClick={() => setCategory(id!)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="extension-catalog">
                {inventory &&
                  (
                    [
                      ['plugins', 'Плагины', Puzzle],
                      ['mcp_servers', 'MCP-серверы', Server],
                      ['skills', 'Навыки', Sparkles],
                    ] as const
                  )
                    .filter(([kind]) => category === 'all' || category === kind)
                    .map(([kind, label, Icon]) => (
                      <section key={kind}>
                        <h3>
                          <Icon size={18} /> {label}
                        </h3>
                        {!inventory[kind].filter((item) =>
                          `${item.name} ${item.detail ?? ''}`
                            .toLowerCase()
                            .includes(query.toLowerCase()),
                        ).length && (
                          <div className="extension-empty">
                            <Icon size={25} />
                            <p>{query ? 'Ничего не найдено' : 'Пока нет подключённых'}</p>
                            <span className="small muted">
                              {query
                                ? 'Попробуйте другое название.'
                                : 'Расширения появляются здесь после установки в официальном CLI аккаунта.'}
                            </span>
                          </div>
                        )}
                        {inventory[kind]
                          .filter((item) =>
                            `${item.name} ${item.detail ?? ''}`
                              .toLowerCase()
                              .includes(query.toLowerCase()),
                          )
                          .map((item) => (
                            <div className="extension-catalog-row" key={item.id ?? item.name}>
                              <div>
                                <span className="extension-item-icon">
                                  <Icon size={20} />
                                </span>
                                <strong>{item.name}</strong>
                                {item.detail && <small>{item.detail}</small>}
                              </div>
                              <span
                                className={`badge ${item.enabled && item.status !== 'unsupported' ? 'success' : ''}`}
                              >
                                {item.enabled && item.status !== 'unsupported'
                                  ? 'Подключён'
                                  : 'Недоступен'}
                              </span>
                            </div>
                          ))}
                      </section>
                    ))}
              </div>
              <p className="muted small">
                Навыки Codex устанавливаются во вкладке «Установка Codex». Показаны инструменты из
                официального CLI выбранного аккаунта; наличие в каталоге не означает, что инструмент
                подключён.
              </p>
              {agents.length > 0 && (
                <div className="extension-agent-actions">
                  <h3>Назначить агенту в текущем чате</h3>
                  {agents.map((agent, index) => (
                    <button
                      className="secondary-button"
                      disabled={session?.status === 'running'}
                      key={agent.id}
                      onClick={() => configure(agent)}
                    >
                      {agent.role || (index === 0 ? 'Основной агент' : `Участник ${index + 1}`)} ·
                      выбрать инструменты
                    </button>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}
