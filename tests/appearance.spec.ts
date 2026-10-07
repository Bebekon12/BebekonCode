import { test, expect } from '@playwright/test';

test('readable chat uses a bundled Cyrillic font and appearance survives reload', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await expect(page.locator('.context-panel')).toHaveCount(0);
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат' }).click();
  const input = page.getByRole('textbox', { name: 'Сообщение агенту' });
  await expect(input).toHaveCSS('font-size', '17px');
  const font = await page.evaluate(async () => {
    await document.fonts.load('17px Inter', 'Русский текст');
    return [...document.fonts].some((font) => font.family === 'Inter' && font.status === 'loaded');
  });
  expect(font).toBe(true);
  await input.fill('Объясни, как проверить изменения в проекте');
  await input.press('Control+Enter');
  await expect(page.getByText(/^Ответ завершён/)).toBeVisible({ timeout: 15_000 });
  await page.screenshot({ path: 'test-results/readable-dark-chat.png' });

  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByLabel('Тема оформления').selectOption('light');
  await page.getByLabel('Размер текста').selectOption('large');
  await page.keyboard.press('Escape');
  await expect(input).toHaveCSS('font-size', '19px');
  await expect(page.locator('.prose').first()).toHaveCSS('font-size', '19px');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.screenshot({ path: 'test-results/readable-light-chat.png' });
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveAttribute('data-text-size', 'large');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await expect(page.getByLabel('Размер текста')).toHaveValue('large');
  await expect(page.getByLabel('Тема оформления')).toHaveValue('light');
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Показать или скрыть контекст' }).click();
  await expect(page.locator('.context-panel')).toBeVisible();
  await page.reload();
  await expect(page.locator('.context-panel')).toBeVisible();
});

test('large text keeps chat actions inside a small Windows window', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByLabel('Размер текста').selectOption('large');
  await page.keyboard.press('Escape');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат' }).click();
  await page.getByRole('textbox', { name: 'Сообщение агенту' }).fill('Проверь удобство интерфейса');
  const input = await page.locator('.composer').boundingBox();
  const send = await page.getByRole('button', { name: 'Отправить сообщение' }).boundingBox();
  expect(input).not.toBeNull();
  expect(send).not.toBeNull();
  expect(send!.y + send!.height).toBeLessThanOrEqual(640);
  expect(send!.x + send!.width).toBeLessThanOrEqual(960);
  const layout = await page.evaluate(() => {
    const composer = document.querySelector('.composer')!;
    return {
      page: document.documentElement.scrollWidth <= innerWidth,
      composer: composer.scrollWidth <= composer.clientWidth,
    };
  });
  expect(layout).toEqual({ page: true, composer: true });
  const heading = await page.getByRole('heading', { name: 'Над чем поработаем?' }).boundingBox();
  const timeline = await page.locator('.chat-timeline').boundingBox();
  expect(heading!.y).toBeGreaterThanOrEqual(timeline!.y);
  expect(heading!.y + heading!.height).toBeLessThanOrEqual(timeline!.y + timeline!.height);
  await page.screenshot({ path: 'test-results/readable-compact-chat.png' });
});
