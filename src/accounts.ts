import { useCallback, useEffect, useRef, useState } from 'react';
import type { AccountProfile, AccountStatus, ClientTransport } from './contracts';
import { errorText } from './locale';

export interface AccountState {
  statuses: Record<string, AccountStatus>;
  errors: Record<string, string>;
  signingIn: Record<string, boolean>;
  refresh: (accountId: string) => Promise<void>;
  markSigningIn: (accountId: string, value: boolean) => void;
}

/**
 * Live account status (sign-in, plan, usage) for accounts that need sign-in. Loaded once per
 * account and then updated from provider notifications; there is no periodic polling.
 */
export function useAccountState(
  client: ClientTransport | null,
  accounts: AccountProfile[],
  onAccountsChanged: () => void,
): AccountState {
  const [statuses, setStatuses] = useState<Record<string, AccountStatus>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [signingIn, setSigningIn] = useState<Record<string, boolean>>({});
  const requested = useRef(new Set<string>());
  const changed = useRef(onAccountsChanged);
  changed.current = onAccountsChanged;

  const refresh = useCallback(
    async (accountId: string) => {
      if (!client) return;
      try {
        const status = await client.accountStatus(accountId);
        setStatuses((current) => ({ ...current, [accountId]: status }));
        setErrors(({ [accountId]: _removed, ...rest }) => rest);
      } catch (error) {
        setErrors((current) => ({ ...current, [accountId]: errorText(error) }));
      }
    },
    [client],
  );
  const markSigningIn = useCallback(
    (accountId: string, value: boolean) =>
      setSigningIn((current) => ({ ...current, [accountId]: value })),
    [],
  );

  const managed = accounts
    .filter((account) => account.provider !== 'mock')
    .map((account) => account.id)
    .join('|');
  useEffect(() => {
    for (const id of managed.split('|').filter(Boolean)) {
      if (requested.current.has(id)) continue;
      requested.current.add(id);
      void refresh(id);
    }
  }, [managed, refresh]);

  useEffect(() => {
    if (!client) return;
    let unlisten: (() => void) | undefined;
    let alive = true;
    void client
      .subscribeAccounts((event) => {
        if (event.kind === 'usage' && event.status) {
          const update = event.status;
          setStatuses((current) => {
            const previous = current[event.account_id];
            if (!previous) return current;
            return {
              ...current,
              [event.account_id]: {
                ...previous,
                usage: update.usage.length ? update.usage : previous.usage,
                plan: previous.plan ?? update.plan,
                limit_reached: update.limit_reached,
                checked_at: update.checked_at,
              },
            };
          });
          return;
        }
        if (event.kind === 'notice' && event.message) {
          const message = event.message;
          setErrors((current) => ({ ...current, [event.account_id]: message }));
          return;
        }
        if (event.kind === 'login_failed') {
          markSigningIn(event.account_id, false);
          setErrors((current) => ({
            ...current,
            [event.account_id]: event.message ?? 'Вход не завершён',
          }));
          return;
        }
        if (event.kind === 'login_completed') {
          markSigningIn(event.account_id, false);
          if (event.message) {
            const message = event.message;
            setErrors((current) => ({ ...current, [event.account_id]: message }));
          }
        }
        void refresh(event.account_id);
        changed.current();
      })
      .then((stop) => {
        if (alive) unlisten = stop;
        else stop();
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [client, refresh, markSigningIn]);

  return { statuses, errors, signingIn, refresh, markSigningIn };
}
