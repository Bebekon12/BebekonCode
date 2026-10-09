import { test, expect } from '@playwright/test';

test('plugins install and remove in the selected account; desktop use stays unavailable', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  for (const name of ['Плагины GPT', 'Другой GPT']) {
    await page.getByRole('button', { name: 'Добавить аккаунт ChatGPT' }).click();
    await page.getByLabel('Название аккаунта').fill(name);
    await page.getByRole('button', { name: 'Добавить', exact: true }).click();
  }
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Плагины', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Плагины и инструменты' });
  const account = dialog.getByLabel('Аккаунт инструментов');
  const options = await account.locator('option').allTextContents();
  const first = await account
    .locator('option')
    .filter({ hasText: 'Плагины GPT' })
    .getAttribute('value');
  const second = await account
    .locator('option')
    .filter({ hasText: 'Другой GPT' })
    .getAttribute('value');
  expect(options.length).toBe(3);
  await account.selectOption(first!);
  await expect(
    dialog.getByRole('button', { name: 'Установить Computer Use', exact: true }),
  ).toBeDisabled();
  await dialog.getByRole('button', { name: 'Unity', exact: true }).click();
  await expect(dialog.getByRole('status')).toContainText('Каталог Unity подключён');
  await dialog.getByRole('button', { name: 'Установить unity', exact: true }).click();
  await dialog.getByRole('button', { name: 'Установить плагин', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Удалить плагин', exact: true })).toBeEnabled();
  await page.screenshot({ path: 'test-results/plugins-manager-dark.png', animations: 'disabled' });
  await account.selectOption(second!);
  await expect(dialog.getByRole('button', { name: 'Подробнее: unity' })).toHaveCount(0);
  await account.selectOption(first!);
  await dialog.getByRole('button', { name: 'Подробнее: unity' }).click();
  await dialog.getByRole('button', { name: 'Удалить плагин', exact: true }).click();
  await expect(
    dialog.getByRole('button', { name: 'Установить плагин', exact: true }),
  ).toBeEnabled();
  await page.setViewportSize({ width: 960, height: 640 });
  expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  await page.screenshot({
    path: 'test-results/plugins-manager-compact.png',
    animations: 'disabled',
  });
});

test('composer help is on demand and settings remain usable with keyboard and small windows', async ({
  page,
}) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(page.locator('.composer-help-panel')).toBeHidden();
  await page.getByLabel('О вложениях и контексте').click();
  await expect(page.locator('.composer-help-panel')).toContainText('DOCX');
  await page.getByLabel('О вложениях и контексте').click();
  await expect(page.locator('.composer-help-panel')).toBeHidden();
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await expect(page.getByLabel('Проверять обновления при запуске')).toHaveRole('switch');
  await page.getByLabel('Тема оформления').selectOption('light');
  await page.screenshot({
    path: 'test-results/settings-light-redesign.png',
    animations: 'disabled',
  });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
