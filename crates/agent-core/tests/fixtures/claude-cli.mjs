// Test-only CLI fixture; never a provider shim in the distributed application.
import fs from 'node:fs';
import net from 'node:net';
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
if (
  !args.includes('--restricted') ||
  !args.includes('--disable-slash-commands') ||
  option('--permission-mode') !== 'default' ||
  option('--setting-sources') !== '' ||
  !args.includes('--strict-mcp-config')
)
  process.exit(24);
const settings = JSON.parse(option('--settings'));
if (
  !['Edit', 'Write', 'Bash', 'PowerShell', 'mcp__*'].every((tool) =>
    settings.permissions.ask.includes(tool),
  )
)
  process.exit(25);
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
let prompt = '';
for await (const chunk of process.stdin) prompt += chunk;
const id = args.includes('--resume') ? option('--resume') : randomUUID();
const history = `${process.env.CLAUDE_CONFIG_DIR}/fixture-invocations.jsonl`;
fs.appendFileSync(
  history,
  JSON.stringify({
    id,
    tools: option('--tools'),
    model: option('--model'),
    effort: args.includes('--effort') ? option('--effort') : null,
  }) + '\n',
);
send({ type: 'system', subtype: 'init', session_id: id });
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
  if (before.hookSpecificOutput.permissionDecision === 'ask') {
    const decision = await exchange({
      hook_event_name: 'PermissionRequest',
      tool_name: 'Write',
      tool_input,
    });
    if (decision.hookSpecificOutput.decision.behavior === 'allow')
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
  send({ type: 'result', subtype: 'success', result: 'Claude fixture result' });
}
