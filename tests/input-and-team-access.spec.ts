import { test, expect } from '@playwright/test';

test('large pasted Unicode text keeps its end and is sent intact', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const text = 'Начало\n' + 'Большое техническое задание 🧊\n'.repeat(1800) + '\nКОНЕЦ_ТЕКСТА';
  const input = page.getByLabel('Сообщение агенту');
  await expect(input).not.toHaveAttribute('maxlength');
  await input.focus();
  await page.keyboard.insertText(text);
  await expect(input).toHaveValue(text);
  await page.getByRole('button', { name: 'Отправить сообщение', exact: true }).click();
  const sent = await page.evaluate(async () => {
    const modulePath = '/src/preview.ts';
    const { preview } = await import(modulePath);
    const snapshot = await preview.snapshot();
    const session = snapshot.sessions.find(
      (s: { parent_session_id: string | null }) => !s.parent_session_id,
    );
    const events = await preview.events(session.id);
    return events.find((e: { payload: { type: string } }) => e.payload.type === 'turn_started')
      .payload.prompt;
  });
  expect(sent).toBe(text);
});

for (const mode of ['team', 'auto'])
  test(`second ${mode} participant keeps access plugins MCP and skills`, async ({ page }) => {
    await page.goto('/?preview=1');
    await page.getByRole('button', { name: 'Создать команду агентов', exact: true }).click();
    const wizard = page.getByRole('dialog', { name: 'Новый чат' }),
      member = wizard.locator('.chat-agent-card').nth(1);
    if (mode === 'auto') await wizard.getByRole('button', { name: /Автокоманда/ }).click();
    await member.locator('.agent-card-toggle').click();
    await member.getByLabel('Роль', { exact: true }).fill('Интерфейс');
    await member.getByLabel('Доступ', { exact: true }).selectOption('workspace_auto');
    await member.getByRole('button', { name: 'Плагины, MCP и навыки' }).click();
    for (const name of ['Пример плагина', 'preview-mcp', 'Пример навыка']) {
      const box = member.getByRole('checkbox', { name: new RegExp(name) });
      await expect(box).toBeEnabled();
      await box.check();
    }
    await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
    await page.locator('.team-panel > summary').click();
    await page
      .locator('.team-panel')
      .getByRole('button', { name: 'Настроить Интерфейс', exact: true })
      .click();
    const settings = page.getByRole('dialog', { name: 'Настройки агента' });
    await expect(settings.getByLabel('Доступ', { exact: true })).toHaveValue('workspace_auto');
    await settings.getByRole('button', { name: 'Плагины, MCP и навыки' }).click();
    for (const name of ['Пример плагина', 'preview-mcp', 'Пример навыка'])
      await expect(settings.getByRole('checkbox', { name: new RegExp(name) })).toBeChecked();
    await settings.getByLabel('Доступ', { exact: true }).selectOption('standard');
    await settings.getByRole('button', { name: 'Сохранить', exact: true }).click();
    await page
      .locator('.team-panel')
      .getByRole('button', { name: 'Настроить Интерфейс', exact: true })
      .click();
    await expect(settings.getByLabel('Доступ', { exact: true })).toHaveValue('standard');
    await page.screenshot({ path: `test-results/${mode}-participant-access.png` });
  });
