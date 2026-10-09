import { useEffect, useRef, useState } from 'react';
import { ChevronDown, Puzzle } from 'lucide-react';
import type {
  AgentConfig,
  ClientTransport,
  Extensions,
  ModelInfo,
  Snapshot,
  ToolPolicy,
} from '../contracts';
import { accountLabel, errorText } from '../locale';
import { ReasoningSlider } from './ModelPicker';
import { fullAccessProviders, fullAccessWarning } from './AccessPicker';

export function AgentPicker({
  client,
  data,
  value,
  change,
  lockedAccount = false,
  worker = false,
  initialToolsOpen = false,
}: {
  client: ClientTransport;
  data: Snapshot;
  value: AgentConfig;
  change: (value: AgentConfig) => void;
  lockedAccount?: boolean;
  worker?: boolean;
  initialToolsOpen?: boolean;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [toolsOpen, setToolsOpen] = useState(initialToolsOpen);
  const [extensions, setExtensions] = useState<Extensions>();
  const [toolsError, setToolsError] = useState('');
  const current = useRef({ value, change });
  current.current = { value, change };
  useEffect(() => {
    let alive = true;
    setModels([]);
    setError('');
    setLoading(true);
    if (!value.account_profile_id) {
      setLoading(false);
      return;
    }
    void client
      .accountModels(value.account_profile_id)
      .then((list) => {
        if (!alive) return;
        setModels(list);
        const { value: latest, change: patch } = current.current;
        const model =
          list.find((item) => item.id === latest.model) ??
          list.find((item) => item.is_default) ??
          list[0];
        patch({
          ...latest,
          model: model?.id ?? '',
          reasoning_effort: model?.reasoning_efforts?.includes(latest.reasoning_effort ?? '')
            ? latest.reasoning_effort
            : null,
        });
      })
      .catch((e) => alive && setError(errorText(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [client, value.account_profile_id]);
  useEffect(() => {
    setExtensions(undefined);
    setToolsError('');
    if (!toolsOpen || !value.account_profile_id) return;
    let alive = true;
    void client
      .accountExtensions(value.account_profile_id)
      .then((result) => alive && setExtensions(result))
      .catch((e) => alive && setToolsError(errorText(e)));
    return () => {
      alive = false;
    };
  }, [client, toolsOpen, value.account_profile_id]);
  const model = models.find((item) => item.id === value.model);
  const accounts = data.accounts.filter((a) => a.provider === value.provider);
  const selected = (kind: keyof ToolPolicy, id: string) =>
    value.tools[kind]?.includes(id) ?? !(value.provider === 'anthropic' && kind === 'mcp_servers');
  return (
    <div className="agent-picker">
      {value.provider === 'mock' && (
        <p className="small muted">
          Демонстрационный режим: без запросов к ИИ и работы с файлами. Для ответов ИИ подключите
          аккаунт.
        </p>
      )}
      {value.provider === 'anthropic' && (
        <details className="small muted">
          <summary>Возможности Claude в Windows</summary>
          <p>
            Модель и доступ проверяет Anthropic. Доступны файлы проекта и инструкции выбранных
            навыков аккаунта. Команды shell и локальный MCP требуют отдельного подтверждения:
            песочницы ОС Windows нет. Встроенный Skill, плагины и полный доступ Claude недоступны.
          </p>
        </details>
      )}
      {!lockedAccount && (
        <div className="field-grid">
          <label className="field">
            Провайдер
            <select
              aria-label="Провайдер"
              value={value.provider}
              onChange={(e) => {
                if (e.target.value === value.provider) return;
                const account =
                  data.accounts.find(
                    (a) => a.provider === e.target.value && a.auth_status === 'signed_in',
                  ) ?? data.accounts.find((a) => a.provider === e.target.value);
                change({
                  ...value,
                  provider: e.target.value,
                  account_profile_id: account?.id ?? '',
                  model: '',
                  reasoning_effort: null,
                  tools: {},
                  // Full access exists only for some providers; never carry it over silently.
                  permission_profile:
                    value.permission_profile === 'full_access'
                      ? 'standard'
                      : value.permission_profile,
                });
              }}
            >
              {data.providers.map((p) => (
                <option key={p.id} value={p.id} disabled={!p.available}>
                  {p.name}
                  {!p.available ? ' · недоступен' : ''}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            Аккаунт / подписка
            <select
              aria-label="Аккаунт"
              value={value.account_profile_id}
              onChange={(e) =>
                change({
                  ...value,
                  account_profile_id: e.target.value,
                  model: '',
                  reasoning_effort: null,
                  tools: {},
                })
              }
            >
              {!accounts.length && <option value="">Подключите аккаунт</option>}
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {accountLabel(a)}
                  {a.provider !== 'mock' && a.auth_status !== 'signed_in' ? ' · нужен вход' : ''}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      <div className="field-grid">
        <label className="field">
          Модель
          <select
            aria-label="Модель"
            value={value.model}
            disabled={loading || !models.length}
            onChange={(e) => change({ ...value, model: e.target.value, reasoning_effort: null })}
          >
            {loading && <option value={value.model}>Загрузка моделей…</option>}
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {value.provider === 'mock' ? 'Локальное демо' : m.name}
              </option>
            ))}
          </select>
        </label>
        <ReasoningSlider
          model={model}
          value={value.reasoning_effort}
          change={(reasoning_effort) => change({ ...value, reasoning_effort })}
        />
      </div>
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      <div className="field-grid">
        <label className="field">
          Доступ
          <select
            aria-label="Доступ"
            value={value.permission_profile}
            onChange={(e) => change({ ...value, permission_profile: e.target.value })}
          >
            <option value="standard">По правилам провайдера</option>
            <option value="workspace_auto">Авто в проекте</option>
            <option value="read_only">Только чтение</option>
            <option value="full_access" disabled={!fullAccessProviders.includes(value.provider)}>
              Полный доступ{fullAccessProviders.includes(value.provider) ? '' : ' — только Codex'}
            </option>
          </select>
          {value.permission_profile === 'full_access' ? (
            <small className="full-access-note" role="alert">
              {fullAccessWarning}
            </small>
          ) : (
            <small className="muted">
              {value.permission_profile === 'workspace_auto'
                ? 'Правки файлов проекта без повторных вопросов. Запросы дополнительных разрешений остаются видимыми.'
                : worker
                  ? 'Участники с правом записи работают по очереди, чтобы не перезаписывать изменения друг друга.'
                  : 'Доступ ограничен выбранным проектом.'}
            </small>
          )}
        </label>
        <label className="field">
          Роль
          <input
            aria-label="Роль"
            value={value.role}
            maxLength={160}
            placeholder={worker ? 'Например: проверить архитектуру' : 'Основной исполнитель'}
            onChange={(e) => change({ ...value, role: e.target.value })}
          />
        </label>
      </div>
      <button
        type="button"
        className="text-button"
        aria-expanded={toolsOpen}
        onClick={() => setToolsOpen(!toolsOpen)}
      >
        <Puzzle size={15} /> Плагины, MCP и навыки <ChevronDown size={13} />
      </button>
      {toolsOpen && (
        <div className="agent-tools">
          <p className="small muted">
            Настройки относятся к этому агенту. Здесь выбираются инструменты, уже установленные
            официальным CLI в профиле аккаунта.
          </p>
          {toolsError && <p className="field-error">{toolsError}</p>}
          {!extensions && !toolsError && <p className="small muted">Загрузка инструментов…</p>}
          {extensions?.errors.map((error) => (
            <p className="field-error" key={error}>
              {error}
            </p>
          ))}
          {extensions &&
            (['plugins', 'mcp_servers', 'skills'] as const).map((kind) => (
              <fieldset key={kind}>
                <legend>
                  {{ plugins: 'Плагины', mcp_servers: 'MCP-серверы', skills: 'Навыки' }[kind]}
                </legend>
                {!extensions[kind].length && <p className="small muted">Нет подключённых</p>}
                {extensions[kind].length > 0 &&
                  !(value.provider === 'anthropic' && kind === 'mcp_servers') && (
                    <label className="tool-choice">
                      <input
                        type="checkbox"
                        checked={value.tools[kind] == null}
                        onChange={(e) =>
                          change({
                            ...value,
                            tools: { ...value.tools, [kind]: e.target.checked ? null : [] },
                          })
                        }
                      />{' '}
                      Все доступные этому аккаунту
                    </label>
                  )}
                {extensions[kind].map((item) => (
                  <label
                    className="tool-choice"
                    key={item.id ?? item.name}
                    title={item.detail ?? undefined}
                  >
                    <input
                      type="checkbox"
                      disabled={!item.enabled || !item.id}
                      checked={item.enabled && selected(kind, item.id ?? '')}
                      onChange={(e) => {
                        const ids =
                          value.tools[kind] ??
                          (value.provider === 'anthropic' && kind === 'mcp_servers'
                            ? []
                            : extensions[kind].filter((i) => i.enabled && i.id).map((i) => i.id!));
                        change({
                          ...value,
                          tools: {
                            ...value.tools,
                            [kind]: e.target.checked
                              ? [...new Set([...ids, item.id!])]
                              : ids.filter((id) => id !== item.id),
                          },
                        });
                      }}
                    />
                    <span>
                      {item.name}
                      <small>{item.detail}</small>
                    </span>
                  </label>
                ))}
              </fieldset>
            ))}
        </div>
      )}
    </div>
  );
}
