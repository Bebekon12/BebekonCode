// Packaged application test in an isolated data profile; only the mock provider runs.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile } from 'node:fs/promises';
import { execFileSync, spawn } from 'node:child_process';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { chromium, expect } from '@playwright/test';
const version = JSON.parse(
  await readFile(new URL('../package.json', import.meta.url), 'utf8'),
).version;
const root = await mkdtemp(join(tmpdir(), 'bebekon-workflows-native-'));
const project = join(root, 'project');
await mkdir(project);
await writeFile(join(project, 'sample.txt'), 'before\n');
for (const args of [
  ['init', '--quiet'],
  ['add', 'sample.txt'],
  [
    '-c',
    'user.name=Fixture',
    '-c',
    'user.email=fixture@example.invalid',
    '-c',
    'core.hooksPath=/dev/null',
    'commit',
    '--quiet',
    '-m',
    'fixture',
  ],
])
  execFileSync('git', args, { cwd: project, windowsHide: true, stdio: 'pipe' });
const binary = resolve(process.env.BEBEKON_TEST_BINARY || 'target/release/bebekoncode-desktop.exe');
const child = spawn(binary, ['--data-dir', join(root, 'data')], {
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    WEBVIEW2_USER_DATA_FOLDER: join(root, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
      '--remote-debugging-port=9239 --remote-debugging-address=127.0.0.1',
  },
});
let browser;
try {
  for (let i = 0; i < 100; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9239');
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  assert(browser, 'Packaged WebView did not start');
  let page;
  for (let i = 0; i < 50; i++) {
    page = browser
      .contexts()
      .flatMap((c) => c.pages())
      .find((p) => p.url().includes('tauri.localhost'));
    if (page) break;
    await new Promise((r) => setTimeout(r, 100));
  }
  assert(page, 'Packaged page missing');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
  const result = await page.evaluate(async (project) => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const workspace = await invoke('add_workspace', { root: project });
    const session = await invoke('create_session', {
      input: {
        workspace_id: workspace.id,
        provider: 'mock',
        account_profile_id: 'mock-local',
        model: 'mock-stream-v1',
        permission_profile: 'standard',
      },
    });
    const point = await invoke('create_checkpoint', {
      sessionId: session.id,
      label: 'Native before',
    });
    await invoke('file_operation', {
      workspaceId: workspace.id,
      operation: { kind: 'save', path: 'sample.txt', expected: 'before\n', content: 'after\n' },
    });
    const diff = await invoke('checkpoint_diff', { sessionId: session.id, id: point.id });
    if (diff.length !== 1) throw new Error('Missing checkpoint diff');
    await invoke('restore_checkpoint_file', {
      sessionId: session.id,
      id: point.id,
      path: diff[0].path,
      expected: diff[0].expected,
    });
    const restored = await invoke('file_operation', {
      workspaceId: workspace.id,
      operation: { kind: 'read', path: 'sample.txt' },
    });
    const branch = await invoke('branch_chat', { sessionId: session.id, isolated: true });
    await invoke('enqueue_task', { sessionId: session.id, prompt: 'Native queue 1' });
    await invoke('enqueue_task', { sessionId: session.id, prompt: 'Native queue 2' });
    let queue = [];
    for (let i = 0; i < 150; i++) {
      queue = await invoke('task_queue');
      if (queue.length === 2 && queue.every((t) => t.state === 'completed')) break;
      await new Promise((r) => setTimeout(r, 100));
    }
    const agents = [
      {
        provider: 'mock',
        account_profile_id: 'mock-local',
        model: 'mock-stream-v1',
        permission_profile: 'standard',
        reasoning_effort: null,
        fast_mode: false,
        tools: {},
        role: 'Ведущий',
      },
      {
        provider: 'mock',
        account_profile_id: 'mock-local',
        model: 'mock-stream-v1',
        permission_profile: 'read_only',
        reasoning_effort: null,
        fast_mode: false,
        tools: {},
        role: 'Архитектор',
      },
    ];
    const team = await invoke('create_chat', {
      input: { workspace_id: workspace.id, mode: 'auto', agents },
    });
    const teamLaunches = [];
    for (const prompt of [
      'Unity: предложи интерфейсы радиального меню патронов и ледяного эффекта',
      'Продолжай',
    ]) {
      const run = await invoke('send_message', { sessionId: team.id, prompt, attachments: [] });
      let events = [];
      for (let i = 0; i < 160; i++) {
        events = await invoke('session_events', { sessionId: team.id, before: null });
        if (events.some((e) => e.run_id === run && e.payload.type === 'turn_completed')) break;
        if (events.some((e) => e.run_id === run && e.payload.type === 'provider_error'))
          throw new Error('Auto team failed');
        await new Promise((r) => setTimeout(r, 100));
      }
      if (!events.some((e) => e.run_id === run && e.payload.type === 'turn_completed'))
        throw new Error('Auto team did not finish');
      const launches = events.filter(
        (e) => e.run_id === run && e.payload.type === 'provider_run_started',
      );
      if (launches.some((e) => e.payload.purpose === 'planning'))
        throw new Error('Unexpected paid planner');
      teamLaunches.push(launches.length);
    }
    const configured = await invoke('configure_conversation', {
      sessionId: session.id,
      settings: { compact_tokens: null, claude_compact_percent: null, response_style: 'visual' },
    });
    if (JSON.parse(configured.conversation_settings).response_style !== 'visual')
      throw new Error('Conversation settings did not persist');
    const first = await invoke('send_message', {
      sessionId: session.id,
      prompt: 'Native running composer',
      attachments: [],
    });
    const queued = await invoke('enqueue_message', {
      sessionId: session.id,
      prompt: 'Native follow-up with attachment',
      attachments: [{ name: 'context.txt', mime: 'text/plain', data: btoa('Additional context') }],
    });
    let followup;
    for (let i = 0; i < 160; i++) {
      followup = (await invoke('task_queue')).find((t) => t.id === queued);
      if (['completed', 'failed'].includes(followup?.state)) break;
      await new Promise((r) => setTimeout(r, 100));
    }
    if (followup?.state !== 'completed')
      throw new Error('Attached follow-up failed: ' + JSON.stringify(followup));
    const history = await invoke('session_events', { sessionId: session.id, before: null });
    if (
      !history.some(
        (e) =>
          e.run_id === followup.run_id &&
          e.payload.type === 'user_attachments' &&
          e.payload.files[0].name === 'context.txt',
      )
    )
      throw new Error('Queued attachment missing from run history');
    if (!history.some((e) => e.run_id === first && e.payload.type === 'turn_completed'))
      throw new Error('Initial turn not completed');
    const participantModes = [];
    for (const mode of ['team', 'auto']) {
      const configuredChat = await invoke('create_chat', {
        input: { workspace_id: workspace.id, mode, agents },
      });
      let snapshot = await invoke('snapshot');
      const participant = snapshot.sessions.find(
        (s) => s.parent_session_id === configuredChat.id && s.chat_mode !== 'task',
      );
      await invoke('configure_session', {
        sessionId: participant.id,
        config: { ...agents[1], permission_profile: 'workspace_auto' },
      });
      const longPrompt =
        'Native long input: ' + 'Техническое задание\n'.repeat(2500) + 'END_OF_LONG_TEXT';
      const run = await invoke('send_message', {
        sessionId: configuredChat.id,
        prompt: longPrompt,
        attachments: [],
      });
      let history = [];
      for (let i = 0; i < 160; i++) {
        history = await invoke('session_events', { sessionId: configuredChat.id, before: null });
        if (history.some((e) => e.run_id === run && e.payload.type === 'turn_completed')) break;
        if (history.some((e) => e.run_id === run && e.payload.type === 'provider_error'))
          throw new Error('Configured participant failed');
        await new Promise((r) => setTimeout(r, 100));
      }
      if (!history.some((e) => e.run_id === run && e.payload.type === 'turn_completed'))
        throw new Error('Configured team timed out');
      if (
        history.find((e) => e.run_id === run && e.payload.type === 'turn_started')?.payload
          .prompt !== longPrompt
      )
        throw new Error('Long text was truncated');
      snapshot = await invoke('snapshot');
      const task = snapshot.sessions.find(
        (s) => s.parent_session_id === configuredChat.id && s.chat_mode === 'task',
      );
      if (task?.permission_profile !== 'workspace_auto')
        throw new Error('Participant access was replaced');
      participantModes.push(mode);
    }
    return {
      participantModes,
      conversationStyle: 'visual',
      queuedAttachment: true,
      teamLaunches,
      restored,
      isolated: branch.working_directory !== session.working_directory,
      worktree: !!branch.worktree_id,
      states: queue.map((t) => t.state),
      checkpoints: (await invoke('checkpoints', { sessionId: session.id })).length,
    };
  }, project);
  assert.equal(result.restored, 'before\n');
  assert(result.isolated && result.worktree);
  assert.deepEqual(result.states, ['completed', 'completed']);
  assert.equal(result.checkpoints, 2);
  assert.deepEqual(result.teamLaunches, [2, 1]);
  await page.reload();
  await page.locator('.recent-chat').filter({ hasText: 'Native queue 1' }).first().click();
  await page.getByRole('button', { name: 'Задачи и версии', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Задачи и версии' })).toContainText('Native queue');
  await page.screenshot({
    path: `test-results/native-workflows-${version}.png`,
    animations: 'disabled',
  });
  await page.keyboard.press('Escape');
  const ring = page.getByRole('button', { name: 'Контекст и оформление ответа', exact: true });
  await expect(ring).toHaveCount(1);
  await expect(ring).toBeVisible();
  const ringBounds = await ring.boundingBox();
  assert(ringBounds && ringBounds.width <= 30);
  await ring.click();
  const contextPanel = page.getByRole('dialog', { name: 'Контекст и оформление ответа' });
  await expect(contextPanel).toBeVisible();
  const panelBounds = await contextPanel.boundingBox();
  const viewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight }));
  assert(
    panelBounds &&
      panelBounds.x >= 0 &&
      panelBounds.y >= 0 &&
      panelBounds.x + panelBounds.width <= viewport.width &&
      panelBounds.y + panelBounds.height <= viewport.height,
  );
  await page.keyboard.press('Escape');
  await expect(contextPanel).toHaveCount(0);
  await page.screenshot({
    path: `test-results/native-context-ring-${version}.png`,
    animations: 'disabled',
  });
  console.log(
    JSON.stringify({
      passed: 'packaged queue, worktree, checkpoint diff/restore, UI',
      version,
      claude_model_calls: 0,
      result,
    }),
  );
} finally {
  await browser?.close();
  child.kill();
}
