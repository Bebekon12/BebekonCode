import { describe, expect, it } from 'vitest';
import { readUsage } from './claude-usage.mjs';

describe('official Claude quota SDK helper', () => {
  it('uses an empty input and isolated restricted options, closes the SDK and omits behaviors', async () => {
    let received;
    let argument;
    let closed = false;
    const controller = new AbortController();
    const value = await readUsage(
      (config) => {
        received = config;
        return {
          usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET: async (arg) => {
            argument = arg;
            return {
              session: {},
              subscription_type: 'pro',
              rate_limits_available: true,
              rate_limits: { five_hour: { utilization: 25, resets_at: null } },
              behaviors: { sensitive: 'must be omitted' },
            };
          },
          close: () => {
            closed = true;
          },
        };
      },
      { binary: 'official.exe', profile: 'isolated-account' },
      controller,
    );
    expect(argument).toEqual({ skipBehaviors: true });
    expect(closed).toBe(true);
    expect(value).not.toHaveProperty('behaviors');
    expect(received.options).toMatchObject({
      cwd: 'isolated-account',
      pathToClaudeCodeExecutable: 'official.exe',
      tools: [],
      mcpServers: {},
      plugins: [],
      strictMcpConfig: true,
      settingSources: [],
      persistSession: false,
      permissionMode: 'default',
      extraArgs: { restricted: null },
      settings: { disableAllHooks: true, enabledPlugins: {} },
      env: { CLAUDE_CONFIG_DIR: 'isolated-account' },
    });
    expect(await received.options.canUseTool()).toMatchObject({ behavior: 'deny' });
    const next = received.prompt.next();
    controller.abort();
    expect(await next).toEqual({ value: undefined, done: true });
  });
  it('closes the official SDK when its experimental API fails', async () => {
    let closed = false;
    await expect(
      readUsage(
        () => ({
          usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET: async () => {
            throw new Error('unsupported');
          },
          close: () => {
            closed = true;
          },
        }),
        { binary: 'official.exe', profile: 'account' },
        new AbortController(),
      ),
    ).rejects.toThrow('unsupported');
    expect(closed).toBe(true);
  });
});
