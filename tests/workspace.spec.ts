import { test, expect } from '@playwright/test';

test('project tree and separate chat modes stay discoverable without duplicated chats', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto('/?preview=1');
  const sidebar = page.getByRole('complementary', { name: 'Проекты и чаты' });
  for (const name of ['Проекты', 'Обычные чаты', 'Команды агентов', 'Авторазбиение']) {
    await expect(sidebar.getByRole('region', { name, exact: true })).toBeVisible();
  }
  await sidebar.getByRole('button', { name: 'Добавить проект', exact: true }).click();
  await page.getByRole('button', { name: 'Новый чат в проекте sample-project' }).click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Сообщение агенту' });
  await input.fill('Проверь навигацию проекта');
  await input.press('Control+Enter');
  await expect(page.getByText(/^Ответ завершён/)).toBeVisible({ timeout: 15_000 });
  const project = sidebar.getByRole('region', { name: 'Проект sample-project', exact: true });
  await expect(project.locator('.recent-chat')).toHaveCount(1);
  await expect(
    sidebar.getByRole('region', { name: 'Обычные чаты', exact: true }).locator('.recent-chat'),
  ).toHaveCount(0);
  const user = await page.locator('.step-message:not(.agent)').boundingBox();
  const answer = await page.locator('.step-message.agent').boundingBox();
  expect(user!.x).toBeGreaterThan(answer!.x + 80);
  expect(user!.x + user!.width).toBeCloseTo(answer!.x + answer!.width, 0);
  expect(answer!.width).toBeGreaterThan(900);
  await page.screenshot({ path: 'test-results/workspace-wide-chat.png' });
  await sidebar.getByRole('button', { name: 'Свернуть проект sample-project' }).click();
  await expect(project.locator('.recent-chat')).toHaveCount(0);
  await sidebar.getByRole('button', { name: 'Развернуть проект sample-project' }).click();
  await expect(project.locator('.recent-chat')).toHaveCount(1);
  await sidebar.getByRole('button', { name: 'Создать обычный чат', exact: true }).click();
  await expect(
    page.getByRole('dialog', { name: 'Новый чат' }).getByLabel('Проект', { exact: true }),
  ).toHaveValue('');
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(
    sidebar.getByRole('region', { name: 'Обычные чаты', exact: true }).locator('.recent-chat'),
  ).toHaveCount(1);
  await expect(project.locator('.recent-chat')).toHaveCount(1);
  await expect(sidebar.locator('.recent-chat')).toHaveCount(2);
});

test('team has one shared answer and settings open without taking space from the chat', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 920 });
  await page.goto('/?preview=1');
  await page.getByRole('button', { name: 'Создать команду агентов', exact: true }).click();
  const wizard = page.getByRole('dialog', { name: 'Новый чат' });
  await expect(wizard.getByRole('button', { name: /Команда агентов/ })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(wizard.locator('.chat-agent-card')).toHaveCount(2);
  await wizard.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await expect(page.locator('.team-panel')).not.toHaveAttribute('open');
  const before = await page.locator('.chat-timeline').boundingBox();
  await page.locator('.team-panel > summary').click();
  await expect(page.locator('.team-member')).toHaveCount(2);
  const after = await page.locator('.chat-timeline').boundingBox();
  expect(after!.height).toBe(before!.height);
  await page.getByRole('button', { name: 'Настроить Рецензент', exact: true }).click();
  await expect(
    page.getByRole('dialog', { name: 'Настройки агента' }).getByLabel('Доступ', { exact: true }),
  ).toBeEnabled();
  await page.keyboard.press('Escape');
  await page.locator('.team-panel > summary').click();
  const input = page.getByRole('textbox', { name: 'Сообщение агенту' });
  await input.fill('Подготовьте единый итог по навигации');
  await input.press('Control+Enter');
  await expect(page.getByText(/^Ответ завершён/)).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('.step-message.agent')).toHaveCount(1);
  await expect(page.locator('.step-message.agent .entry-label')).toHaveText('Ответ команды');
  await expect(page.locator('.chat-activity-details')).not.toHaveAttribute('open');
  await expect(page.locator('.context-panel')).toHaveCount(0);
  await page.screenshot({ path: 'test-results/workspace-team-result.png' });
  await page.getByRole('button', { name: 'Плагины', exact: true }).click();
  const plugins = page.getByRole('dialog', { name: 'Плагины и инструменты' });
  await plugins.getByRole('button', { name: 'Подключённые', exact: true }).click();
  await expect(plugins.getByText('Пример плагина', { exact: true })).toBeVisible();
  await expect(
    plugins.getByRole('button', { name: 'Рецензент · выбрать инструменты' }),
  ).toBeVisible();
  await plugins.getByRole('button', { name: 'Рецензент · выбрать инструменты' }).click();
  const agentSettings = page.getByRole('dialog', { name: 'Настройки агента' });
  await expect(
    agentSettings.getByRole('button', { name: 'Плагины, MCP и навыки' }),
  ).toHaveAttribute('aria-expanded', 'true');
  await expect(agentSettings.getByRole('checkbox', { name: /Пример плагина/ })).toBeChecked();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Создать чат с авторазбиением' }).click();
  await expect(page.getByRole('button', { name: /Авторазбиение/ }).last()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
});
