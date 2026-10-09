import { describe, expect, it } from 'vitest';
import type { AccountStatus } from './contracts';
import { mergeStatusSnapshot, mergeUsageUpdate } from './accounts';

const status: AccountStatus = {
  account_id: 'one',
  state: 'signed_in',
  email: null,
  plan: 'plus',
  usage: [{ used_percent: 25, window_minutes: 300, resets_at: 1000, source: 'codex' }],
  credits: '0',
  limit_reached: null,
  message: null,
  manage_usage_url: null,
  plan_usage_enabled: true,
  checked_at: 10,
};
describe('account quota events and refresh races', () => {
  it('retains a provider quota event when a later auth refresh has no percentages, without changing its timestamp', () => {
    const next = mergeStatusSnapshot(status, {
      ...status,
      usage: [],
      credits: null,
      checked_at: 20,
    });
    expect(next.usage).toEqual(status.usage);
    expect(next.checked_at).toBe(10);
    expect(next.credits).toBe('0');
  });
  it('accepts an event before the first status response, then preserves it through that response', () => {
    const event = mergeUsageUpdate(undefined, status);
    expect(mergeStatusSnapshot(event, { ...status, usage: [], checked_at: 5 }).usage).toEqual(
      status.usage,
    );
  });
  it('clears data on sign-out and keeps accounts separate', () => {
    const signedOut = { ...status, state: 'signed_out' as const, usage: [] };
    expect(mergeStatusSnapshot(status, signedOut).usage).toEqual([]);
    expect(mergeUsageUpdate(signedOut, status).state).toBe('signed_out');
    const other = { ...signedOut, account_id: 'two' };
    expect(mergeStatusSnapshot(status, other)).toEqual(other);
  });
  it('ignores out-of-order percentages and replaces them with a newer snapshot', () => {
    const older = { ...status, checked_at: 5, usage: [{ ...status.usage[0]!, used_percent: 50 }] };
    expect(mergeUsageUpdate(status, older).usage[0]?.used_percent).toBe(25);
    expect(mergeStatusSnapshot(status, { ...older, checked_at: 20 }).usage[0]?.used_percent).toBe(
      50,
    );
  });
  it('keeps quota errors visible with the old snapshot and clears them on a fresh account event', () => {
    const failed = mergeStatusSnapshot(status, {
      ...status,
      usage: [],
      checked_at: 20,
      usage_error: 'API unavailable',
    });
    expect(failed.usage).toEqual(status.usage);
    expect(failed.checked_at).toBe(10);
    expect(failed.usage_error).toBe('API unavailable');
    expect(mergeUsageUpdate(failed, { ...status, checked_at: 21 }).usage_error).toBeNull();
  });
});
