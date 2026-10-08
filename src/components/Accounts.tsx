import { useState } from 'react';
import {
  ArrowUpRight,
  ChevronDown,
  LogIn,
  LogOut,
  Pencil,
  Plug,
  Plus,
  RefreshCw,
  Server,
  Sparkles,
  Trash2,
  UserRound,
} from 'lucide-react';
import type {
  AccountProfile,
  AccountStatus,
  ClientTransport,
  Extensions,
  ProviderInfo,
  UsageWindow,
} from '../contracts';
import type { AccountState } from '../accounts';
import { errorText } from '../locale';
import { planLabel, resetLabel, usageTone, windowLabel } from '../usage';

const stateLabels: Record<AccountStatus['state'], string> = {
  signed_in: 'Вход выполнен',
  signed_out: 'Вход не выполнен',
  not_required: 'Вход не нужен',
  unavailable: 'CLI недоступен',
  error: 'Ошибка',
};

export function UsageBars({ usage, compact }: { usage: UsageWindow[]; compact?: boolean }) {
  if (!usage.length)
    return <p className="muted small">Провайдер не сообщает сведения об использовании.</p>;
  return (
    <div className={`usage-bars ${compact ? 'compact' : ''}`}>
      {usage.map((window, index) => (
        <div className="usage-row" key={`${window.window_minutes}-${index}`}>
          <span className="usage-name">{windowLabel(window.window_minutes)}</span>
          <span
            className={`usage-track ${usageTone(window)}`}
            role="meter"
            aria-label={`${windowLabel(window.window_minutes)}: использовано ${Math.round(window.used_percent)}%`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(window.used_percent)}
          >
            <span style={{ width: `${Math.max(2, window.used_percent)}%` }} />
          </span>
          <span className="usage-value">{Math.round(window.used_percent)}%</span>
          {!compact && <span className="usage-reset">{resetLabel(window.resets_at)}</span>}
        </div>
      ))}
    </div>
  );
}

export function ProviderAccounts({
  client,
  provider,
  accounts,
  state,
  onChanged,
}: {
  client: ClientTransport;
  provider: ProviderInfo;
  accounts: AccountProfile[];
  state: AccountState;
  onChanged: () => void;
}) {
  const [label, setLabel] = useState('');
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState('');
  const create = async () => {
    setError('');
    try {
      const account = await client.addAccount(provider.id, label);
      setLabel('');
      setAdding(false);
      onChanged();
      void state.refresh(account.id);
    } catch (error) {
      setError(errorText(error));
    }
  };
  return (
    <div className="provider-accounts">
      <div className="row-between">
        <strong className="section-label">Аккаунты</strong>
      </div>
      {accounts.map((account) => (
        <AccountCard
          key={account.id}
          client={client}
          account={account}
          state={state}
          onChanged={onChanged}
        />
      ))}
      {!accounts.length && (
        <p className="muted small">
          {provider.id === 'anthropic'
            ? 'Добавьте аккаунт Claude. Неизменённый официальный CLI откроет собственный вход Anthropic. Можно использовать поддерживаемую подписку или вход Console; использование оплачивается владельцем аккаунта.'
            : 'Добавьте аккаунт ChatGPT. Вход и разрешение на использование плана выполняются на официальной странице OpenAI.'}
        </p>
      )}
      {adding ? (
        <form
          className="account-add"
          onSubmit={(event) => {
            event.preventDefault();
            void create();
          }}
        >
          <input
            autoFocus
            aria-label="Название аккаунта"
            placeholder="Например: Личный"
            maxLength={40}
            value={label}
            onChange={(event) => setLabel(event.target.value)}
          />
          <button className="primary-button" type="submit" disabled={!label.trim()}>
            Добавить
          </button>
          <button type="button" className="secondary-button" onClick={() => setAdding(false)}>
            Отмена
          </button>
        </form>
      ) : (
        <button
          className="secondary-button"
          disabled={!provider.available}
          onClick={() => setAdding(true)}
        >
          <Plus size={14} /> Добавить аккаунт {provider.id === 'anthropic' ? 'Claude' : 'ChatGPT'}
        </button>
      )}
      {error && <p className="notice error">{error}</p>}
    </div>
  );
}

function AccountCard({
  client,
  account,
  state,
  onChanged,
}: {
  client: ClientTransport;
  account: AccountProfile;
  state: AccountState;
  onChanged: () => void;
}) {
  const status = state.statuses[account.id];
  const signingIn = state.signingIn[account.id];
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [renaming, setRenaming] = useState(false);
  const [label, setLabel] = useState(account.label);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [extensions, setExtensions] = useState<Extensions | null>(null);
  const [showExtensions, setShowExtensions] = useState(false);
  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await work();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  };
  const signedIn = status?.state === 'signed_in';
  const shownError = error || state.errors[account.id];
  return (
    <section className="account-card" aria-label={`Аккаунт ${account.label}`}>
      <div className="account-head">
        <span className="account-avatar">
          <UserRound size={16} />
        </span>
        <div className="account-identity">
          {renaming ? (
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void run(async () => {
                  await client.renameAccount(account.id, label);
                  setRenaming(false);
                  onChanged();
                });
              }}
            >
              <input
                autoFocus
                aria-label="Новое название аккаунта"
                maxLength={40}
                value={label}
                onChange={(event) => setLabel(event.target.value)}
              />
            </form>
          ) : (
            <strong>{account.label}</strong>
          )}
          <small>{status?.email ?? (status ? stateLabels[status.state] : 'Проверка…')}</small>
        </div>
        {status?.plan && <span className="plan-badge">{planLabel(status.plan)}</span>}
        <span className={`account-state ${status?.state ?? 'loading'}`}>
          <span className={`status-dot ${signedIn ? 'completed' : 'idle'}`} />
          {signingIn ? 'Ожидание входа…' : status ? stateLabels[status.state] : 'Проверка…'}
        </span>
      </div>

      {signedIn && status && (
        <>
          {status.usage.length > 0 && <UsageBars usage={status.usage} />}
          {status.limit_reached && (
            <p className="notice warning-notice">
              Этот аккаунт достиг текущего лимита провайдера. BebekonCode не переключает аккаунты
              автоматически — выберите другой аккаунт вручную при создании сессии.
            </p>
          )}
          {status.credits && <p className="muted small">Кредиты: {status.credits}</p>}
          {status.message && (
            <details className="small muted">
              <summary>Сведения аккаунта</summary>
              <p>{status.message}</p>
            </details>
          )}
        </>
      )}
      {status?.manage_usage_url && (
        <button
          className="text-button manage-usage"
          onClick={() => void run(() => client.openUsage(account.provider))}
        >
          Управлять использованием в {account.provider === 'anthropic' ? 'Claude' : 'ChatGPT'}{' '}
          <ArrowUpRight size={13} />
        </button>
      )}
      {signingIn && (
        <p className="muted small">
          {account.provider === 'anthropic'
            ? 'Завершите штатный вход Claude Code в браузере. BebekonCode не получает пароль или токены. Если страница не открылась, используйте claude auth login с CLAUDE_CONFIG_DIR, указанным ниже.'
            : 'Завершите вход на открывшейся странице OpenAI в браузере. Пароль вводится только там; BebekonCode хранит выданные токены зашифрованными (Windows DPAPI) и передаёт их только процессу Codex этого аккаунта.'}
        </p>
      )}
      {account.provider === 'anthropic' && account.config_dir && (
        <details className="small muted">
          <summary>Профиль Claude и альтернативный вход</summary>
          <p>
            Профиль этого аккаунта: <code className="path">{account.config_dir}</code>
          </p>
          <p>Для входа через Claude Console выполните в PowerShell:</p>
          <code className="path">
            $env:CLAUDE_CONFIG_DIR = '{account.config_dir.replaceAll("'", "''")}'; claude auth login
            --console
          </code>
          <p>Затем нажмите «Обновить». Все штатные способы входа CLI сохранены.</p>
        </details>
      )}

      <div className="account-actions">
        {!signedIn && (
          <button
            className="primary-button"
            disabled={busy || signingIn || status?.state === 'unavailable'}
            onClick={() =>
              void run(async () => {
                state.markSigningIn(account.id, true);
                try {
                  await client.accountLogin(account.id);
                } catch (error) {
                  state.markSigningIn(account.id, false);
                  throw error;
                }
              })
            }
          >
            <LogIn size={14} />{' '}
            {account.provider === 'anthropic' ? 'Войти через Claude Code' : 'Continue with ChatGPT'}
          </button>
        )}
        <button
          className="secondary-button"
          disabled={busy}
          onClick={() => void run(() => state.refresh(account.id))}
        >
          <RefreshCw size={13} className={busy ? 'spin' : ''} /> Обновить
        </button>
        {signedIn && account.provider !== 'anthropic' && (
          <button
            className="secondary-button"
            aria-expanded={showExtensions}
            disabled={busy}
            onClick={() => {
              setShowExtensions((value) => !value);
              if (!extensions)
                void run(async () => setExtensions(await client.accountExtensions(account.id)));
            }}
          >
            <Plug size={13} /> Плагины и MCP <ChevronDown size={13} />
          </button>
        )}
        <span className="account-actions-space" />
        <button
          className="icon-button"
          aria-label={`Переименовать ${account.label}`}
          title="Переименовать"
          disabled={busy}
          onClick={() => setRenaming((value) => !value)}
        >
          <Pencil size={14} />
        </button>
        {signedIn && (
          <button
            className="icon-button"
            aria-label={`Выйти из ${account.label}`}
            title="Выйти"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await client.accountLogout(account.id);
                await state.refresh(account.id);
                onChanged();
              })
            }
          >
            <LogOut size={14} />
          </button>
        )}
        <button
          className="icon-button"
          aria-label={`Удалить ${account.label}`}
          title="Удалить"
          disabled={busy}
          onClick={() => setConfirmRemove(true)}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {confirmRemove && (
        <div className="file-decision" role="alertdialog" aria-label="Удаление аккаунта">
          <p>
            Удалить «{account.label}»? Будет удалён локальный профиль провайдера, если он не
            используется чатами. Учётная запись провайдера и ваши проекты сохранятся.
          </p>
          <div>
            <button className="secondary-button" onClick={() => setConfirmRemove(false)}>
              Отмена
            </button>
            <button
              className="secondary-button danger"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  await client.removeAccount(account.id);
                  setConfirmRemove(false);
                  onChanged();
                })
              }
            >
              Удалить
            </button>
          </div>
        </div>
      )}

      {showExtensions && extensions && (
        <div className="extensions">
          <ExtensionList title="Плагины" icon={<Plug size={13} />} items={extensions.plugins} />
          <ExtensionList
            title="MCP-серверы"
            icon={<Server size={13} />}
            items={extensions.mcp_servers}
          />
          <ExtensionList title="Навыки" icon={<Sparkles size={13} />} items={extensions.skills} />
          {extensions.errors.map((message) => (
            <p className="muted small" key={message}>
              {message}
            </p>
          ))}
          <p className="muted small">
            {account.provider === 'anthropic'
              ? 'Расширения Claude не запускаются в текущем режиме Windows.'
              : 'Установка и настройка выполняются средствами Codex для этого аккаунта.'}
          </p>
        </div>
      )}
      {shownError && <p className="notice error">{shownError}</p>}
    </section>
  );
}

function ExtensionList({
  title,
  icon,
  items,
}: {
  title: string;
  icon: React.ReactNode;
  items: Extensions['plugins'];
}) {
  return (
    <div className="extension-group">
      <div className="extension-title">
        {icon} {title} <span className="muted">{items.length}</span>
      </div>
      {items.length ? (
        <ul>
          {items.map((item, index) => (
            <li key={`${item.name}-${index}`}>
              <span className={`status-dot ${item.enabled ? 'completed' : 'idle'}`} />
              <span className="extension-name">{item.name}</span>
              {item.detail && <span className="muted">{item.detail}</span>}
              {item.status && <span className="extension-status">{item.status}</span>}
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted small">Нет</p>
      )}
    </div>
  );
}
