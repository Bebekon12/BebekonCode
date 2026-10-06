import type { AgentConfig, Session, Snapshot } from './contracts';

export const modeLabels = {
  single: 'Обычный чат',
  team: 'Команда агентов',
  auto: 'Авторазбиение',
  task: 'Подзадача',
};
export const effortLabels: Record<string, string> = {
  none: 'Без рассуждения',
  minimal: 'Минимальный',
  low: 'Низкий',
  medium: 'Средний',
  high: 'Высокий',
  xhigh: 'Очень высокий',
  max: 'Максимальный',
  ultra: 'Предельный',
};
export function sessionConfig(session: Session): AgentConfig {
  return {
    provider: session.provider,
    account_profile_id: session.account_profile_id,
    model: session.model,
    reasoning_effort: session.reasoning_effort,
    permission_profile: session.permission_profile,
    tools: JSON.parse(session.tool_policy || '{}'),
    role: session.role,
  };
}
export function defaultAgent(data: Snapshot): AgentConfig {
  const account =
    data.accounts.find(
      (a) =>
        a.provider !== 'mock' &&
        a.auth_status === 'signed_in' &&
        data.providers.some((p) => p.id === a.provider && p.available),
    ) ?? data.accounts.find((a) => data.providers.some((p) => p.id === a.provider && p.available));
  return {
    provider: account?.provider ?? '',
    account_profile_id: account?.id ?? '',
    model: '',
    reasoning_effort: null,
    permission_profile: 'standard',
    tools: {},
    role: '',
  };
}
