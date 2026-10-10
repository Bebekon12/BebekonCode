import { test, expect } from '@playwright/test';

test('Claude account and mixed team expose models, effort and honest tool limitations', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  const accounts = page.getByRole('dialog');
  await accounts.getByRole('button', { name: /Claude Готов/ }).click();
  await accounts.getByRole('button', { name: 'Добавить аккаунт Claude', exact: true }).click();
  await accounts.getByLabel('Название аккаунта').fill('Мой Claude');
  await accounts.getByRole('button', { name: 'Добавить', exact: true }).click();
  await accounts.getByRole('button', { name: 'Войти через Claude Code', exact: true }).click();
  await expect(accounts.getByText('preview@example.com', { exact: true })).toBeVisible();
  await expect(page.getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })).toHaveCount(
    0,
  );
  await expect(accounts.locator('.usage-bar')).toHaveCount(0);
  await accounts.getByRole('button', { name: /ChatGPT Готов/ }).click();
  await accounts.getByRole('button', { name: 'Добавить аккаунт ChatGPT', exact: true }).click();
  await accounts.getByLabel('Название аккаунта').fill('Мой GPT');
  await accounts.getByRole('button', { name: 'Добавить', exact: true }).click();
  await accounts.getByRole('button', { name: 'Continue with ChatGPT', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })
    .getByRole('button', { name: 'Понятно' })
    .click();
  await page.keyboard.press('Escape');
  const limits = page.getByRole('region', { name: 'Лимиты подписок' });
  await expect(limits.locator('.limit-account')).toHaveCount(2);
  await expect(limits).toContainText('Мой Claude');
  await expect(limits).toContainText('Мой GPT');
  await expect(limits).toContainText('Лимиты недоступны');
  await expect(limits).toContainText('Claude SDK · экспериментальный API');
  await expect(limits.getByRole('progressbar', { name: /Мой GPT: 5 часов/ })).toHaveAttribute(
    'value',
    '75',
  );
  await limits.getByRole('button', { name: 'Обновить все лимиты', exact: true }).click();
  await page.locator('.new-session-button').click();
  const wizard = page.getByRole('dialog', { name: 'Новый чат' });
  await wizard.getByRole('button', { name: /Команда агентов/ }).click();
  const root = wizard.locator('.chat-agent-card').nth(0);
  const worker = wizard.locator('.chat-agent-card').nth(1);
  await root.getByLabel('Провайдер', { exact: true }).selectOption('anthropic');
  await expect(
    root.getByLabel('Доступ', { exact: true }).locator('option[value="full_access"]'),
  ).toBeEnabled();
  await expect(root.getByLabel('Модель', { exact: true })).toHaveValue('sonnet');
  await root.getByLabel('Модель', { exact: true }).selectOption('opus');
  await expect(root.getByRole('button', { name: 'Скоростной режим', exact: true })).toBeDisabled();
  await root.getByLabel('Модель', { exact: true }).selectOption('claude-opus-5-5');
  const fast = root.getByRole('button', { name: 'Скоростной режим', exact: true });
  await fast.click();
  const fastWarning = page.getByRole('dialog', { name: 'Включить скоростной режим' });
  await expect(fastWarning).toContainText('дополнительных usage credits');
  await expect(fastWarning).toContainText('лимит подписки ещё не исчерпан');
  await fastWarning.getByRole('button', { name: 'Включить Fast', exact: true }).click();
  await expect(fast).toHaveAttribute('aria-pressed', 'true');
  await root.getByLabel('Модель', { exact: true }).selectOption('opus');
  await expect(fast).toBeDisabled();
  await expect(fast).toHaveAttribute('aria-pressed', 'false');
  await root.getByLabel('Уровень обдумывания', { exact: true }).fill('3');
  await worker.locator('.agent-card-toggle').click();
  await worker.getByLabel('Провайдер', { exact: true }).selectOption('openai');
  await expect(worker.getByLabel('Модель', { exact: true })).toHaveValue('preview-model');
  // Default team members keep the provider's standard access; full access is never preselected.
  await expect(worker.getByLabel('Доступ', { exact: true })).toHaveValue('standard');
  await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByLabel('Доступ в чате').click();
  await expect(page.getByRole('button', { name: /^Полный доступ Без песочницы/ })).toBeEnabled();
  await page.keyboard.press('Escape');
  await expect(page.locator('.composer-agent')).toHaveCount(2);
  await expect(page.locator('.composer-team')).toContainText('preview-model');
  await page.locator('.composer-agent').first().click();
  const agentSettings = page.getByRole('dialog', { name: 'Настройки агента' });
  await expect(agentSettings.getByLabel('Модель', { exact: true })).toHaveValue('opus');
  await expect(
    agentSettings.getByRole('slider', { name: 'Уровень обдумывания', exact: true }),
  ).toHaveValue('3');
  await agentSettings.getByLabel('Модель', { exact: true }).selectOption('claude-opus-4-6');
  await expect(
    agentSettings.getByRole('slider', { name: 'Уровень обдумывания', exact: true }),
  ).toHaveAttribute('max', '4');
  await agentSettings.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await expect(page.locator('.composer-agent').first()).toContainText('claude-opus-4-6');
  await page.keyboard.press('Escape');
  const access = page.getByRole('button', { name: 'Доступ в чате', exact: true });
  await expect(access).toContainText('По правилам провайдера');
  await access.click();
  await page.getByRole('button', { name: /^Полный доступ Без песочницы/ }).click();
  const warning = page.getByRole('alertdialog', { name: 'Полный доступ', exact: true });
  await expect(warning).toContainText('Учётные данные запрещены');
  await warning.getByRole('button', { name: 'Отмена', exact: true }).click();
  await page.keyboard.press('Escape');
  await expect(access).toContainText('По правилам провайдера');
  await access.click();
  await page.getByRole('button', { name: /^Полный доступ Без песочницы/ }).click();
  await warning.getByRole('button', { name: 'Включить полный доступ', exact: true }).click();
  await expect(access).toContainText('Полный доступ');
  await expect(access).toHaveClass(/access-danger/);
  await access.click();
  await page.getByRole('button', { name: /^По правилам провайдера Claude спрашивает/ }).click();
  await expect(access).toContainText('По правилам провайдера');
  const chats = await page.locator('.recent-chat').count();
  await page.getByRole('button', { name: 'Перейти', exact: true }).click();
  const handoff = page.getByRole('dialog', { name: 'Перейти с контекстом' });
  await expect(handoff).toContainText('прежняя команда автоматически не запускается');
  await handoff.getByLabel('Доступ', { exact: true }).selectOption('full_access');
  await expect(handoff.getByRole('alert')).toContainText('Учётные данные запрещены');
  await handoff.getByLabel('Провайдер', { exact: true }).selectOption('openai');
  await expect(handoff.getByLabel('Доступ', { exact: true })).toHaveValue('standard');
  await handoff.getByRole('button', { name: 'Перейти с контекстом', exact: true }).click();
  await expect(page.locator('.composer-agent')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Модель в чате', exact: true })).toContainText(
    'Модель предпросмотра',
  );
  await expect(page.locator('.team-panel')).toHaveCount(0);
  await expect(page.locator('.recent-chat')).toHaveCount(chats);
});
