import { describe, expect, it } from 'vitest';
import { diffTotals, parseDiff } from './diffstat';

const modified = [
  'diff --git a/src/a.ts b/src/a.ts',
  'index 1111111..2222222 100644',
  '--- a/src/a.ts',
  '+++ b/src/a.ts',
  '@@ -1,3 +1,4 @@',
  ' keep',
  '-old',
  '+new',
  '+--flag is content, not a header',
  '\\ No newline at end of file',
].join('\n');

describe('parseDiff', () => {
  it('counts added and removed lines per file, ignoring headers', () => {
    const [file] = parseDiff(`Изменения вне индекса\n${modified}\nИзменения в индексе\n`);
    expect(file).toMatchObject({ path: 'src/a.ts', added: 2, removed: 1, overlapping: false });
  });

  it('names deleted files from the old path and marks binary files', () => {
    const files = parseDiff(
      [
        'diff --git a/gone.txt b/gone.txt',
        'deleted file mode 100644',
        '--- a/gone.txt',
        '+++ /dev/null',
        '@@ -1,2 +0,0 @@',
        '-one',
        '-two',
        'diff --git a/logo.png b/logo.png',
        'Binary files a/logo.png and b/logo.png differ',
      ].join('\n'),
    );
    expect(files).toEqual([
      expect.objectContaining({ path: 'gone.txt', added: 0, removed: 2 }),
      expect.objectContaining({ path: 'logo.png', binary: true }),
    ]);
  });

  it('decodes quoted non-ASCII paths and flags files in both sections', () => {
    const quoted = [
      'diff --git "a/\\320\\264\\320\\276\\320\\272.md" "b/\\320\\264\\320\\276\\320\\272.md"',
      '--- "a/\\320\\264\\320\\276\\320\\272.md"',
      '+++ "b/\\320\\264\\320\\276\\320\\272.md"',
      '@@ -0,0 +1 @@',
      '+текст',
    ].join('\n');
    const files = parseDiff(`Изменения вне индекса\n${quoted}\nИзменения в индексе\n${quoted}`);
    expect(files).toHaveLength(1);
    expect(files[0]).toMatchObject({ path: 'док.md', added: 2, overlapping: true });
    expect(diffTotals(files)).toEqual({ files: 1, added: 2, removed: 0, overlapping: true });
  });
});
