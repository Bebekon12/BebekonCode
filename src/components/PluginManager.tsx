import { useEffect, useState } from 'react';
import {
  Check,
  ChevronRight,
  Cuboid,
  FileText,
  Monitor,
  Plus,
  Presentation,
  Puzzle,
  RefreshCw,
  Search,
  Table2,
  Trash2,
  X,
} from 'lucide-react';
import type { ClientTransport, ProviderPlugin, PluginInventory } from '../contracts';
import { errorText } from '../locale';

const descriptions: Record<string, string> = {
  unity: 'Навыки и инструменты для работы с Unity.',
  documents: 'Создание и редактирование документов DOCX.',
  presentations: 'Создание презентаций PowerPoint.',
  spreadsheets: 'Работа с таблицами Excel и CSV.',
};
const titles: Record<string, string> = {
  unity: 'Unity',
  'unity-workbench': 'Unity Workbench',
  github: 'GitHub',
  gmail: 'Gmail',
  'google-drive': 'Google Drive',
  slack: 'Slack',
  notion: 'Notion',
  figma: 'Figma',
  canva: 'Canva',
  documents: 'Documents',
  presentations: 'Presentations',
  spreadsheets: 'Spreadsheets',
};
function pluginTitle(name: string) {
  return (
    titles[name] ??
    (/^(app|gpt)-[a-f0-9]{20,}$/i.test(name)
      ? `Плагин ${name.slice(0, 12)}…`
      : name.replaceAll('-', ' ').replace(/^./, (c) => c.toUpperCase()))
  );
}
function marketplaceLabel(value: string) {
  return value.startsWith('openai-')
    ? 'Каталог OpenAI'
    : value === 'unity-agent-plugin'
      ? 'Unity Technologies'
      : value;
}
function priority(entry: ProviderPlugin) {
  return entry.installed
    ? 0
    : entry.name === 'unity'
      ? 1
      : titles[entry.name]
        ? 2
        : /^(app|gpt)-/.test(entry.name)
          ? 4
          : 3;
}
function pluginIcon(name: string) {
  if (/unity/i.test(name)) return Cuboid;
  if (/doc|word/i.test(name)) return FileText;
  if (/presentation|ppt/i.test(name)) return Presentation;
  if (/spreadsheet|excel|xlsx/i.test(name)) return Table2;
  if (/computer|desktop/i.test(name)) return Monitor;
  return Puzzle;
}
const computerUse: ProviderPlugin = {
  id: 'computer-use:unavailable',
  name: 'Computer Use',
  marketplace: 'Codex desktop',
  version: null,
  description: 'Управление экраном, мышью и клавиатурой.',
  installed: false,
  enabled: false,
  can_remove: false,
  unavailable_reason:
    'Официальный плагин требует desktop-интеграции OpenAI. В BebekonCode она пока недоступна.',
  auth_policy: null,
};

export function PluginManager({
  client,
  accountId,
  running,
}: {
  client: ClientTransport;
  accountId: string;
  running: boolean;
}) {
  const [inventory, setInventory] = useState<PluginInventory>();
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<'all' | 'installed'>('all');
  const [selected, setSelected] = useState<ProviderPlugin>();
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [revision, setRevision] = useState(0);
  const [shown, setShown] = useState(80);
  useEffect(() => setShown(80), [search, filter, accountId]);
  useEffect(() => {
    let alive = true;
    setInventory(undefined);
    setSelected(undefined);
    setError('');
    setNotice('');
    setLoading(true);
    void client
      .accountPlugins(accountId)
      .then((result) => {
        if (alive) setInventory(result);
      })
      .catch((reason) => {
        if (alive) setError(errorText(reason));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [client, accountId, revision]);
  const run = async (action: () => Promise<void>, message: string) => {
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await action();
      const result = await client.accountPlugins(accountId);
      setInventory(result);
      setSelected((current) => result.entries.find((entry) => entry.id === current?.id));
      setNotice(message);
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  };
  const entries = inventory?.entries ?? [];
  const visible = [
    ...entries,
    ...(!entries.some((e) => /computer[-_ ]use/i.test(e.name)) ? [computerUse] : []),
  ]
    .filter(
      (e) =>
        (filter === 'all' || e.installed) &&
        `${e.name} ${e.description} ${e.marketplace}`.toLowerCase().includes(search.toLowerCase()),
    )
    .sort((a, b) => priority(a) - priority(b) || a.name.localeCompare(b.name));
  const locked = busy || loading || running;
  return (
    <section className="plugin-manager">
      <div className="plugin-manager-toolbar">
        <label className="model-search">
          <Search size={16} />
          <input
            aria-label="Поиск плагинов"
            placeholder="Найти плагин"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </label>
        <button
          className="icon-button"
          aria-label="Обновить плагины"
          title="Обновить"
          disabled={locked}
          onClick={() => setRevision((r) => r + 1)}
        >
          <RefreshCw size={16} className={loading ? 'spin' : ''} />
        </button>
      </div>
      <div className="plugin-filter-tabs" role="group" aria-label="Фильтр плагинов">
        <button aria-pressed={filter === 'all'} onClick={() => setFilter('all')}>
          Все
        </button>
        <button aria-pressed={filter === 'installed'} onClick={() => setFilter('installed')}>
          Установленные <span>{entries.filter((e) => e.installed).length}</span>
        </button>
      </div>
      <div className="plugin-source-actions">
        <span>Каталоги</span>
        <button
          className="secondary-button"
          disabled={locked}
          onClick={() => setRevision((r) => r + 1)}
        >
          <RefreshCw size={14} /> OpenAI
        </button>
        <button
          className="secondary-button"
          disabled={locked}
          onClick={() =>
            void run(
              () => client.addAccountPluginSource(accountId, 'unity'),
              'Каталог Unity подключён',
            )
          }
        >
          <Cuboid size={14} /> Unity
        </button>
      </div>
      {running && (
        <p className="small muted">Для изменения плагинов остановите задачи этого аккаунта.</p>
      )}
      {loading && (
        <p role="status" className="muted">
          Загрузка плагинов…
        </p>
      )}
      {error && (
        <p role="alert" className="notice error">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="plugin-success">
          <Check size={15} />
          {notice}
        </p>
      )}
      {!loading && (
        <div className={`plugin-browser ${selected ? 'has-selection' : ''}`}>
          <div className="plugin-list-grid">
            {visible.slice(0, shown).map((entry) => {
              const Icon = pluginIcon(entry.name);
              return (
                <article
                  className={`plugin-list-item ${selected?.id === entry.id ? 'selected' : ''}`}
                  key={entry.id}
                >
                  <button
                    className="plugin-open"
                    aria-label={`Подробнее: ${entry.name}`}
                    onClick={() => setSelected(entry)}
                    disabled={busy}
                  >
                    <span className={`plugin-logo ${entry.name === 'unity' ? 'unity' : ''}`}>
                      <Icon size={23} />
                    </span>
                    <span className="plugin-list-copy">
                      <strong>{pluginTitle(entry.name)}</strong>
                      <small>
                        {entry.description ||
                          descriptions[entry.name] ||
                          marketplaceLabel(entry.marketplace)}
                      </small>
                    </span>
                  </button>
                  {entry.installed ? (
                    <Check size={17} className="plugin-installed-mark" aria-label="Установлен" />
                  ) : (
                    <button
                      className="icon-button"
                      aria-label={`Установить ${entry.name}`}
                      title={entry.unavailable_reason ?? 'Установить'}
                      disabled={locked || !!entry.unavailable_reason}
                      onClick={() => setSelected(entry)}
                    >
                      <Plus size={19} />
                    </button>
                  )}
                </article>
              );
            })}
            {visible.length > shown && (
              <button
                className="secondary-button plugin-show-more"
                onClick={() => setShown((s) => s + 80)}
              >
                Показать ещё · {visible.length - shown}
              </button>
            )}
            {!visible.length && <p className="extension-empty">Плагины не найдены.</p>}
            {inventory && !entries.length && filter === 'all' && !search && (
              <p className="plugin-catalog-empty">
                Войдите через Codex для каталога OpenAI или подключите Unity.
              </p>
            )}
          </div>
          {selected && (
            <aside className="plugin-detail" aria-label={`Плагин ${selected.name}`}>
              <button
                className="icon-button plugin-detail-close"
                aria-label="Закрыть описание плагина"
                disabled={busy}
                onClick={() => setSelected(undefined)}
              >
                <X size={16} />
              </button>
              <span className="plugin-logo">
                {(() => {
                  const Icon = pluginIcon(selected.name);
                  return <Icon size={28} />;
                })()}
              </span>
              <h3>{pluginTitle(selected.name)}</h3>
              <span className="small muted">
                {marketplaceLabel(selected.marketplace)}
                {selected.version && ` · ${selected.version}`}
              </span>
              <p>
                {selected.description ||
                  descriptions[selected.name] ||
                  'Навыки и инструменты выбранного каталога.'}
              </p>
              {selected.unavailable_reason ? (
                <p className="plugin-unavailable">{selected.unavailable_reason}</p>
              ) : (
                <>
                  <p className="small muted">
                    Установка в выбранный аккаунт. Инструменты появятся в новом чате; сервисы могут
                    потребовать отдельный вход. Хуки отключены.
                  </p>
                  <button
                    className={selected.installed ? 'secondary-button' : 'primary-button'}
                    disabled={locked || (selected.installed && !selected.can_remove)}
                    onClick={() =>
                      void run(
                        () =>
                          client.changeAccountPlugin(accountId, selected.id, !selected.installed),
                        selected.installed
                          ? 'Плагин удалён'
                          : 'Плагин установлен. Начните новый чат.',
                      )
                    }
                  >
                    {selected.installed ? <Trash2 size={15} /> : <Plus size={15} />}
                    {busy
                      ? 'Подождите…'
                      : selected.installed
                        ? 'Удалить плагин'
                        : 'Установить плагин'}
                  </button>
                  {selected.installed && !selected.can_remove && (
                    <p className="small muted">Управляется провайдером.</p>
                  )}
                </>
              )}
              <details className="plugin-technical">
                <summary>
                  Подробности <ChevronRight size={13} />
                </summary>
                <p className="small muted">
                  {selected.id}
                  <br />
                  {selected.installed
                    ? selected.enabled
                      ? 'Установлен и включён'
                      : 'Установлен, отключён'
                    : 'Не установлен'}
                  {selected.auth_policy && (
                    <>
                      <br />
                      Авторизация: {selected.auth_policy}
                    </>
                  )}
                </p>
              </details>
            </aside>
          )}
        </div>
      )}
    </section>
  );
}
