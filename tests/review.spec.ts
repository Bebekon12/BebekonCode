import { test, expect } from '@playwright/test';

test('document and table comments survive reopening and can be resolved or discussed', async ({
  page,
}) => {
  await page.goto('/?preview=1&review=1');
  await page.getByRole('button', { name: 'Добавить проект', exact: true }).click();
  await page.getByRole('button', { name: 'Новый чат в проекте sample-project' }).click();
  await page.getByRole('button', { name: 'Создать чат', exact: true }).click();
  await page.getByRole('button', { name: 'Файлы проекта', exact: true }).click();
  const files = page.getByRole('dialog');
  await files.getByRole('button', { name: 'задачи.csv', exact: true }).click();
  await files.locator('[data-review-anchor="csv:B2"]').click();
  await files.getByLabel('Новый комментарий').fill('Проверь статус дизайна');
  await files.getByRole('button', { name: 'Добавить', exact: true }).click();
  await expect(files.locator('.review-comment')).toContainText('Проверь статус дизайна');
  await files.getByRole('button', { name: 'Решено', exact: true }).click();
  await files.getByLabel('Фильтр комментариев').selectOption('resolved');
  await files.getByRole('button', { name: 'Открыть снова', exact: true }).click();
  await files.getByLabel('Фильтр комментариев').selectOption('open');
  await files.getByRole('button', { name: 'обзор.md', exact: true }).click();
  await files.getByRole('button', { name: 'Просмотр и комментарии', exact: true }).click();
  await files.locator('.review-paragraph').nth(1).click();
  await files.getByLabel('Новый комментарий').fill('Уточнить план');
  await files.getByRole('button', { name: 'Закрыть окно', exact: true }).click();
  await expect(files.getByText(/Не сохранять черновик/)).toBeVisible();
  await files.getByRole('button', { name: 'Продолжить редактирование' }).click();
  await files.getByRole('button', { name: 'Добавить', exact: true }).click();
  await files.getByRole('button', { name: 'задачи.csv', exact: true }).click();
  await expect(files.locator('.review-comment')).toContainText('Проверь статус дизайна');
  await page.screenshot({ path: 'test-results/table-review.png' });
  // A long discussion is bounded explicitly, rather than silently losing comments.
  await page.evaluate(async () => {
    const modulePath = '/src/preview.ts';
    const { preview } = (await import(modulePath)) as typeof import('../src/preview');
    const workspace = (await preview.snapshot()).workspaces.find((w) => w.id !== 'chat-scratch')!;
    for (let i = 0; i < 2; i++)
      await preview.addReviewComment(workspace.id, 'задачи.csv', {
        fingerprint: 'preview-document-v1',
        anchor: 'csv:A1',
        quote: 'Задача',
        body: 'Ж'.repeat(6000),
      });
  });
  await files
    .locator('.review-toolbar')
    .getByRole('button', { name: 'Обновить', exact: true })
    .click();
  await expect(files.locator('.review-comment')).toHaveCount(3);
  await files.getByRole('button', { name: 'Обсудить в чате', exact: true }).click();
  await expect(page.getByLabel('Сообщение агенту')).toHaveValue(/Проверь статус дизайна/);
  await expect(page.getByLabel('Сообщение агенту')).toHaveValue(/Список сокращён/);
  expect(
    await page
      .getByLabel('Сообщение агенту')
      .evaluate((e: HTMLTextAreaElement) => new TextEncoder().encode(e.value).length),
  ).toBeLessThan(32000);
});

test('Office parser reads real XML parts, cached formulas and rejects external entities', async ({
  page,
}) => {
  await page.goto('/?preview=1');
  const result = await page.evaluate(async () => {
    const modulePath = '/src/review.ts';
    const { parseReview, parseDelimited } = (await import(
      modulePath
    )) as typeof import('../src/review');
    const base = { fingerprint: 'test', text: null };
    const doc = parseReview({
      ...base,
      kind: 'docx',
      parts: [
        {
          name: 'word/document.xml',
          content:
            '<w:document xmlns:w="urn:word"><w:body><w:p><w:r><w:t>Русский текст</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Ячейка</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>',
        },
      ],
    });
    const xlsx = parseReview({
      ...base,
      kind: 'xlsx',
      parts: [
        {
          name: 'xl/workbook.xml',
          content:
            '<workbook xmlns:r="urn:rels"><sheets><sheet name="План" r:id="rId1"/></sheets></workbook>',
        },
        {
          name: 'xl/_rels/workbook.xml.rels',
          content:
            '<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>',
        },
        { name: 'xl/sharedStrings.xml', content: '<sst><si><t>Название</t></si></sst>' },
        {
          name: 'xl/worksheets/sheet1.xml',
          content:
            '<worksheet><sheetData><row><c r="A1" t="s"><v>0</v></c><c r="B2"><f>1+2</f><v>3</v></c><c r="C2" t="inlineStr"><is><t>Готово</t></is></c></row></sheetData></worksheet>',
        },
      ],
    });
    let blocked = false;
    try {
      parseReview({
        ...base,
        kind: 'docx',
        parts: [
          {
            name: 'word/document.xml',
            content: '<!DOCTYPE x [<!ENTITY bad SYSTEM "file:///secret">]><x/>',
          },
        ],
      });
    } catch {
      blocked = true;
    }
    return { doc, xlsx, blocked, csv: parseDelimited('a,b\n"line\nbreak","a""b"', ',') };
  });
  expect(result.doc.blocks[0]!.text).toBe('Русский текст');
  expect(result.doc.sheets[0]!.rows).toEqual([['Ячейка']]);
  expect(result.xlsx.sheets[0]!.name).toBe('План');
  expect(result.xlsx.sheets[0]!.rows[1]).toEqual(['', '3', 'Готово']);
  expect(result.csv.rows[1]).toEqual(['line\nbreak', 'a"b']);
  expect(result.blocked).toBe(true);
});
