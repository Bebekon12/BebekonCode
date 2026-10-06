import { describe, expect, it } from 'vitest';
import { summarizeChanges } from './git';

describe('git change summary', () => {
  it('groups porcelain codes into modified, added and deleted files', () => {
    const files = [' M', 'M ', 'R ', '??', 'A ', ' D', 'D '].map((status, index) => ({
      status,
      path: `file-${index}`,
    }));
    expect(summarizeChanges(files)).toEqual({ modified: 3, added: 2, deleted: 2 });
  });
});
