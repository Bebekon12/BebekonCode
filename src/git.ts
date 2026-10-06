import type { GitStatus } from './contracts';

export interface ChangeSummary {
  modified: number;
  added: number;
  deleted: number;
}

// Counts porcelain v1 XY codes reported by the core; renames count as modifications.
export function summarizeChanges(files: GitStatus['files']): ChangeSummary {
  const summary = { modified: 0, added: 0, deleted: 0 };
  for (const { status } of files) {
    if (status === '??' || status.includes('A')) summary.added++;
    else if (status.includes('D')) summary.deleted++;
    else summary.modified++;
  }
  return summary;
}
