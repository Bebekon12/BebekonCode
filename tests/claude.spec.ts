import { test, expect } from '@playwright/test';

test('Claude account and mixed team expose models, effort and honest tool limitations', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  const accounts = page.getByRole('dialog');
  await accounts.getByRole('button', { name: 'Добавить аккаунт Claude', exact: true }).click();
  await accounts.getByLabel('Название аккаунта').fill('Мой Claude');
  await accounts.getByRole('button', { name: 'Добавить', exact: true }).click();
  await accounts.getByRole('button', { name: 'Войти через Claude Code', exact: true }).click();
  await expect(accounts.getByText('preview@example.com', { exact: true })).toBeVisible();
  await expect(page.getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })).toHaveCount(
    0,
  );
  await expect(accounts.locator('.usage-bar')).toHaveCount(0);
  await accounts.getByRole('button', { name: 'Плагины и MCP', exact: true }).click();
  await expect(
    accounts.getByText('Плагины, MCP и навыки Claude пока недоступны в Windows.', { exact: true }),
  ).toBeVisible();
  await accounts.getByRole('button', { name: 'Добавить аккаунт ChatGPT', exact: true }).click();
  await accounts.getByLabel('Название аккаунта').fill('Мой GPT');
  await accounts.getByRole('button', { name: 'Добавить', exact: true }).click();
  await accounts.getByRole('button', { name: 'Continue with ChatGPT', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })
    .getByRole('button', { name: 'Понятно' })
    .click();
  await page.keyboard.press('Escape');
  await page.locator('.new-session-button').click();
  const wizard = page.getByRole('dialog', { name: 'Новый чат' });
  await wizard.getByRole('button', { name: /Команда агентов/ }).click();
  const root = wizard.locator('.chat-agent-card').nth(0);
  const worker = wizard.locator('.chat-agent-card').nth(1);
  await root.getByLabel('Провайдер', { exact: true }).selectOption('anthropic');
  await expect(root.getByLabel('Модель', { exact: true })).toHaveValue('sonnet');
  await root.getByLabel('Модель', { exact: true }).selectOption('opus');
  await root.getByLabel('Уровень рассуждения', { exact: true }).selectOption('high');
  await worker.getByLabel('Провайдер', { exact: true }).selectOption('openai');
  await expect(worker.getByLabel('Модель', { exact: true })).toHaveValue('preview-model');
  await expect(worker.getByLabel('Доступ', { exact: true })).toHaveValue('read_only');
  await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(page.getByLabel('Модель в чате')).toHaveValue('opus');
  await expect(page.getByLabel('Рассуждение в чате')).toHaveValue('high');
  const chats = await page.locator('.recent-chat').count();
  await page.getByRole('button', { name: 'Перейти', exact: true }).click();
  const handoff = page.getByRole('dialog', { name: 'Перейти с контекстом' });
  await handoff.getByLabel('Провайдер', { exact: true }).selectOption('openai');
  await handoff.getByRole('button', { name: 'Перейти с контекстом', exact: true }).click();
  await expect(page.getByLabel('Модель в чате')).toHaveValue('preview-model');
  await expect(page.locator('.recent-chat')).toHaveCount(chats);
});
