import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

describe('design tokens', () => {
  it.each(['src/styles.css', 'src/chat-polish.css'])(
    'keeps raw colors inside the token block in %s so themes stay swappable',
    (file) => {
      const css = readFileSync(file, 'utf8');
      const rules = css.replace(/:root(?:\[[^\]]+\])?\s*\{[^}]*\}/g, '');
      expect(rules.match(/#[0-9a-f]{3,8}\b|rgba?\(/gi) ?? []).toEqual([]);
    },
  );
});
