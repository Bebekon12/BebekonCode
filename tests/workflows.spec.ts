import { test, expect } from '@playwright/test';

test('queue management and branching are visible without invoking a provider', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('button', { name: 'Задачи и версии', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Задачи и версии' });
  await dialog.getByLabel('Задание в очередь').fill('Задача предпросмотра');
  await dialog.getByRole('button', { name: 'Добавить и выполнять по очереди' }).click();
  await expect(dialog).toContainText('В очереди');
  await dialog.getByRole('button', { name: 'Отменить задачу' }).click();
  await expect(dialog).toContainText('Отменено');
  await dialog.getByRole('button', { name: 'Контрольные точки', exact: true }).click();
  await expect(dialog).toContainText('Новые файлы не удаляются');
  await dialog.getByRole('button', { name: 'Ветка чата', exact: true }).click();
  await expect(dialog.getByRole('checkbox')).not.toBeChecked();
  await dialog.getByRole('button', { name: 'Создать ветку', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.chat-title')).toContainText('Ветка');
});
