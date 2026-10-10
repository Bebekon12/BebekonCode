import { expect, test } from '@playwright/test';

test('Fast is opt-in, independent of effort, and remains off in another chat', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
  await page.getByRole('button', { name: 'Добавить аккаунт ChatGPT' }).click();
  await page.getByLabel('Название аккаунта').fill('Fast preview');
  await page.getByRole('button', { name: 'Добавить', exact: true }).click();
  await page.getByRole('button', { name: 'Continue with ChatGPT' }).click();
  await page
    .getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })
    .getByRole('button', { name: 'Понятно' })
    .click();
  await page.keyboard.press('Escape');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('button', { name: 'Модель в чате', exact: true }).click();
  const fast = page.getByRole('button', { name: 'Скоростной режим', exact: true });
  await expect(fast).toHaveAttribute('aria-pressed', 'false');
  const effort = page.getByRole('slider', { name: 'Уровень обдумывания' });
  await effort.fill('2');
  await fast.click();
  const confirm = page.getByRole('dialog', { name: 'Включить скоростной режим' });
  await expect(confirm).toContainText('2,5×');
  await confirm.getByRole('button', { name: 'Отмена' }).click();
  await expect(fast).toHaveAttribute('aria-pressed', 'false');
  await fast.click();
  await confirm.getByRole('button', { name: 'Включить Fast' }).click();
  await expect(fast).toHaveAttribute('aria-pressed', 'true');
  await expect(effort).toHaveValue('2');
  await page.screenshot({ path: 'test-results/fast-mode-preview.png' });
  await page.getByRole('button', { name: 'Готово', exact: true }).click();
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('button', { name: 'Модель в чате', exact: true }).click();
  await expect(fast).toHaveAttribute('aria-pressed', 'false');
});
