import { useEffect, useRef, useState } from 'react';
import { ChevronRight, FileDiff } from 'lucide-react';
import type { GitStatus } from '../contracts';
import { diffTotals, parseDiff, type DiffFile } from '../diffstat';

export interface ChangeSource {
  load: () => Promise<{ status: GitStatus; diff: string }>;
  open: () => void;
}

interface Summary {
  files: DiffFile[];
  untracked: string[];
}

/**
 * Codex-style summary under the final answer: changed files with +/− lines.
 * Counts the uncommitted working tree, so earlier uncommitted edits are included.
 */
export function RunChanges({ source, refreshKey }: { source: ChangeSource; refreshKey: string }) {
  const [summary, setSummary] = useState<Summary | null>(null);
  const load = useRef(source.load);
  load.current = source.load;
  useEffect(() => {
    let alive = true;
    setSummary(null);
    load
      .current()
      .then(({ status, diff }) => {
        if (!alive) return;
        const files = parseDiff(diff);
        const known = new Set(files.map((file) => file.path));
        const untracked = status.files
          .filter((file) => file.status === '??' && !known.has(file.path))
          .map((file) => file.path);
        setSummary({ files, untracked });
      })
      // Not a Git project or Git unavailable: the answer simply has no change summary.
      .catch(() => alive && setSummary(null));
    return () => {
      alive = false;
    };
  }, [refreshKey]);
  if (!summary || (!summary.files.length && !summary.untracked.length)) return null;
  const totals = diffTotals(summary.files);
  const count = totals.files + summary.untracked.length;
  const shown = summary.files.slice(0, 5);
  return (
    <section className="run-changes" aria-label="Изменения в проекте">
      <button className="run-changes-head" onClick={source.open}>
        <FileDiff size={16} />
        <span className="run-changes-title">
          {count} {plural(count, 'файл изменён', 'файла изменено', 'файлов изменено')}
        </span>
        <DiffCounts
          added={totals.added}
          removed={totals.removed}
          approximate={totals.overlapping}
        />
        <span className="run-changes-open">
          Просмотреть <ChevronRight size={14} />
        </span>
      </button>
      <ul>
        {shown.map((file) => (
          <li key={file.path}>
            <code title={file.path}>{file.path}</code>
            {file.binary ? (
              <span className="muted">двоичный</span>
            ) : (
              <DiffCounts added={file.added} removed={file.removed} />
            )}
          </li>
        ))}
        {summary.untracked.slice(0, Math.max(0, 5 - shown.length)).map((path) => (
          <li key={path}>
            <code title={path}>{path}</code>
            <span className="muted">новый</span>
          </li>
        ))}
      </ul>
      <p
        className="run-changes-note"
        title="Все незафиксированные изменения проекта. Строки новых файлов не подсчитаны."
      >
        Рабочие изменения{count > 5 ? ` · ещё ${count - 5} файлов` : ''}
      </p>
    </section>
  );
}

export function DiffCounts({
  added,
  removed,
  approximate = false,
}: {
  added: number;
  removed: number;
  approximate?: boolean;
}) {
  return (
    <span
      className="diff-counts"
      title={
        approximate
          ? 'Файл изменён и в индексе, и вне его: строки могут учитываться дважды'
          : undefined
      }
    >
      {approximate && '≈ '}
      <span className="diff-added">+{added}</span>
      <span className="diff-removed">−{removed}</span>
    </span>
  );
}

function plural(count: number, one: string, few: string, many: string) {
  const tens = count % 100;
  const units = count % 10;
  if (tens >= 11 && tens <= 14) return many;
  if (units === 1) return one;
  if (units >= 2 && units <= 4) return few;
  return many;
}
