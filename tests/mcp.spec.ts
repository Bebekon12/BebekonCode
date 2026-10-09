import { test, expect } from '@playwright/test';

test('local MCP settings persist per account and @ selects a server for the chat', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  const settings = page.getByRole('dialog');
  await settings.getByRole('button', { name: /Claude Готов/ }).click();
  await settings.getByRole('button', { name: 'Добавить аккаунт Claude', exact: true }).click();
  await settings.getByLabel('Название аккаунта').fill('MCP test');
  await settings.getByRole('button', { name: 'Добавить', exact: true }).click();
  await settings.getByRole('button', { name: 'Войти через Claude Code', exact: true }).click();
  await expect(settings.getByText('preview@example.com', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await settings.getByRole('button', { name: 'MCP', exact: true }).click();
  await settings.getByLabel('Имя MCP', { exact: true }).fill('unity-test');
  await settings.getByLabel('Программа MCP', { exact: true }).fill('C:\\Tools\\unity.exe');
  await settings
    .getByLabel('Аргументы MCP', { exact: true })
    .fill('mcp\n--project-path\nE:/Unity/Test');
  await settings.getByRole('button', { name: 'Добавить MCP', exact: true }).click();
  await expect(
    settings.getByRole('button', { name: 'Удалить unity-test', exact: true }),
  ).toBeVisible();
  await page.keyboard.press('Escape');
  await page.locator('.new-session-button').click();
  const wizard = page.getByRole('dialog', { name: 'Новый чат' });
  await wizard.getByLabel('Провайдер', { exact: true }).selectOption('anthropic');
  await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Сообщение агенту' });
  await input.fill('Проверь @uni');
  await page.getByRole('button', { name: '@unity-test', exact: true }).click();
  await expect(input).toHaveValue('Проверь @unity-test ');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  const reopened = page.getByRole('dialog');
  await reopened.getByRole('button', { name: 'MCP', exact: true }).click();
  await expect(
    reopened.getByRole('button', { name: 'Удалить unity-test', exact: true }),
  ).toBeVisible();
  await reopened.getByRole('button', { name: 'Удалить unity-test', exact: true }).click();
  await expect(
    reopened.getByRole('button', { name: 'Удалить unity-test', exact: true }),
  ).toHaveCount(0);
});
