import { test, expect } from '@playwright/test';

test('three participant controls remain visible in a compact window with large text', async ({
  page,
}) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByLabel('Размер текста').selectOption('large');
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Создать команду агентов', exact: true }).click();
  await page.getByRole('button', { name: 'Добавить участника', exact: true }).click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const participants = page.locator('.composer-agent');
  await expect(participants).toHaveCount(3);
  for (const participant of await participants.all()) {
    const box = (await participant.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(960);
    expect(box.y + box.height).toBeLessThanOrEqual(640);
  }
  const send = (await page.getByRole('button', { name: 'Отправить сообщение' }).boundingBox())!;
  expect(send.y + send.height).toBeLessThanOrEqual(640);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await participants.filter({ hasText: 'Исследователь' }).click();
  await expect(
    page.getByRole('dialog', { name: 'Настройки агента' }).getByLabel('Роль', { exact: true }),
  ).toHaveValue('Исследователь');
  await page.keyboard.press('Escape');
  await page.screenshot({ path: 'test-results/compact-team.png' });
});
