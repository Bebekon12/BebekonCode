import { test, expect } from '@playwright/test';

test('project, session, streaming, stop and keyboard commands', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/?preview=1');
  await expect(page.getByRole('button', { name: 'Add your first project' })).toBeEnabled();
  await page.screenshot({ path: 'test-results/overview.png', fullPage: true });
  await page.getByRole('button', { name: 'Add your first project' }).click();
  await page.getByRole('button', { name: 'Start a session', exact: true }).first().click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(
    page.getByLabel('Agent', { exact: true }).locator('option[value="openai"]'),
  ).toHaveAttribute('disabled', '');
  await page.getByRole('button', { name: 'Create session' }).click();
  await page
    .getByRole('textbox', { name: 'Message to agent' })
    .fill('Design a safe provider boundary');
  await page.getByRole('textbox', { name: 'Message to agent' }).press('Control+Enter');
  await expect(page.getByRole('button', { name: 'Stop agent' })).toBeVisible();
  await expect(page.getByText('This is a local demo response.', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Stop agent' }).click();
  await expect(page.getByText('Stopped by you')).toBeVisible();
  await page.screenshot({ path: 'test-results/session.png', fullPage: true });
  await page.keyboard.press('Control+k');
  await page.getByRole('textbox', { name: 'Search commands' }).fill('settings');
  await page.getByRole('button', { name: 'Open settings' }).click();
  await expect(page.getByRole('dialog', { name: 'Settings' })).toBeVisible();
  await page.getByRole('button', { name: 'About & updates' }).click();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(page.getByText('No stable release available')).toBeVisible();
  await page.screenshot({ path: 'test-results/settings.png', fullPage: true });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('multiple sessions retain separate timelines', async ({ page }) => {
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Add your first project' }).click();
  await page.keyboard.press('Control+n');
  await page.getByRole('button', { name: 'Create session' }).click();
  await page.getByRole('textbox', { name: 'Message to agent' }).fill('First independent task');
  await page.getByRole('button', { name: 'Send message' }).click();
  await page.keyboard.press('Control+n');
  await page.getByRole('button', { name: 'Create session' }).click();
  await page.getByRole('textbox', { name: 'Message to agent' }).fill('Second independent task');
  await page.getByRole('button', { name: 'Send message' }).click();
  await page.getByRole('button', { name: 'Stop agent' }).click();
  await expect(page.getByText('Stopped by you')).toBeVisible();
  await page.getByRole('button', { name: /First independent task Demo/ }).click();
  await expect(page.getByText('Turn completed')).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole('textbox', { name: 'Message to agent' })).toBeEnabled();
  await expect(page.locator('.prose').filter({ hasText: 'Second independent task' })).toHaveCount(
    0,
  );
});

test('layout remains contained at Windows 150% logical resolution', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto('/?preview=1');
  await expect(page.getByRole('button', { name: 'Add your first project' })).toBeEnabled();
  const dimensions = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth,
    viewport: innerWidth,
  }));
  expect(dimensions.width).toBeLessThanOrEqual(dimensions.viewport);
  await page.screenshot({ path: 'test-results/compact.png', fullPage: true });
});
