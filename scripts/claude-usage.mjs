import { query } from '@anthropic-ai/claude-agent-sdk';

// No user message is ever yielded: initialize the official SDK only for its public usage method.
export async function readUsage(createQuery, { binary, profile }, controller) {
  const { signal } = controller;
  const session = createQuery({
    prompt: (async function* () {
      await new Promise((resolve) => {
        if (signal.aborted) resolve();
        else signal.addEventListener('abort', resolve, { once: true });
      });
    })(),
    options: {
      pathToClaudeCodeExecutable: binary,
      cwd: profile,
      env: { ...process.env, CLAUDE_CONFIG_DIR: profile },
      tools: [],
      mcpServers: {},
      strictMcpConfig: true,
      plugins: [],
      settingSources: [],
      settings: { disableAllHooks: true, enabledPlugins: {} },
      persistSession: false,
      permissionMode: 'default',
      extraArgs: { restricted: null },
      canUseTool: async () => ({ behavior: 'deny', message: 'Usage reader has no tools' }),
      abortController: controller,
    },
  });
  try {
    const value = await session.usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET({
      skipBehaviors: true,
    });
    return {
      session: value.session,
      subscription_type: value.subscription_type,
      rate_limits_available: value.rate_limits_available,
      rate_limits: value.rate_limits,
    };
  } finally {
    session.close();
  }
}

async function main() {
  const [binary, profile] = process.argv.slice(2);
  if (!binary || !profile || process.env.CLAUDE_CONFIG_DIR !== profile) process.exit(2);
  const controller = new AbortController();
  const deadline = setTimeout(() => controller.abort(), 18_000);
  try {
    const value = await readUsage(query, { binary, profile }, controller);
    process.stdout.write(JSON.stringify(value));
  } catch {
    // SDK errors can contain sensitive context. Rust supplies the public unavailable message.
    process.exitCode = 1;
  } finally {
    clearTimeout(deadline);
    controller.abort();
  }
}

if (process.env.BEBEKON_USAGE_HELPER === '1') await main();
