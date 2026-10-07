import { useEffect, useState } from 'react';
import { Puzzle, Server, Sparkles } from 'lucide-react';
import type { ClientTransport, Extensions, Session, Snapshot } from '../contracts';
import { accountLabel, errorText } from '../locale';
import { Dialog } from './Dialog';

export function ExtensionsDialog({
  client,
  data,
  session,
  configure,
  close,
}: {
  client: ClientTransport;
  data: Snapshot;
  session?: Session;
  configure: (session: Session) => void;
  close: () => void;
}) {
  const [accountId, setAccountId] = useState(
    session?.account_profile_id ?? data.accounts[0]?.id ?? '',
  );
  const [inventory, setInventory] = useState<Extensions>();
  const [error, setError] = useState('');
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
  }, [client, accountId]);
  const agents = session
    ? [
        session,
        ...data.sessions.filter(
          (s) => s.parent_session_id === session.id && s.chat_mode !== 'task',
        ),
      ].filter((s) => s.account_profile_id === accountId)
    : [];
  return (
    <Dialog title="Плагины и инструменты" close={close} wide>
      <p className="muted dialog-description">
        Инструменты подключены к аккаунту. Для каждого агента можно выбрать свой набор.
      </p>
      <label className="field">
        Аккаунт
        <select
          aria-label="Аккаунт инструментов"
          value={accountId}
          onChange={(e) => setAccountId(e.target.value)}
        >
          {data.accounts.map((account) => (
            <option key={account.id} value={account.id}>
              {accountLabel(account)}
            </option>
          ))}
        </select>
      </label>
      {!accountId && <p className="muted">Сначала подключите аккаунт в разделе «Аккаунты».</p>}
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
      <div className="extension-catalog">
        {inventory &&
          (
            [
              ['plugins', 'Плагины', Puzzle],
              ['mcp_servers', 'MCP-серверы', Server],
              ['skills', 'Навыки', Sparkles],
            ] as const
          ).map(([kind, label, Icon]) => (
            <section key={kind}>
              <h3>
                <Icon size={18} /> {label}
              </h3>
              {!inventory[kind].length && <p className="muted small">Нет подключённых</p>}
              {inventory[kind].map((item) => (
                <div className="extension-catalog-row" key={item.id ?? item.name}>
                  <div>
                    <strong>{item.name}</strong>
                    {item.detail && <small>{item.detail}</small>}
                  </div>
                  <span className="muted small">
                    {item.enabled && item.status !== 'unsupported' ? 'Подключён' : 'Недоступен'}
                  </span>
                </div>
              ))}
            </section>
          ))}
      </div>
      <p className="muted small">
        Установка новых расширений здесь пока недоступна. Показаны инструменты из официального CLI
        выбранного аккаунта.
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
              {agent.role || (index === 0 ? 'Основной агент' : `Участник ${index + 1}`)} · выбрать
              инструменты
            </button>
          ))}
        </div>
      )}
    </Dialog>
  );
}
