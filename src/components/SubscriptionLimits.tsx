import { useEffect, useState } from 'react';
import { Activity, ArrowUpRight, ChevronDown, RefreshCw } from 'lucide-react';
import type { AccountProfile } from '../contracts';
import type { AccountState } from '../accounts';
import { planLabel, resetLabel, windowLabel } from '../usage';

export function SubscriptionLimits({
  accounts,
  state,
  collapsed,
  openUsage,
}: {
  accounts: AccountProfile[];
  state: AccountState;
  collapsed: boolean;
  openUsage: (provider: string) => void;
}) {
  const [open, setOpen] = useState(true);
  const [popup, setPopup] = useState(false);
  useEffect(() => setPopup(false), [collapsed]);
  const visible = collapsed ? popup : open;
  const [refreshing, setRefreshing] = useState(false);
  const connected = accounts.filter((a) => a.provider !== 'mock');
  return (
    <section
      className={`subscription-limits ${collapsed ? 'limits-collapsed' : ''}`}
      aria-label="Лимиты подписок"
    >
      <div className="limits-heading">
        <button
          title="Лимиты подписок"
          aria-expanded={visible}
          onClick={() => (collapsed ? setPopup(!popup) : setOpen(!open))}
        >
          <Activity size={17} />
          <span className="sidebar-label">Лимиты подписок</span>
          <ChevronDown size={13} className="sidebar-label" />
        </button>
        {!collapsed && (
          <button
            className="icon-button"
            title="Обновить все лимиты"
            aria-label="Обновить все лимиты"
            disabled={refreshing || !connected.length}
            onClick={async () => {
              setRefreshing(true);
              try {
                await Promise.allSettled(connected.map((a) => state.refresh(a.id)));
              } finally {
                setRefreshing(false);
              }
            }}
          >
            <RefreshCw size={13} className={refreshing ? 'spin' : ''} />
          </button>
        )}
      </div>
      {visible && (
        <div className="limits-accounts">
          {!connected.length && (
            <p className="small muted">Подключите ChatGPT или Claude в аккаунтах.</p>
          )}
          {connected.map((a) => {
            const s = state.statuses[a.id];
            const error = state.errors[a.id];
            return (
              <div className="limit-account" key={a.id}>
                <div className="limit-account-heading">
                  <span className={`provider-mark ${a.provider}`}>
                    {a.provider === 'anthropic' ? '✳' : '◎'}
                  </span>
                  <strong title={a.label}>{a.label}</strong>
                  <span className="small muted">
                    {s?.plan
                      ? planLabel(s.plan)
                      : a.provider === 'anthropic'
                        ? 'Claude'
                        : 'ChatGPT'}
                  </span>
                </div>
                {s?.state === 'signed_in' ? (
                  <>
                    {s.usage.map((w, i) => {
                      const remaining = Math.round(
                        Math.max(0, Math.min(100, 100 - w.used_percent)),
                      );
                      return (
                        <div
                          className="limit-window"
                          key={i}
                          title={`${resetLabel(w.resets_at)} · проверено ${new Date(s.checked_at * 1000).toLocaleTimeString('ru-RU')}`}
                        >
                          <div>
                            <span>{w.label ?? windowLabel(w.window_minutes)}</span>
                            <strong>{remaining}% осталось</strong>
                          </div>
                          <progress
                            aria-label={`${a.label}: ${w.label ?? windowLabel(w.window_minutes)}, осталось`}
                            max={100}
                            value={remaining}
                          />
                          <span className="limit-note">{resetLabel(w.resets_at)}</span>
                        </div>
                      );
                    })}
                    {!s.usage.length && <span className="limit-note">Лимиты недоступны</span>}
                    {s.usage_error && (
                      <p className="limit-note limit-stale" title={s.usage_error}>
                        {s.usage.length > 0 && 'Не удалось обновить · '}
                        {s.usage_error}
                      </p>
                    )}
                    {a.provider === 'anthropic' && (
                      <span className="limit-note">Claude SDK · экспериментальный API</span>
                    )}
                    {s.usage_detail && (
                      <details className="limit-cli-detail">
                        <summary>Статус Claude CLI</summary>
                        <pre>{s.usage_detail}</pre>
                      </details>
                    )}
                    {s.limit_reached && <span className="field-error">Лимит исчерпан</span>}
                    {s.usage.length > 0 && (
                      <span className="limit-note">
                        Данные на{' '}
                        {new Date(s.checked_at * 1000).toLocaleTimeString('ru-RU', {
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </span>
                    )}
                    {error && (
                      <span className="limit-note limit-stale" title={error}>
                        Не удалось обновить · данные на{' '}
                        {new Date(s.checked_at * 1000).toLocaleTimeString('ru-RU', {
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </span>
                    )}
                    <button
                      className="text-button limit-manage"
                      onClick={() => openUsage(a.provider)}
                      title="Открыть использование у провайдера"
                    >
                      Открыть мои лимиты <ArrowUpRight size={12} />
                    </button>
                  </>
                ) : (
                  <span className="limit-note">
                    {error
                      ? 'Не удалось обновить'
                      : !s
                        ? 'Проверка…'
                        : s.state === 'signed_out'
                          ? 'Нужен вход'
                          : 'Данные недоступны'}
                  </span>
                )}
              </div>
            );
          })}
          {connected.length > 0 && (
            <span className="limit-note">Официальные данные · вручную и по событиям Codex</span>
          )}
        </div>
      )}
    </section>
  );
}
