import { useState } from 'react';
import { ChevronRight, FileDiff } from 'lucide-react';
import type { ChangedFile } from '../contracts';

/** Persisted counts for the latest turn; never fetch the whole-project diff. */
export function RunChanges({ summary }: { summary: { files: ChangedFile[]; limited: boolean } }) {
  const [expanded, setExpanded] = useState(false);
  const { files, limited } = summary;
  if (!files.length) return null;
  const added = files.reduce((sum, file) => sum + (file.added ?? 0), 0);
  const removed = files.reduce((sum, file) => sum + (file.removed ?? 0), 0);
  const unknown = limited || files.some((file) => file.added === null || file.removed === null);
  return (
    <section className="run-changes" aria-label="Изменения за последнее сообщение">
      <button
        className="run-changes-head"
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
      >
        <FileDiff size={16} />
        <span className="run-changes-title">
          {files.length
            ? `${files.length} ${plural(files.length, 'файл изменён', 'файла изменено', 'файлов изменено')}`
            : 'Снимок изменений неполный'}
        </span>
        <DiffCounts
          added={added}
          removed={removed}
          approximate={unknown}
          approximationReason="Учтены подсчитанные строки; часть файлов или строк не удалось сравнить"
        />
        <span className="run-changes-open">
          {expanded ? 'Свернуть' : 'Просмотреть'} <ChevronRight size={14} />
        </span>
      </button>
      <ul>
        {(expanded ? files : files.slice(0, 5)).map((file) => (
          <li key={file.path}>
            <code title={file.path}>{file.path}</code>
            {file.added === null || file.removed === null ? (
              <span className="muted">
                {file.status === 'added'
                  ? 'новый'
                  : file.status === 'deleted'
                    ? 'удалён'
                    : 'изменён'}{' '}
                · строки не подсчитаны
              </span>
            ) : (
              <DiffCounts added={file.added} removed={file.removed} />
            )}
          </li>
        ))}
      </ul>
      <p
        className="run-changes-note"
        title="Сравнение файлов до и после сообщения. Учитываются также параллельные правки в этой папке. Содержимое файлов в истории не сохраняется."
      >
        За последнее сообщение
        {!expanded && files.length > 5 ? ` · ещё ${files.length - 5} файлов` : ''}
        {limited ? ' · снимок неполный' : ''}
      </p>
    </section>
  );
}

export function DiffCounts({
  added,
  removed,
  approximate = false,
  approximationReason = 'Файл изменён и в индексе, и вне его: строки могут учитываться дважды',
}: {
  added: number;
  removed: number;
  approximate?: boolean;
  approximationReason?: string;
}) {
  return (
    <span className="diff-counts" title={approximate ? approximationReason : undefined}>
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
