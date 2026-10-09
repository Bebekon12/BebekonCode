import { test, expect } from '@playwright/test';

test('full access requires confirmation and can be returned to a restricted mode', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const access = page.getByLabel('Доступ в чате');
  await expect(access).toContainText('По правилам провайдера');
  await access.click();
  await page.getByRole('button', { name: /Полный доступ.*Без песочницы/ }).click();
  const confirmation = page.getByRole('alertdialog', { name: 'Полный доступ', exact: true });
  await expect(confirmation).toContainText('может повредить данные вне проекта');
  await expect(access).toContainText('По правилам провайдера');
  await confirmation.getByRole('button', { name: 'Отмена', exact: true }).click();
  await page.keyboard.press('Escape');
  await expect(access).toContainText('По правилам провайдера');
  await access.click();
  await page.getByRole('button', { name: /Полный доступ.*Без песочницы/ }).click();
  await confirmation.getByRole('button', { name: 'Включить полный доступ', exact: true }).click();
  await expect(access).toContainText('Полный доступ');
  await expect(access).toHaveClass(/access-danger/);
  await access.click();
  await page.getByRole('button', { name: /Только чтение.*Изучать/ }).click();
  await expect(access).toContainText('Только чтение');
  await expect(access).not.toHaveClass(/access-danger/);
});

test('create an ordinary chat without a project and change its access and tools', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  const wizard = page.getByRole('dialog', { name: 'Новый чат' });
  await expect(wizard.getByLabel('Проект', { exact: true })).toHaveValue('');
  await expect(wizard.getByRole('button', { name: /Обычный чат/ })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(page.locator('.chat-title')).toContainText('Без проекта');
  await expect(page.getByRole('tab')).toHaveCount(0);
  await page.getByLabel('Доступ в чате').click();
  await page.getByRole('button', { name: /Только чтение.*Изучать/ }).click();
  await expect(page.getByLabel('Доступ в чате')).toContainText('Только чтение');
  await page
    .locator('.chat-controls')
    .getByRole('button', { name: 'Инструменты', exact: true })
    .click();
  const settings = page.getByRole('dialog', { name: 'Настройки агента' });
  await settings.getByRole('button', { name: /Плагины, MCP и навыки/ }).click();
  await settings.getByLabel('Пример плагина').uncheck();
  await settings.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await page
    .locator('.chat-controls')
    .getByRole('button', { name: 'Инструменты', exact: true })
    .click();
  await page.getByRole('button', { name: /Плагины, MCP и навыки/ }).click();
  await expect(page.getByLabel('Пример плагина')).not.toBeChecked();
  await page.keyboard.press('Escape');
  await page.screenshot({ path: 'test-results/chat-without-project.png' });
});

test('team creation exposes independent participant settings and auto mode', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  const wizard = page.getByRole('dialog', { name: 'Новый чат' });
  await wizard.getByRole('button', { name: /Команда агентов/ }).click();
  await expect(wizard.locator('.chat-agent-card')).toHaveCount(2);
  await wizard.getByRole('button', { name: 'Добавить участника' }).click();
  await expect(wizard.locator('.chat-agent-card')).toHaveCount(3);
  await expect(wizard.getByLabel('Доступ', { exact: true }).nth(1)).toBeEnabled();
  await wizard.locator('.agent-card-toggle').nth(1).click();
  await wizard.getByLabel('Доступ', { exact: true }).nth(1).selectOption('workspace_auto');
  await wizard.getByLabel('Роль', { exact: true }).nth(1).fill('Архитектор');
  await page.screenshot({ path: 'test-results/team-creation.png' });
  await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(page.locator('.chat-title')).toContainText('Команда агентов');
  await page.locator('.team-panel > summary').click();
  await page.getByRole('button', { name: 'Настроить Архитектор', exact: true }).click();
  await expect(
    page.getByRole('dialog', { name: 'Настройки агента' }).getByLabel('Роль', { exact: true }),
  ).toHaveValue('Архитектор');
  await expect(
    page.getByRole('dialog', { name: 'Настройки агента' }).getByLabel('Доступ', { exact: true }),
  ).toHaveValue('workspace_auto');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+n');
  await page.getByRole('button', { name: /Авторазбиение ИИ/ }).click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(page.locator('.chat-title')).toContainText('Авторазбиение');
});

test('reasoning and handoff remain in the same chat', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  await page.getByRole('button', { name: 'Добавить аккаунт ChatGPT' }).click();
  await page.getByLabel('Название аккаунта').fill('Для чатов');
  await page.getByRole('button', { name: 'Добавить', exact: true }).click();
  await page.getByRole('button', { name: 'Continue with ChatGPT' }).click();
  await page
    .getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })
    .getByRole('button', { name: 'Понятно' })
    .click();
  await page.keyboard.press('Escape');
  await page.locator('.new-session-button').click();
  await expect(page.getByLabel('Провайдер', { exact: true })).toHaveValue('openai');
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('button', { name: 'Модель в чате', exact: true }).click();
  await page.getByRole('slider', { name: 'Уровень обдумывания', exact: true }).fill('3');
  await expect(page.getByRole('slider', { name: 'Уровень обдумывания', exact: true })).toHaveValue(
    '3',
  );
  await page.screenshot({ path: 'test-results/effort-slider.png' });
  await page.getByRole('button', { name: 'Сбросить уровень обдумывания' }).click();
  await expect(page.getByRole('slider', { name: 'Уровень обдумывания', exact: true })).toHaveValue(
    '0',
  );
  await page.getByRole('button', { name: 'Готово', exact: true }).click();
  const chats = await page.locator('.recent-chat').count();
  await page.getByRole('button', { name: 'Перейти', exact: true }).click();
  const transition = page.getByRole('dialog', { name: 'Перейти с контекстом' });
  await transition.getByLabel('Провайдер', { exact: true }).selectOption('mock');
  await transition.getByRole('button', { name: 'Перейти с контекстом', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Модель в чате', exact: true })).toContainText(
    'Локальное демо',
  );
  await expect(page.locator('.recent-chat')).toHaveCount(chats);
});
