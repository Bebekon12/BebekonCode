import { test, expect } from '@playwright/test';

test('attachments can be selected, pasted, dropped, removed and recorded in history', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByLabel('Выбрать вложения').setInputFiles([
    { name: 'заметки.txt', mimeType: 'text/plain', buffer: Buffer.from('Текст задачи') },
    { name: 'video.mp4', mimeType: 'video/mp4', buffer: Buffer.from([0, 1, 2]) },
  ]);
  await expect(page.locator('.attachment-card')).toHaveCount(2);
  await expect(page.locator('.attachment-list')).toContainText('без анализа кадров');
  await page.getByRole('button', { name: 'Убрать video.mp4', exact: true }).click();
  const input = page.getByLabel('Сообщение агенту');
  await input.evaluate((element) => {
    const transfer = new DataTransfer();
    const bytes = Uint8Array.from(
      atob(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a3ioAAAAASUVORK5CYII=',
      ),
      (c) => c.charCodeAt(0),
    );
    transfer.items.add(new File([bytes], 'фото.png', { type: 'image/png' }));
    element.dispatchEvent(new ClipboardEvent('paste', { clipboardData: transfer, bubbles: true }));
  });
  await expect(page.locator('.attachment-card img')).toHaveCount(1);
  await page.getByRole('button', { name: 'Посмотреть фото.png', exact: true }).click();
  const imageDialog = page.getByRole('dialog', { name: 'фото.png', exact: true });
  await expect(imageDialog.locator('img')).toBeVisible();
  await imageDialog.getByRole('button', { name: 'Исходный размер', exact: true }).click();
  await expect(imageDialog.locator('.image-preview')).toHaveClass(/actual-size/);
  await page.keyboard.press('Escape');
  await expect(imageDialog).toHaveCount(0);
  await page.locator('.composer').evaluate((element) => {
    const transfer = new DataTransfer();
    transfer.items.add(new File(['Документ'], 'план.md', { type: 'text/markdown' }));
    element.dispatchEvent(new DragEvent('drop', { dataTransfer: transfer, bubbles: true }));
  });
  await expect(page.locator('.attachment-card')).toHaveCount(3);
  await input.fill('Прочитай вложения');
  await page.getByRole('button', { name: 'Отправить сообщение', exact: true }).click();
  await expect(page.locator('.sent-attachments')).toContainText('заметки.txt');
  await expect(page.locator('.sent-attachments')).toContainText('фото.png');
  await expect(page.locator('.sent-attachments')).not.toContainText('video.mp4');
  await expect(page.locator('.attachment-card')).toHaveCount(0);
  await page.getByRole('button', { name: 'Посмотреть фото.png', exact: true }).click();
  await expect(imageDialog.locator('img')).toBeVisible();
  await page.keyboard.press('Escape');
});

test('public catalog separates Codex skill setup from unavailable plugin installation', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Плагины', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Плагины и инструменты' });
  await expect(dialog.locator('.plugin-store-card')).toHaveCount(2);
  await expect(dialog).toContainText('Навыки Codex можно установить через официальный установщик');
  await dialog.getByRole('button', { name: 'Установка Codex', exact: true }).click();
  await expect(
    dialog.getByRole('button', { name: 'Посмотреть доступные навыки', exact: true }),
  ).toBeDisabled();
  await dialog.getByLabel('Название устанавливаемого навыка').fill('../unsafe');
  await expect(dialog).toContainText('Используйте латинские');
  await dialog.getByRole('button', { name: 'Каталог', exact: true }).click();
  await dialog.getByLabel('Провайдер каталога').selectOption('anthropic');
  await expect(dialog.locator('.plugin-store-card')).toHaveCount(1);
  await dialog.getByLabel('Поиск в каталоге').fill('несуществующий');
  await expect(dialog).toContainText('Плагины не найдены');
  await dialog.getByLabel('Поиск в каталоге').fill('review');
  await expect(dialog.locator('.plugin-store-card')).toContainText('code-review');
  await page.screenshot({ path: 'test-results/public-catalog.png' });
});

test('settings expose limits and Codex setup creates an account-bound unsent draft', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Настройки' });
  await settings.getByLabel('Найти раздел настроек').fill('лимиты');
  await settings.getByRole('button', { name: 'Лимиты', exact: true }).click();
  await expect(settings).toContainText('Подключите аккаунт');
  await settings.getByRole('button', { name: 'Перейти к аккаунтам', exact: true }).click();
  await settings.getByLabel('Найти раздел настроек').fill('');
  await settings.getByRole('button', { name: /ChatGPT Готов/ }).click();
  await settings.getByRole('button', { name: 'Добавить аккаунт ChatGPT', exact: true }).click();
  await settings.getByLabel('Название аккаунта').fill('Установки GPT');
  await settings.getByRole('button', { name: 'Добавить', exact: true }).click();
  await settings.getByRole('button', { name: 'Continue with ChatGPT', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })
    .getByRole('button', { name: 'Понятно' })
    .click();
  await settings.getByRole('button', { name: 'Лимиты', exact: true }).click();
  await expect(settings.locator('.settings-limit-card')).toContainText('Установки GPT');
  await expect(settings.getByRole('progressbar', { name: /5 часов/ })).toHaveAttribute(
    'value',
    '75',
  );
  await page.screenshot({ path: 'test-results/settings-limits-redesign.png' });
  await page.setViewportSize({ width: 960, height: 640 });
  await settings.getByRole('button', { name: 'Основные', exact: true }).click();
  await settings.getByLabel('Размер текста').selectOption('large');
  expect(await settings.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
    true,
  );
  await page.screenshot({ path: 'test-results/settings-redesign-compact.png' });
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Плагины', exact: true }).click();
  const plugins = page.getByRole('dialog', { name: 'Плагины и инструменты' });
  await plugins.getByRole('button', { name: 'Установка Codex', exact: true }).click();
  await plugins.getByLabel('Название устанавливаемого навыка').fill('pdf');
  await plugins.getByRole('button', { name: 'Подготовить установку', exact: true }).click();
  await expect(plugins).toHaveCount(0);
  await expect(page.getByLabel('Сообщение агенту')).toHaveValue(
    /\$skill-installer Установи навык pdf/,
  );
  await expect(page.getByRole('button', { name: 'Остановить агента', exact: true })).toHaveCount(0);
  await expect(page.locator('.step-message')).toHaveCount(0);
});

test('access descriptions and effort slider fit a compact window', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByLabel('Доступ в чате').click();
  const menu = page.getByLabel('Уровни доступа');
  await expect(menu).toContainText('Авто в проекте');
  const box = (await menu.boundingBox())!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(960);
  await page.screenshot({ path: 'test-results/access-picker.png' });
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
});
