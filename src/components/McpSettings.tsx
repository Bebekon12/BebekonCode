import { useEffect, useState } from 'react';
import type { AccountProfile, ClientTransport, LocalMcpServer } from '../contracts';
import { accountLabel, errorText } from '../locale';

export function McpSettings({
  client,
  accounts,
}: {
  client: ClientTransport;
  accounts: AccountProfile[];
}) {
  const available = accounts.filter((a) => a.provider === 'anthropic');
  const [accountId, setAccountId] = useState(available[0]?.id ?? '');
  const [servers, setServers] = useState<LocalMcpServer[]>();
  const [name, setName] = useState('');
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let alive = true;
    setServers(undefined);
    setError('');
    if (accountId)
      void client
        .accountMcp(accountId)
        .then((value) => {
          if (alive) setServers(value);
        })
        .catch((reason) => {
          if (alive) setError(errorText(reason));
        });
    return () => {
      alive = false;
    };
  }, [client, accountId]);
  const save = async (next: LocalMcpServer[]) => {
    setBusy(true);
    setError('');
    try {
      await client.saveAccountMcp(accountId, next);
      setServers(next);
      setName('');
      setCommand('');
      setArgs('');
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <p className="muted">
        Добавьте локальный MCP для отдельного аккаунта Claude. Затем выберите его через @ в
        сообщении или в инструментах чата. Сохранение не запускает сервер.
      </p>
      <div className="notice">
        Каждый вызов требует подтверждения. Сервер работает без песочницы Windows; захват экрана,
        мышь и клавиатура требуют отдельного подтверждения. Секреты и переменные окружения здесь не
        хранятся.
      </div>
      <p className="small muted">
        Добавление MCP для Codex пока недоступно: для полного доступа нужен отдельный мост
        подтверждений действий. Существующие настройки Codex доступны в инструментах чата.
      </p>
      {!available.length ? (
        <p>Сначала добавьте аккаунт Claude в разделе «Аккаунты и провайдеры».</p>
      ) : (
        <>
          <label>
            Аккаунт MCP
            <select
              aria-label="Аккаунт MCP"
              value={accountId}
              disabled={busy}
              onChange={(e) => setAccountId(e.target.value)}
            >
              {available.map((a) => (
                <option key={a.id} value={a.id}>
                  {accountLabel(a)}
                </option>
              ))}
            </select>
          </label>
          {servers?.map((server) => (
            <div className="provider-card" key={server.name}>
              <strong>{server.name}</strong>
              <p>
                <code>{server.command}</code>
              </p>
              <p className="small muted">{server.args.join(' · ')}</p>
              <button
                disabled={busy}
                onClick={() => void save(servers.filter((s) => s.name !== server.name))}
              >
                Удалить {server.name}
              </button>
            </div>
          ))}
          <form
            className="mcp-form"
            onSubmit={(e) => {
              e.preventDefault();
              if (servers && !busy)
                void save([
                  ...servers,
                  {
                    name: name.trim(),
                    command: command.trim(),
                    args: args.split('\n').filter((line) => line.length > 0),
                  },
                ]);
            }}
          >
            <label>
              Имя сервера
              <input
                aria-label="Имя MCP"
                value={name}
                onChange={(e) => setName(e.target.value)}
                pattern="[A-Za-z0-9-]+"
                maxLength={64}
                required
                disabled={busy || !servers}
              />
            </label>
            <label>
              Локальная программа (.exe)
              <input
                aria-label="Программа MCP"
                placeholder="C:\Tools\server.exe"
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                required
                disabled={busy || !servers}
              />
            </label>
            <label>
              Аргументы — по одному на строку
              <textarea
                aria-label="Аргументы MCP"
                value={args}
                onChange={(e) => setArgs(e.target.value)}
                disabled={busy || !servers}
                rows={3}
              />
            </label>
            <button type="submit" disabled={busy || !servers}>
              Добавить MCP
            </button>
          </form>
        </>
      )}
      {error && (
        <p role="alert" className="notice error">
          {error}
        </p>
      )}
    </>
  );
}
