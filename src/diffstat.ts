export interface DiffFile {
  path: string;
  added: number;
  removed: number;
  binary: boolean;
  /** The file appears in both the unstaged and the staged section; counts may overlap. */
  overlapping: boolean;
  lines: string[];
}

export interface DiffTotals {
  files: number;
  added: number;
  removed: number;
  overlapping: boolean;
}

// Section titles written by the core between `git diff` and `git diff --cached`.
const sectionTitles = new Set(['Изменения вне индекса', 'Изменения в индексе']);

/** Splits the core's combined unified diff into files with added/removed line counts. */
export function parseDiff(text: string): DiffFile[] {
  const files = new Map<string, DiffFile>();
  let current: DiffFile | null = null;
  let inHunk = false;
  let oldPath = '';
  let section = 0;
  const seen = new Map<string, number>();
  const finish = () => {
    if (!current) return;
    const existing = files.get(current.path);
    if (existing) {
      existing.added += current.added;
      existing.removed += current.removed;
      existing.binary ||= current.binary;
      existing.lines.push(...current.lines);
      existing.overlapping ||= seen.get(current.path) !== section;
    } else {
      files.set(current.path, current);
      seen.set(current.path, section);
    }
    current = null;
  };
  for (const line of text.split(/\r?\n/)) {
    if (sectionTitles.has(line)) {
      finish();
      section++;
      continue;
    }
    if (line.startsWith('diff --git ')) {
      finish();
      inHunk = false;
      oldPath = '';
      const quoted = line.lastIndexOf(' "b/');
      const plain = line.lastIndexOf(' b/');
      current = {
        path:
          quoted >= 0
            ? gitPath(`"${line.slice(quoted + 4)}`)
            : gitPath(plain >= 0 ? line.slice(plain + 3) : line.slice(11)),
        added: 0,
        removed: 0,
        binary: false,
        overlapping: false,
        lines: [line],
      };
      continue;
    }
    if (!current) continue;
    current.lines.push(line);
    if (!inHunk) {
      // A deleted file has `+++ /dev/null`, so its name comes from the preceding `--- a/` line.
      if (line.startsWith('--- a/')) oldPath = gitPath(line.slice(6));
      else if (line.startsWith('--- "a/')) oldPath = gitPath(`"${line.slice(7)}`);
      else if (line.startsWith('+++ b/')) current.path = gitPath(line.slice(6));
      else if (line.startsWith('+++ "b/')) current.path = gitPath(`"${line.slice(7)}`);
      else if (line === '+++ /dev/null' && oldPath) current.path = oldPath;
      else if (line.startsWith('Binary files ')) current.binary = true;
    }
    if (line.startsWith('@@')) inHunk = true;
    else if (inHunk && line.startsWith('+')) current.added++;
    else if (inHunk && line.startsWith('-')) current.removed++;
  }
  finish();
  return [...files.values()];
}

export function diffTotals(files: DiffFile[]): DiffTotals {
  return files.reduce<DiffTotals>(
    (totals, file) => ({
      files: totals.files + 1,
      added: totals.added + file.added,
      removed: totals.removed + file.removed,
      overlapping: totals.overlapping || file.overlapping,
    }),
    { files: 0, added: 0, removed: 0, overlapping: false },
  );
}

/** Git quotes non-ASCII paths as C strings with octal UTF-8 bytes. */
function gitPath(raw: string): string {
  const path = raw.replace(/\t$/, '');
  if (!path.startsWith('"') || !path.endsWith('"')) return path;
  const escaped: Record<string, string> = { n: '\n', t: '\t', '"': '"', '\\': '\\' };
  const encoder = new TextEncoder();
  const bytes: number[] = [];
  const body = path.slice(1, -1);
  for (let i = 0; i < body.length; i++) {
    const char = body.charAt(i);
    const octal = /^\\([0-7]{3})/.exec(body.slice(i))?.[1];
    if (octal) {
      bytes.push(parseInt(octal, 8));
      i += 3;
    } else if (char === '\\' && i + 1 < body.length) {
      const next = body.charAt(i + 1);
      bytes.push(...encoder.encode(escaped[next] ?? next));
      i++;
    } else {
      bytes.push(...encoder.encode(char));
    }
  }
  return new TextDecoder().decode(new Uint8Array(bytes));
}
