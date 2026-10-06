import { describe, expect, it } from 'vitest';
import { planLabel, resetLabel, usageTone, windowLabel } from './usage';

describe('usage presentation', () => {
  it('names windows by their reported length', () => {
    expect(windowLabel(300)).toBe('5 часов');
    expect(windowLabel(10080)).toBe('Неделя');
    expect(windowLabel(43200)).toBe('30 дней');
    expect(windowLabel(120)).toBe('2 ч');
    expect(windowLabel(null)).toBe('Лимит');
  });
  it('shows plan names and keeps unknown ones readable', () => {
    expect(planLabel('plus')).toBe('Plus');
    expect(planLabel('prolite')).toBe('Pro Lite');
    expect(planLabel('self_serve_business_prolite')).toBe('Self serve business prolite');
  });
  it('never invents a reset time', () => {
    expect(resetLabel(null)).toBe('время сброса не сообщается');
  });
  it('flags high usage', () => {
    const window = { window_minutes: 300, resets_at: null, source: 'codex' };
    expect(usageTone({ ...window, used_percent: 95 })).toBe('danger');
    expect(usageTone({ ...window, used_percent: 75 })).toBe('warning');
    expect(usageTone({ ...window, used_percent: 10 })).toBe('ok');
  });
});
