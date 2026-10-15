// Offline only: prepare matched, disposable projects. Never invokes a model or provider.
import { mkdtemp, mkdir, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

const root = await mkdtemp(join(tmpdir(), 'bebekon-claude-comparison-'));
const scenarios = [
  ['architecture', ['Прочитай исходники и кратко объясни связи между модулями. Не меняй файлы.']],
  [
    'find-bug',
    ['Найди причину неверного subtotal. Укажи место и воспроизводящий пример, без правок.'],
  ],
  ['edit-method', ['Исправь subtotal и запусти node check.mjs. Другие методы не меняй.']],
  [
    'method-with-test',
    [
      'Добавь чистую функцию totalQuantity(items) и проверку в check.mjs. Не исправляй посторонние ошибки.',
    ],
  ],
  [
    'resume',
    [
      'Изучи src/cart.mjs и запомни назначение функций, без правок.',
      'В том же диалоге исправь subtotal и проверь результат.',
    ],
  ],
  [
    'tools',
    ['Найди subtotal поиском, прочитай реализацию, исправь ошибку и выполни node check.mjs.'],
  ],
  [
    'mcp',
    [
      'Вызови настроенный инструмент MCP один раз, получи фиксированные данные корзины и сопоставь subtotal с ожидаемым результатом. Не меняй файлы.',
    ],
  ],
  [
    'long-context',
    [
      'Прочитай docs и составь таблицу требований, затем исправь subtotal согласно им и выполни node check.mjs.',
    ],
  ],
];
const files = {
  'CLAUDE.md':
    'Use local project files. Keep changes focused. Do not access credentials or network. Report actual checks only.\n',
  'README.md':
    '# Offline comparison fixture\nIntentionally incorrect subtotal. No dependencies or network required.\n',
  'src/cart.mjs':
    'export function subtotal(items) { return items.reduce((sum, item) => sum + item.price * (item.quantity - 1), 0); }\n',
  'src/sample.mjs':
    'export const items = [{ price: 10, quantity: 2 }, { price: 5, quantity: 1 }];\n',
  'src/main.mjs':
    "import { subtotal } from './cart.mjs';\nimport { items } from './sample.mjs';\nconsole.log(subtotal(items));\n",
  'check.mjs':
    "import assert from 'node:assert/strict';\nimport { subtotal } from './src/cart.mjs';\nassert.equal(subtotal([]), 0);\nassert.equal(subtotal([{price:10, quantity:2}, {price:5, quantity:1}]), 25);\nconsole.log('checks passed');\n",
};
const manifest = {
  generated_by: 'offline preparation; no inference',
  model_runs: 0,
  comparison_results: null,
  required_controls: [
    'fixed model ID',
    'same CLI version and account',
    'same effort; Fast off',
    'same instructions/tools/MCP/permissions',
    'new session except resume scenario',
    'single mode in app',
    'record cache conditions',
  ],
  scenarios: [],
};
for (const [name, prompts] of scenarios) {
  const scenarioFiles = { ...files };
  if (name === 'long-context') {
    for (let i = 1; i <= 24; i++) {
      scenarioFiles[`docs/requirement-${String(i).padStart(2, '0')}.md`] =
        `# Requirement ${i}\n` +
        'For each item, multiply price by the full quantity. Empty input has subtotal zero. Preserve unrelated modules.\n'.repeat(
          60,
        );
    }
  }
  const hashes = {};
  const paths = {};
  for (const variant of ['native', 'app']) {
    const project = join(root, name, variant);
    paths[variant] = project;
    for (const [relative, body] of Object.entries(scenarioFiles)) {
      const destination = join(project, relative);
      await mkdir(join(destination, '..'), { recursive: true });
      await writeFile(destination, body, 'utf8');
      const hash = createHash('sha256')
        .update(await readFile(destination))
        .digest('hex');
      if (variant === 'native') hashes[relative] = hash;
      else assert.equal(hash, hashes[relative], `Fixture mismatch: ${name}/${relative}`);
    }
  }
  manifest.scenarios.push({
    name,
    prompts,
    ...paths,
    hashes,
    ready: name !== 'mcp',
    note:
      name === 'mcp'
        ? 'Requires the same user-selected deterministic MCP server and tool in both clients; not auto-installed.'
        : 'Offline fixture ready; intentionally failing check before the edit.',
  });
}
const path = join(root, 'manifest.json');
await writeFile(path, JSON.stringify(manifest, null, 2), 'utf8');
assert.equal(manifest.scenarios.length, 8);
console.log(
  JSON.stringify(
    { manifest: path, matched_project_pairs: 8, model_runs: 0, live_results: null },
    null,
    2,
  ),
);
