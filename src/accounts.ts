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

/** An auth/sandbox refresh without quota data must not erase a newer provider event. */
export function mergeStatusSnapshot(
  previous: AccountStatus | undefined,
  next: AccountStatus,
): AccountStatus {
  if (
    !previous ||
    previous.account_id !== next.account_id ||
    previous.state !== 'signed_in' ||
    next.state !== 'signed_in'
  )
    return next;
  if (!previous.usage.length || (next.usage.length && next.checked_at >= previous.checked_at))
    return next;
  return {
    ...next,
    usage: previous.usage,
    checked_at: previous.checked_at,
    credits: previous.credits,
    limit_reached: previous.limit_reached,
    usage_detail: previous.usage_detail ?? next.usage_detail,
  };
}

export function mergeUsageUpdate(
  previous: AccountStatus | undefined,
  update: AccountStatus,
): AccountStatus {
  if (!previous) return update;
  if (previous.account_id !== update.account_id || previous.state !== 'signed_in') return previous;
  const fresh = update.usage.length > 0 && update.checked_at >= previous.checked_at;
  return {
    ...previous,
    usage: fresh ? update.usage : previous.usage,
    plan: previous.plan ?? update.plan,
    credits: update.credits ?? previous.credits,
    limit_reached: fresh ? update.limit_reached : previous.limit_reached,
    checked_at: fresh ? update.checked_at : previous.checked_at,
    usage_detail: update.usage_detail ?? previous.usage_detail,
    usage_error: fresh ? null : previous.usage_error,
  };
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
  // Accounts whose last status refresh failed; a later quota update supersedes only that error.
  const refreshFailed = useRef(new Set<string>());
  const changed = useRef(onAccountsChanged);
  changed.current = onAccountsChanged;

  const refresh = useCallback(
    async (accountId: string) => {
      if (!client) return;
      try {
        const status = await client.accountStatus(accountId);
        refreshFailed.current.delete(accountId);
        setStatuses((current) => ({
          ...current,
          [accountId]: mergeStatusSnapshot(current[accountId], status),
        }));
        setErrors(({ [accountId]: _removed, ...rest }) => rest);
      } catch (error) {
        refreshFailed.current.add(accountId);
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
          const fresh = update.usage.length > 0;
          setStatuses((current) => ({
            ...current,
            [event.account_id]: mergeUsageUpdate(current[event.account_id], update),
          }));
          if (fresh && refreshFailed.current.delete(event.account_id))
            setErrors(({ [event.account_id]: _removed, ...rest }) => rest);
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
