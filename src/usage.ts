import type { UsageWindow } from './contracts';

// Labels come from the window length the provider reports; nothing is assumed about order.
export function windowLabel(minutes: number | null): string {
  if (minutes === null) return 'Лимит';
  if (minutes === 300) return '5 часов';
  if (minutes === 10080) return 'Неделя';
  if (minutes === 43200) return '30 дней';
  if (minutes % 1440 === 0) return `${minutes / 1440} дн.`;
  if (minutes % 60 === 0) return `${minutes / 60} ч`;
  return `${minutes} мин`;
}

const plans: Record<string, string> = {
  free: 'Free',
  go: 'Go',
  plus: 'Plus',
  pro: 'Pro',
  prolite: 'Pro Lite',
  promax: 'Pro Max',
  team: 'Team',
  business: 'Business',
  enterprise: 'Enterprise',
  edu: 'Edu',
  api_key: 'API-ключ',
  unknown: 'Тариф не определён',
};
export const planLabel = (plan: string) =>
  plans[plan] ?? plan.replace(/_/g, ' ').replace(/^\w/, (letter) => letter.toUpperCase());

export function resetLabel(resetsAt: number | null, now = Date.now() / 1000): string {
  if (!resetsAt) return 'время сброса не сообщается';
  const date = new Date(resetsAt * 1000);
  const sameDay = new Date(now * 1000).toDateString() === date.toDateString();
  return `сброс ${
    sameDay
      ? date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })
      : date.toLocaleString('ru-RU', {
          day: 'numeric',
          month: 'short',
          hour: '2-digit',
          minute: '2-digit',
        })
  }`;
}

export const usageTone = (window: UsageWindow) =>
  window.used_percent >= 90 ? 'danger' : window.used_percent >= 70 ? 'warning' : 'ok';
