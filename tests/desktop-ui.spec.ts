import { test, expect } from '@playwright/test';

test('project, session, streaming, stop and keyboard commands', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/?preview=1');
  await expect(page.getByRole('button', { name: 'Добавить проект', exact: true })).toBeEnabled();
  await page.screenshot({ path: 'test-results/overview.png', fullPage: true });
  await page.getByRole('button', { name: 'Добавить проект', exact: true }).click();
  await page.locator('.new-session-button').click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(
    page.getByLabel('Провайдер', { exact: true }).locator('option[value="anthropic"]'),
  ).not.toHaveAttribute('disabled', '');
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page
    .getByRole('textbox', { name: 'Сообщение агенту' })
    .fill('Design a safe provider boundary');
  await page.getByRole('textbox', { name: 'Сообщение агенту' }).press('Control+Enter');
  await expect(page.getByRole('button', { name: 'Остановить агента' })).toBeVisible();
  await expect(page.getByText('Это ответ локального демо.', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Остановить агента' }).click();
  await expect(page.getByText('Остановлено вами')).toBeVisible();
  await page.getByRole('button', { name: 'Поиск и фильтры истории' }).click();
  await page.getByRole('textbox', { name: 'Поиск по событиям' }).fill('не существующий текст');
  await expect(page.getByText('Подходящих событий нет.')).toBeVisible();
  // Case-insensitive Cyrillic search over the rendered assistant text.
  await page.getByRole('textbox', { name: 'Поиск по событиям' }).fill('ЛОКАЛЬНОГО ДЕМО');
  await expect(page.locator('.stepper > .step')).toHaveCount(1);
  await page.getByRole('textbox', { name: 'Поиск по событиям' }).fill('');
  await page
    .getByRole('toolbar', { name: 'Фильтры событий' })
    .getByRole('button', { name: 'Инструменты', exact: true })
    .click();
  await expect(page.locator('.prose')).toHaveCount(0);
  await page.getByRole('button', { name: 'Все события', exact: true }).click();
  await expect(page.getByText('Остановлено вами')).toBeVisible();
  await page.screenshot({ path: 'test-results/session.png', fullPage: true });
  await page.keyboard.press('Control+k');
  await page.getByRole('textbox', { name: 'Поиск команд' }).fill('настройки');
  await page.getByRole('button', { name: 'Открыть настройки' }).click();
  await expect(page.getByRole('dialog', { name: 'Настройки' })).toBeVisible();
  await page.getByRole('dialog').getByRole('button', { name: 'О программе и обновления' }).click();
  await page.getByRole('button', { name: 'Проверить обновления', exact: true }).click();
  await expect(page.getByText('Стабильных выпусков пока нет')).toBeVisible();
  await page.screenshot({ path: 'test-results/settings.png', fullPage: true });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('multiple sessions retain separate timelines', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Добавить проект', exact: true }).click();
  await page.keyboard.press('Control+n');
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('textbox', { name: 'Сообщение агенту' }).fill('First independent task');
  await page.getByRole('button', { name: 'Отправить сообщение' }).click();
  await page.keyboard.press('Control+n');
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('textbox', { name: 'Сообщение агенту' }).fill('Second independent task');
  await page.getByRole('button', { name: 'Отправить сообщение' }).click();
  await page.getByRole('button', { name: 'Остановить агента' }).click();
  await expect(page.getByText('Остановлено вами')).toBeVisible();
  await page.getByRole('button', { name: /First independent task/ }).click();
  await expect(page.getByText('Ответ завершён')).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole('textbox', { name: 'Сообщение агенту' })).toBeEnabled();
  await expect(page.locator('.prose').filter({ hasText: 'Second independent task' })).toHaveCount(
    0,
  );
});

test('layout remains contained at Windows 150% logical resolution', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto('/?preview=1');
  await expect(page.getByRole('button', { name: 'Добавить проект', exact: true })).toBeEnabled();
  const dimensions = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth,
    viewport: innerWidth,
  }));
  expect(dimensions.width).toBeLessThanOrEqual(dimensions.viewport);
  await page.screenshot({ path: 'test-results/compact.png', fullPage: true });
});

test('project tabs and layout preferences survive a reload', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Добавить проект', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'sample-project' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await page.getByRole('button', { name: 'Закрыть вкладку sample-project' }).click();
  await expect(page.getByRole('tab')).toHaveCount(0);
  // Closing a tab keeps the project in the sidebar.
  await page.getByRole('button', { name: 'sample-project', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'sample-project' })).toBeVisible();

  await page.getByRole('button', { name: 'Скрыть приветствие' }).click();
  await page.getByRole('button', { name: 'Свернуть меню' }).click();
  await expect(page.locator('.sidebar.collapsed')).toBeVisible();
  await page.reload();
  await expect(page.locator('.sidebar.collapsed')).toBeVisible();
  await expect(page.getByRole('region', { name: 'Добро пожаловать' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Развернуть меню' }).click();
  await expect(page.locator('.sidebar.collapsed')).toHaveCount(0);
});

test('codex account sign-in, usage limits and command approval', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Добавить проект', exact: true }).click();
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  await page.getByRole('button', { name: 'Добавить аккаунт ChatGPT' }).click();
  await page.getByRole('textbox', { name: 'Название аккаунта' }).fill('Личный');
  await page.getByRole('button', { name: 'Добавить', exact: true }).click();
  const card = page.getByRole('region', { name: 'Аккаунт Личный' });
  await expect(card.getByText('Вход не выполнен').first()).toBeVisible();
  await card.getByRole('button', { name: 'Continue with ChatGPT' }).click();
  const welcome = page.getByRole('dialog', { name: 'Вы используете свой план ChatGPT' });
  await expect(welcome).toBeVisible();
  await welcome.getByRole('button', { name: 'Понятно' }).click();
  await expect(welcome).toHaveCount(0);
  await expect(
    card.getByRole('button', { name: /Управлять использованием в ChatGPT/ }),
  ).toBeVisible();
  await expect(card.getByText('Plus', { exact: true })).toBeVisible();
  await expect(card.getByRole('meter', { name: /5 часов: использовано 25%/ })).toBeVisible();
  await expect(card.getByRole('meter', { name: /Неделя: использовано 58%/ })).toBeVisible();
  const sandbox = card.getByRole('status', { name: 'Песочница Windows' });
  await expect(sandbox).toContainText('Требует внимания');
  await sandbox.getByRole('button', { name: 'Настроить песочницу Windows' }).click();
  await expect(sandbox).toContainText('Готова');
  await expect(sandbox.getByRole('button')).toHaveCount(0);
  await page.screenshot({ path: 'test-results/accounts.png', fullPage: true });
  await page.keyboard.press('Escape');

  await page.keyboard.press('Control+n');
  await page.getByLabel('Провайдер', { exact: true }).selectOption('openai');
  await expect(page.getByLabel('Аккаунт', { exact: true })).toHaveValue(/.+/);
  await expect(page.getByLabel('Модель', { exact: true })).toHaveValue('preview-model');
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const composer = page.getByRole('textbox', { name: 'Сообщение агенту' });
  await composer.fill('Запусти тесты');
  await composer.press('Control+Enter');
  const approval = page.getByRole('alertdialog', { name: 'Codex хочет выполнить команду' });
  await expect(approval).toBeVisible();
  await expect(approval.getByText('npm test')).toBeVisible();
  await expect(approval).toContainText('остальные команды и других участников');
  await expect(approval.getByRole('button', { name: 'Разрешить в этом контексте' })).toHaveCount(0);
  await page.screenshot({ path: 'test-results/approval.png', fullPage: true });
  await approval.getByRole('button', { name: 'Разрешить один раз' }).click();
  await expect(page.getByText('Разрешено один раз')).toBeVisible();
  await expect(page.getByText(/^Ответ завершён/)).toBeVisible({ timeout: 15_000 });
});
