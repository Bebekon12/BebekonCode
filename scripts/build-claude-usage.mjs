import { build } from 'esbuild';
import { resolve } from 'node:path';
import { readFileSync } from 'node:fs';

const license = readFileSync(
  resolve(import.meta.dirname, '../node_modules/@anthropic-ai/claude-agent-sdk/LICENSE.md'),
  'utf8',
).replaceAll('*/', '* /');

await build({
  entryPoints: [resolve(import.meta.dirname, 'claude-usage.mjs')],
  outfile: process.argv[2],
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node18',
  banner: {
    js: `/* ${license} */\nimport { createRequire } from 'node:module'; const require = createRequire(import.meta.url);`,
  },
  legalComments: 'inline',
});
