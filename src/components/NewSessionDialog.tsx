import { useEffect, useState } from 'react';
import { Folder, Plus, Settings2 } from 'lucide-react';
import type { ClientTransport, CreateSession, ModelInfo, Snapshot } from '../contracts';
import type { AccountState } from '../accounts';
import { accountLabel, errorText } from '../locale';
import { planLabel } from '../usage';
import { UsageBars } from './Accounts';
import { Dialog } from './Dialog';

export function NewSession({
  client,
  data,
  accountState,
  initialProject,
  initialProvider,
  openAccounts,
  close,
  create,
  busy,
}: {
  client: ClientTransport;
  data: Snapshot;
  accountState: AccountState;
  initialProject: string;
  initialProvider?: string;
  openAccounts: () => void;
  close: () => void;
  create: (input: CreateSession) => void;
  busy: boolean;
}) {
  const [project, setProject] = useState(initialProject || data.workspaces[0]?.id || '');
  const [provider, setProvider] = useState(
    (initialProvider &&
      data.providers.find((item) => item.id === initialProvider && item.available)?.id) ||
      data.providers.find((item) => item.available)?.id ||
      '',
  );
  const accounts = data.accounts.filter((account) => account.provider === provider);
  const [account, setAccount] = useState(accounts[0]?.id ?? '');
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [model, setModel] = useState('');
  const [modelError, setModelError] = useState('');
  const [loadingModels, setLoadingModels] = useState(false);
  const [permissions, setPermissions] = useState('standard');
  const status = account ? accountState.statuses[account] : undefined;
  const needsSignIn = provider !== 'mock' && status?.state !== 'signed_in';

  useEffect(() => {
    setModels([]);
    setModel('');
    setModelError('');
    if (!account) return;
    let alive = true;
    setLoadingModels(true);
    void client
      .accountModels(account)
      .then((list) => {
        if (!alive) return;
        setModels(list);
        setModel((list.find((item) => item.is_default) ?? list[0])?.id ?? '');
      })
      .catch((error) => alive && setModelError(errorText(error)))
      .finally(() => alive && setLoadingModels(false));
    return () => {
      alive = false;
    };
  }, [client, account]);

  return (
    <Dialog title="Новая сессия агента" close={close}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          create({
            workspace_id: project,
            provider,
            account_profile_id: account,
            model,
            permission_profile: permissions,
          });
        }}
      >
        <p className="muted dialog-description">
          Сессия навсегда привязана к выбранным агенту и аккаунту.
        </p>
        <label className="field">
          Проект
          <select
            aria-label="Проект"
            value={project}
            onChange={(event) => setProject(event.target.value)}
          >
            {data.workspaces.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <div className="field-grid">
          <label className="field">
            Агент
            <select
              aria-label="Агент"
              value={provider}
              onChange={(event) => {
                setProvider(event.target.value);
                setAccount(
                  data.accounts.find((item) => item.provider === event.target.value)?.id ?? '',
                );
              }}
            >
              {data.providers.map((item) => (
                <option key={item.id} value={item.id} disabled={!item.available}>
                  {item.name}
                  {item.available ? '' : ' · недоступен'}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            Аккаунт
            <select
              aria-label="Аккаунт"
              value={account}
              disabled={!accounts.length}
              onChange={(event) => setAccount(event.target.value)}
            >
              {accounts.map((item) => {
                const itemStatus = accountState.statuses[item.id];
                const suffix =
                  item.provider === 'mock'
                    ? ''
                    : itemStatus?.state === 'signed_in'
                      ? itemStatus.plan
                        ? ` · ${planLabel(itemStatus.plan)}`
                        : ''
                      : ' · вход не выполнен';
                return (
                  <option key={item.id} value={item.id}>
                    {accountLabel(item)}
                    {suffix}
                  </option>
                );
              })}
            </select>
          </label>
        </div>
        {provider !== 'mock' && !accounts.length && (
          <div className="notice">
            Для этого агента нет аккаунтов.{' '}
            <button type="button" className="text-button" onClick={openAccounts}>
              <Settings2 size={13} /> Добавить аккаунт
            </button>
          </div>
        )}
        {account && needsSignIn && (
          <div className="notice">
            {status ? 'Вход в этот аккаунт не выполнен.' : 'Проверяем вход…'}{' '}
            <button type="button" className="text-button" onClick={openAccounts}>
              Открыть аккаунты
            </button>
          </div>
        )}
        {status?.state === 'signed_in' && (
          <div className="account-usage-preview">
            <UsageBars usage={status.usage} compact />
            {status.limit_reached && (
              <p className="notice warning-notice">
                Этот аккаунт достиг лимита провайдера. Выберите другой аккаунт или дождитесь сброса.
              </p>
            )}
          </div>
        )}
        <label className="field">
          Модель
          <select
            aria-label="Модель"
            value={model}
            disabled={loadingModels || !models.length}
            onChange={(event) => setModel(event.target.value)}
          >
            {loadingModels && <option>Загрузка списка моделей…</option>}
            {models.map((item) => (
              <option key={item.id} value={item.id} title={item.description}>
                {item.name}
                {item.is_default ? ' · по умолчанию' : ''}
              </option>
            ))}
          </select>
          {modelError && <span className="field-error">{modelError}</span>}
        </label>
        <div className="field">
          <span>Изоляция</span>
          <div className="isolation-choice">
            <Folder size={18} />
            <div>
              <strong>Текущий проект</strong>
              <p>
                {provider === 'mock'
                  ? 'Симулятор не читает и не меняет файлы.'
                  : 'Агент работает в папке проекта в песочнице провайдера.'}
              </p>
            </div>
            <span className="choice-check">✓</span>
          </div>
          <p className="small muted">Изолированные сессии Git worktree появятся позже.</p>
        </div>
        <label className="field">
          Разрешения
          <select
            aria-label="Разрешения"
            value={permissions}
            onChange={(event) => setPermissions(event.target.value)}
          >
            <option value="standard">Стандартный</option>
            <option value="read_only">Только чтение</option>
          </select>
        </label>
        <div className="dialog-footer">
          <button type="button" className="secondary-button" onClick={close}>
            Отмена
          </button>
          <button
            className="primary-button"
            type="submit"
            disabled={busy || !project || !provider || !account || !model || needsSignIn}
          >
            <Plus size={15} /> Создать сессию
          </button>
        </div>
      </form>
    </Dialog>
  );
}
