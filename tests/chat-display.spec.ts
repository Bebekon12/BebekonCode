import { test, expect } from '@playwright/test';

test('tools collapse into readable groups, goals stay highlighted and the answer is intact', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByLabel('Сообщение агенту').fill('Проверь отображение');
  await page.getByLabel('Сообщение агенту').press('Enter');
  await expect(page.locator('.live-status')).toBeVisible();
  await expect(page.locator('.todo-card .todo-in_progress')).toHaveCount(1);
  await expect(page.getByText(/^Ответ завершён/)).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('.live-status')).toHaveCount(0);
  const groups = page.locator('.tool-group');
  await expect(groups).toHaveCount(3);
  await expect(groups.first().locator('.tool-group-head')).toHaveText('1 команда, 1 поиск');
  await expect(groups.nth(1).locator('.tool-group-head')).toHaveText(
    '1 команда (ошибок: 1), 1 файл прочитан',
  );
  await expect(groups.nth(2).locator('.tool-group-head')).toContainText(
    'Создан ChangedEventHandler.ts',
  );
  await expect(groups.nth(2).locator('.diff-added')).toHaveText('+69');
  await expect(page.locator('.tool-list')).toHaveCount(0);
  await groups.nth(1).locator('.tool-group-head').click();
  await expect(groups.nth(1).locator('.tool-row')).toHaveCount(2);
  await expect(groups.nth(1).locator('.tool-row.failed')).toContainText('npm run typecheck');
  await expect(page.locator('.todo-card .todo-completed')).toHaveCount(4);
  await expect(page.locator('.step-message.agent').last()).toContainText(
    'Это ответ локального демо',
  );
  await expect(page.locator('.user-message')).toHaveText(/Проверь отображение/);
  await page.screenshot({ path: 'test-results/chat-display.png' });
});
