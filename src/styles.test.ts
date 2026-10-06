import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

describe('design tokens', () => {
  it('keeps raw colors inside the token block so themes stay swappable', () => {
    const css = readFileSync('src/styles.css', 'utf8');
    const rules = css.slice(css.indexOf('}') + 1);
    expect(rules.match(/#[0-9a-f]{3,8}\b|rgba?\(/gi) ?? []).toEqual([]);
  });
});
