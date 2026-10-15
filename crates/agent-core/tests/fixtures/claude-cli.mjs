// Test-only CLI fixture; never a provider shim in the distributed application.
import fs from 'node:fs';
import net from 'node:net';
import { createInterface } from 'node:readline';
import { randomUUID } from 'node:crypto';
const args = process.argv.slice(2);
const send = (value) => process.stdout.write(JSON.stringify(value) + '\n');
if (process.env.ANTHROPIC_API_KEY || process.env.ACCESS_TOKEN || process.env.CODEX_HOME)
  process.exit(23);
if (args[0] === '--version') {
  console.log('2.1.294 (fixture only)');
  process.exit(0);
}
if (args[0] === 'auth') {
  if (fs.existsSync(`${process.env.CLAUDE_CONFIG_DIR}/fixture-auth-wait`)) {
    fs.writeFileSync(`${process.env.CLAUDE_CONFIG_DIR}/fixture-auth-started`, 'test only');
    await new Promise(() => setInterval(() => {}, 1000));
  }
  send({
    loggedIn: true,
    authMethod: 'claude.ai',
    email: 'fixture@example.invalid',
    subscriptionType: 'fixture',
  });
  process.exit(0);
}
const option = (name) => args[args.indexOf(name) + 1];
const permissionMode = option('--permission-mode');
if (
  args.includes('--restricted') !== (permissionMode !== 'bypassPermissions') ||
  !['default', 'dontAsk', 'plan', 'auto', 'bypassPermissions'].includes(permissionMode) ||
  option('--setting-sources') !== '' ||
  !args.includes('--strict-mcp-config')
)
  process.exit(24);
const settings = JSON.parse(option('--settings'));
if (
  settings.permissions.ask.includes('mcp__*') !== ['default', 'plan'].includes(permissionMode) ||
  ['Bash', 'PowerShell'].some(
    (tool) =>
      settings.permissions.ask.includes(tool) !==
      !['dontAsk', 'auto', 'bypassPermissions'].includes(permissionMode),
  ) ||
  ['Edit', 'Write'].some(
    (tool) =>
      settings.permissions.ask.includes(tool) !==
      (['default', 'plan'].includes(permissionMode) && !settings.permissions.allow?.length),
  )
)
  process.exit(25);
if (permissionMode !== 'default' && process.env.CLAUDE_CODE_SUBPROCESS_ENV_SCRUB) process.exit(26);
const pipe = settings.hooks.PreToolUse[0].hooks[0].args[1];
const exchange = (value) =>
  new Promise((resolve, reject) => {
    const socket = net.connect(pipe);
    let data = '';
    socket.on('error', reject);
    socket.on('connect', () => socket.write(JSON.stringify(value) + '\n'));
    socket.on('data', (chunk) => {
      data += chunk;
      if (data.includes('\n')) {
        socket.destroy();
        resolve(JSON.parse(data));
      }
    });
  });
const inputLines = createInterface({ input: process.stdin, crlfDelay: Infinity });
const inputs = inputLines[Symbol.asyncIterator]();
const first = await inputs.next();
let prompt = first.value || '';
const id = args.includes('--resume') ? option('--resume') : randomUUID();
const history = `${process.env.CLAUDE_CONFIG_DIR}/fixture-invocations.jsonl`;
fs.appendFileSync(
  history,
  JSON.stringify({
    id,
    tools: option('--tools'),
    model: option('--model'),
    effort: args.includes('--effort') ? option('--effort') : null,
    fast_mode: settings.fastMode,
    compact_window: settings.env?.CLAUDE_CODE_AUTO_COMPACT_WINDOW,
    compact_percent: settings.env?.CLAUDE_AUTOCOMPACT_PCT_OVERRIDE,
    permission_mode: permissionMode,
  }) + '\n',
);
send({ type: 'system', subtype: 'init', session_id: id, permissionMode });
const input = JSON.parse(prompt);
const compact = input.message.content?.[0]?.text === '/compact';
if (compact) {
  if (
    !args.includes('--resume') ||
    args.includes('--disable-slash-commands') ||
    option('--tools') !== ''
  )
    process.exit(27);
  send({ type: 'system', subtype: 'compact_boundary' });
  send({
    type: 'result',
    subtype: 'success',
    is_error: false,
    result: '',
    num_turns: 1,
    usage: { input_tokens: 50, output_tokens: 10 },
  });
  process.exit(0);
}
if (!args.includes('--disable-slash-commands')) process.exit(28);

if (prompt.includes('[EXIT_WITHOUT_RESULT]')) process.exit(0);
if (prompt.includes('[LIVE]')) {
  if (!args.includes('--replay-user-messages')) process.exit(29);
  const extra = JSON.parse((await inputs.next()).value);
  if (!extra.uuid || extra.message.content[0].text !== 'Live addition') process.exit(30);
  if (extra.message.content[1]?.type !== 'image') process.exit(31);
  send({ ...extra, session_id: id, parent_tool_use_id: null });
  send({ ...extra, session_id: id, parent_tool_use_id: null });
  send({
    type: 'result',
    subtype: 'success',
    uuid: randomUUID(),
    result: 'First result',
    num_turns: 1,
    usage: {
      input_tokens: 5,
      output_tokens: 2,
      cache_read_input_tokens: 0,
      cache_creation_input_tokens: 0,
    },
  });
}
if (prompt.includes('[WAIT]')) await new Promise(() => setInterval(() => {}, 1000));
if (prompt.includes('[LIMIT]')) {
  send({
    type: 'result',
    subtype: 'error_during_execution',
    is_error: true,
    result: 'You hit your usage limit',
  });
  process.exit(1);
}
if (prompt.includes('[WRITE]')) {
  const tool_input = { file_path: 'approved.txt', content: 'Approved through the local pipe' };
  const before = await exchange({ hook_event_name: 'PreToolUse', tool_name: 'Write', tool_input });
  if (before.hookSpecificOutput?.permissionDecision === 'ask') {
    const decision = await exchange({
      hook_event_name: 'PermissionRequest',
      tool_name: 'Write',
      tool_input,
    });
    if (decision.hookSpecificOutput.decision.behavior === 'allow')
      fs.writeFileSync('approved.txt', tool_input.content);
  } else if (
    (['auto', 'bypassPermissions'].includes(permissionMode) ||
      (permissionMode === 'dontAsk' && settings.permissions.allow?.includes('Write')) ||
      settings.permissions.allow?.includes('Write(./approved.txt)')) &&
    !before.hookSpecificOutput?.permissionDecision
  ) {
    fs.writeFileSync('approved.txt', tool_input.content);
  }
}
if (args.includes('--json-schema')) {
  send({
    type: 'result',
    subtype: 'success',
    structured_output: {
      tasks: [{ title: 'Claude fixture analysis', prompt: 'Read the project', agent: 0 }],
    },
  });
} else {
  send({ type: 'stream_event', event: { type: 'message_start' } });
  send({
    type: 'stream_event',
    event: { delta: { type: 'thinking_delta', thinking: 'This must never be stored' } },
  });
  send({ type: 'stream_event', event: { delta: { type: 'text_delta', text: 'Claude fixture ' } } });
  send({ type: 'stream_event', event: { delta: { type: 'text_delta', text: 'result' } } });
  send({
    type: 'assistant',
    message: { content: [{ type: 'text', text: 'Claude fixture result' }] },
  });
  send({
    type: 'result',
    uuid: randomUUID(),
    subtype: 'success',
    result: 'Claude fixture result',
    num_turns: 2,
    usage: {
      input_tokens: 10,
      output_tokens: 8,
      cache_read_input_tokens: 20,
      cache_creation_input_tokens: 30,
    },
    modelUsage: { 'claude-opus-5-5': { inputTokens: 100000 } },
    total_cost_usd: 12,
  });
}

inputLines.close();
process.stdin.destroy();
