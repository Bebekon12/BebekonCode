import { test, expect, type Page } from '@playwright/test';

async function updaterFixture(page: Page, current = false) {
  // Only this browser test replaces preview methods; production always uses native IPC.
  await page.addInitScript(
    ({ current }) => {
      const fixture = {
        calls: 0,
        check: async () =>
          current
            ? null
            : { version: '9.0.0', currentVersion: '0.3.2', notes: 'Тестовый выпуск', date: null },
        install: (_version: string, progress: (value: unknown) => void) => {
          fixture.calls++;
          progress({ phase: 'downloading', downloaded: 50, total: 100 });
          return new Promise<void>((_resolve, reject) => {
            Object.assign(window, { failUpdate: () => reject(new Error('Подпись не совпадает')) });
          });
        },
      };
      Object.assign(window, { updateFixture: fixture });
    },
    { current },
  );
  await page.route('**/src/preview.ts', async (route) => {
    const response = await route.fetch();
    const body = await response.text();
    expect(body).toContain('checkUpdate: async () => {');
    expect(body).toContain('installUpdate: async () => {');
    await route.fulfill({
      response,
      body: body
        .replace(
          'checkUpdate: async () => {',
          'checkUpdate: async () => { return window.updateFixture.check();',
        )
        .replace(
          'installUpdate: async () => {',
          'installUpdate: async (version, progress) => { return window.updateFixture.install(version, progress);',
        ),
    });
  });
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'О программе и обновления' }).click();
  await page.getByRole('button', { name: 'Проверить обновления', exact: true }).click();
}

test('update requires confirmation, preserves progress during navigation and recovers from failure', async ({
  page,
}) => {
  await updaterFixture(page);
  await expect(page.getByText('Доступна версия 9.0.0')).toBeVisible();
  await page.getByRole('button', { name: 'Установить и перезапустить', exact: true }).click();
  const confirmation = page.getByRole('alertdialog', { name: 'Установка обновления' });
  await confirmation.getByRole('button', { name: 'Отмена', exact: true }).click();
  expect(
    await page.evaluate(
      () => (window as unknown as { updateFixture: { calls: number } }).updateFixture.calls,
    ),
  ).toBe(0);
  await page.getByRole('button', { name: 'Установить и перезапустить', exact: true }).click();
  await confirmation
    .getByRole('button', { name: 'Установить и перезапустить', exact: true })
    .click();
  await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+n');
  await page.keyboard.press('Control+k');
  await expect(page.getByRole('dialog', { name: 'Настройки' })).toBeVisible();
  await expect(
    page.getByRole('dialog').getByRole('button', { name: 'Закрыть окно' }),
  ).toBeDisabled();
  await page.evaluate(() => (window as unknown as { failUpdate: () => void }).failUpdate());
  await expect(page.getByRole('alert')).toContainText('Подпись не совпадает');
  await expect(
    page.getByRole('button', { name: 'Проверить обновления', exact: true }),
  ).toBeEnabled();
  await page.screenshot({ path: 'test-results/update-error.png' });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('current version does not offer installation', async ({ page }) => {
  await updaterFixture(page, true);
  await expect(page.getByText(/Установлена актуальная версия/)).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Установить и перезапустить', exact: true }),
  ).toHaveCount(0);
});
