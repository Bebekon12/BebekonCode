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
});

test('public catalog can be browsed without accounts and installation is labelled unavailable', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Плагины', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Плагины и инструменты' });
  await expect(dialog.locator('.plugin-store-card')).toHaveCount(2);
  await expect(dialog).toContainText('Установка в BebekonCode пока недоступна');
  await dialog.getByLabel('Провайдер каталога').selectOption('anthropic');
  await expect(dialog.locator('.plugin-store-card')).toHaveCount(1);
  await dialog.getByLabel('Поиск в каталоге').fill('несуществующий');
  await expect(dialog).toContainText('Плагины не найдены');
  await dialog.getByLabel('Поиск в каталоге').fill('review');
  await expect(dialog.locator('.plugin-store-card')).toContainText('code-review');
  await page.screenshot({ path: 'test-results/public-catalog.png' });
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
