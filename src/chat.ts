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

export const teamRoles = {
  coordinator: 'Координатор: проверки, команды и итог',
  implementer: 'Исполнитель: анализ кода, правки и интерфейс',
  commands: 'Исполнитель: команды, тесты и правки',
  reviewer: 'Рецензент',
};

function readyAccount(data: Snapshot, provider: string) {
  return data.accounts.find(
    (a) =>
      a.provider === provider &&
      a.auth_status === 'signed_in' &&
      data.providers.some((p) => p.id === provider && p.available),
  );
}

/** Second team member for an agent the user already configured: the other provider if ready. */
export function teamPartner(data: Snapshot, first: AgentConfig): AgentConfig {
  const base = defaultAgent(data);
  const partner =
    first.provider === 'openai' ? 'anthropic' : first.provider === 'anthropic' ? 'openai' : '';
  const account = partner ? readyAccount(data, partner) : undefined;
  if (!account) return { ...base, role: teamRoles.reviewer, permission_profile: 'read_only' };
  return {
    ...base,
    provider: partner,
    account_profile_id: account.id,
    role: partner === 'anthropic' ? teamRoles.implementer : teamRoles.commands,
  };
}

/**
 * Default team, split by the tools each provider actually has here: Codex runs commands and
 * tests, so it coordinates and verifies the final state; Claude (no Windows shell) reads and
 * edits code. The first agent is the coordinator. Access stays at the default; the user decides.
 */
export function defaultTeam(data: Snapshot): AgentConfig[] {
  const codex = readyAccount(data, 'openai');
  const claude = readyAccount(data, 'anthropic');
  const base = defaultAgent(data);
  if (codex && claude)
    return [
      {
        ...base,
        provider: 'openai',
        account_profile_id: codex.id,
        role: teamRoles.coordinator,
      },
      {
        ...base,
        provider: 'anthropic',
        account_profile_id: claude.id,
        role: teamRoles.implementer,
      },
    ];
  return [base, { ...base, role: teamRoles.reviewer, permission_profile: 'read_only' }];
}
