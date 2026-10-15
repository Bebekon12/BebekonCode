import { test, expect } from '@playwright/test';

test('context preferences are local and the composer can queue an attachment while working', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  await page.locator('.new-session-button').click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  const ring = page.getByRole('button', { name: 'Контекст и оформление ответа', exact: true });
  const ringBox = (await ring.boundingBox())!;
  const modelBox = (await page
    .getByRole('button', { name: 'Модель в чате', exact: true })
    .boundingBox())!;
  expect(
    Math.abs(ringBox.y + ringBox.height / 2 - (modelBox.y + modelBox.height / 2)),
  ).toBeLessThan(3);
  expect(ringBox.width).toBeLessThanOrEqual(30);
  await expect(page.locator('.composer')).not.toContainText('Контекст —');
  await page.screenshot({ path: 'test-results/context-ring-single.png' });

  await page.getByRole('button', { name: 'Контекст и оформление ответа', exact: true }).click();
  const panel = page.getByRole('dialog', { name: 'Контекст и оформление ответа' });
  await expect(panel).toContainText('Нет данных');
  await panel.getByLabel('Оформление ответа').selectOption('visual');
  await panel.getByRole('button', { name: 'Сохранить настройки чата' }).click();
  await expect(panel.getByLabel('Оформление ответа')).toHaveValue('visual');
  await expect(page.locator('.step')).toHaveCount(0);
  await page.getByRole('button', { name: 'Контекст и оформление ответа', exact: true }).click();
  await page.getByLabel('Сообщение агенту').fill('First task');
  await page.getByRole('button', { name: 'Отправить сообщение', exact: true }).click();
  await page.getByLabel('Сообщение агенту').fill('Follow-up with file');
  await page.getByLabel('Выбрать вложения').setInputFiles({
    name: 'note.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('Additional context'),
  });
  await page.getByRole('button', { name: 'Добавить в очередь', exact: true }).click();
  const queue = page.getByLabel('Следующие сообщения');
  await expect(queue).toContainText('Follow-up with file');
  await expect(queue).toContainText('С прикреплёнными файлами');
  await queue.getByRole('button', { name: 'Изменить', exact: true }).click();
  await queue.getByLabel('Изменить сообщение очереди').fill('Edited next task');
  await queue.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await expect(queue).toContainText('Edited next task');
  await queue.getByRole('button', { name: 'Убрать', exact: true }).click();
  await expect(queue).toHaveCount(0);
});

for (const provider of ['openai', 'anthropic'])
  test(`live ${provider} correction stays in the active task and queue remains separate`, async ({
    page,
  }) => {
    await page.goto('/?preview=1');
    await page.getByRole('button', { name: 'Аккаунты', exact: true }).click();
    const accounts = page.getByRole('dialog');
    if (provider === 'anthropic')
      await accounts.getByRole('button', { name: /Claude Готов/ }).click();
    await accounts
      .getByRole('button', {
        name: provider === 'anthropic' ? 'Добавить аккаунт Claude' : 'Добавить аккаунт ChatGPT',
        exact: true,
      })
      .click();
    await accounts.getByLabel('Название аккаунта').fill('Live fixture');
    await accounts.getByRole('button', { name: 'Добавить', exact: true }).click();
    await accounts
      .getByRole('button', {
        name: provider === 'anthropic' ? 'Войти через Claude Code' : 'Continue with ChatGPT',
        exact: true,
      })
      .click();
    if (provider === 'openai')
      await page
        .getByRole('dialog', { name: 'Вы используете свой план ChatGPT' })
        .getByRole('button', { name: 'Понятно' })
        .click();
    await page.keyboard.press('Escape');
    await page.locator('.new-session-button').click();
    await page
      .getByRole('dialog', { name: 'Новый чат' })
      .getByLabel('Провайдер', { exact: true })
      .selectOption(provider);
    await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
    await page.clock.install();
    await page.clock.pauseAt(new Date());
    await page.getByLabel('Сообщение агенту').fill('Initial request');
    await page.getByRole('button', { name: 'Отправить сообщение', exact: true }).click();
    await page.clock.runFor(100);
    await page.getByLabel('Сообщение агенту').fill('Use the new requirement');
    await page.getByRole('button', { name: 'Передать уточнение', exact: true }).click();
    await page.clock.runFor(50);
    await expect(page.locator('.followup-message')).toContainText('Use the new requirement');
    await expect(page.locator('.step-running')).toHaveCount(1);
    await page.getByRole('button', { name: 'Остановить агента' }).click();
    await page.clock.runFor(1000);
    await expect(page.locator('.step-stopped')).toHaveCount(1);
  });

test('answer images sit beside prose and charts remain readable in both themes', async ({
  page,
}) => {
  await page.route('https://images.example.com/landscape.svg', (route) =>
    route.fulfill({
      contentType: 'image/svg+xml',
      headers: { 'access-control-allow-origin': '*' },
      body: '<svg xmlns="http://www.w3.org/2000/svg" width="640" height="420" viewBox="0 0 640 420"><rect width="640" height="420" fill="#b9d9e8"/><circle cx="480" cy="100" r="45" fill="#fff1bf"/><path d="M0 330L190 100L390 330Z" fill="#6c8e91"/><path d="M210 350L420 155L640 350Z" fill="#42696e"/><path d="M0 350Q320 260 640 350V420H0Z" fill="#9abcb2"/></svg>',
    }),
  );
  await page.goto('/?preview=1');
  await page.evaluate(async () => {
    const reactPath = '/node_modules/.vite/deps/react.js',
      domPath = '/node_modules/.vite/deps/react-dom_client.js',
      componentPath = '/src/components/ChatMarkdown.tsx';
    const React = (await import(reactPath)).default,
      { createRoot } = (await import(domPath)).default,
      { ChatMarkdown } = await import(componentPath);
    document.body.innerHTML =
      '<main id="answer-fixture" style="width:850px;margin:40px auto;padding:36px;background:var(--surface);border-radius:20px"></main>';
    createRoot(document.getElementById('answer-fixture')!).render(
      React.createElement(ChatMarkdown, {
        streaming: false,
        text: '# Маршрут и впечатления\n\n![Горный пейзаж · демонстрационная иллюстрация](https://images.example.com/landscape.svg)\n\nНачните с обзорной тропы: она даёт хороший вид на долину и оставляет время для отдыха. Изображение помогает представить маршрут, а текст рядом объясняет, что важно учесть.\n\n**Три ориентира:** удобная обувь, запас воды и проверенный прогноз. Все числа ниже — условные данные для проверки отображения. [Источник иллюстрации](https://images.example.com/landscape.svg).\n\n## Изменение высоты\n\n```bebekon-line\n{"title":"Высота по участкам, м · пример","items":[{"label":"Старт","value":120},{"label":"Лес","value":180},{"label":"Обзорная точка","value":340},{"label":"Финиш","value":160}]}\n```\n\n| Участок | Время |\n|---|---|\n| Лесная тропа | 40 минут |\n| Обзорная площадка | 20 минут |',
      }),
    );
  });
  await expect(page.locator('.answer-image img')).toBeVisible();
  await expect(page.getByRole('img', { name: 'Высота по участкам, м · пример' })).toBeVisible();
  await expect(
    page.locator('.chat-markdown > p').filter({ has: page.locator('.answer-image') }),
  ).toHaveCSS('float', 'right');
  await page.screenshot({
    path: 'test-results/conversation-rich-dark.png',
    animations: 'disabled',
  });
  await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'light'));
  await page.screenshot({
    path: 'test-results/conversation-rich-light.png',
    animations: 'disabled',
  });
  await page.getByRole('button', { name: /Увеличить:/ }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Закрыть изображение' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
