import { useEffect, useState } from 'react';
import {
  Bot,
  ChevronRight,
  Circle,
  CircleCheck,
  CircleDashed,
  FilePen,
  FilePlus2,
  FileText,
  Globe,
  Plug,
  Search,
  SquareTerminal,
  Wrench,
  X,
} from 'lucide-react';
import { plural } from './RunChanges';
import type { TodoEntry, ToolItem } from '../timeline';

const icons: Record<ToolItem['kind'], typeof Wrench> = {
  command: SquareTerminal,
  read: FileText,
  edit: FilePen,
  create: FilePlus2,
  write: FilePen,
  search: Search,
  web: Globe,
  agent: Bot,
  mcp: Plug,
  other: Wrench,
  activity: Wrench,
};

const fileName = (path: string) => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;

function Diff({ added, removed }: { added?: number | null; removed?: number | null }) {
  if (added == null && removed == null) return null;
  return (
    <span className="tool-diff">
      <span className="diff-added">+{added ?? 0}</span>
      <span className="diff-removed">−{removed ?? 0}</span>
    </span>
  );
}

/** "Выполнено 3 команды, изменён App.tsx +4 −1", in the spirit of Claude Desktop. */
function headline(items: ToolItem[]) {
  const by = (kinds: ToolItem['kind'][]) => items.filter((item) => kinds.includes(item.kind));
  const parts: string[] = [];
  const count = (list: ToolItem[], one: string, few: string, many: string) => {
    if (!list.length) return;
    const failed = list.filter((item) => item.state === 'failed').length;
    parts.push(
      `${list.length} ${plural(list.length, one, few, many)}${failed ? ` (ошибок: ${failed})` : ''}`,
    );
  };
  count(by(['command']), 'команда', 'команды', 'команд');
  count(by(['read']), 'файл прочитан', 'файла прочитано', 'файлов прочитано');
  count(by(['search']), 'поиск', 'поиска', 'поисков');
  count(by(['web']), 'веб-запрос', 'веб-запроса', 'веб-запросов');
  count(by(['agent']), 'субагент', 'субагента', 'субагентов');
  count(by(['mcp', 'other']), 'вызов инструмента', 'вызова инструментов', 'вызовов инструментов');
  const activities = by(['activity']);
  if (activities.length === 1) parts.push(activities[0]!.summary);
  else count(activities, 'действие', 'действия', 'действий');
  const files = by(['edit', 'create', 'write']);
  const unique = [...new Set(files.map((item) => item.summary))];
  const created = files.some((item) => item.kind === 'create');
  const text = parts.join(', ');
  return {
    text: text ? text[0]!.toUpperCase() + text.slice(1) : '',
    file:
      unique.length === 1
        ? { label: `${created ? 'создан' : 'изменён'} ${fileName(unique[0]!)}`, title: unique[0]! }
        : unique.length > 1
          ? {
              label: `${unique.length} ${plural(unique.length, 'файл изменён', 'файла изменено', 'файлов изменено')}`,
              title: unique.join('\n'),
            }
          : undefined,
    added: files.reduce((sum, item) => sum + (item.added ?? 0), 0),
    removed: files.reduce((sum, item) => sum + (item.removed ?? 0), 0),
    hasDiff: files.some((item) => item.added != null || item.removed != null),
  };
}

export function ToolGroup({ items, expanded = false }: { items: ToolItem[]; expanded?: boolean }) {
  const [open, setOpen] = useState(expanded);
  const finished = items.filter((item) => item.state !== 'running');
  if (!finished.length) return null;
  const head = headline(finished);
  return (
    <div className={`tool-group ${open ? 'open' : ''}`}>
      <button className="tool-group-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        {head.text && <span>{head.text}</span>}
        {head.text && head.file && <span>,&nbsp;</span>}
        {head.file && (
          <span className="tool-group-file" title={head.file.title}>
            {head.text
              ? head.file.label
              : head.file.label[0]!.toUpperCase() + head.file.label.slice(1)}
          </span>
        )}
        {head.hasDiff && <Diff added={head.added} removed={head.removed} />}
        <ChevronRight size={14} className="tool-group-chevron" />
      </button>
      {open && (
        <ul className="tool-list">
          {finished.map((item) => {
            const Icon = icons[item.kind];
            return (
              <li key={item.id} className={`tool-row tool-${item.kind} ${item.state}`}>
                <Icon size={14} className="tool-icon" />
                <span
                  className={item.kind === 'command' ? 'tool-summary mono' : 'tool-summary'}
                  title={item.detail ? `${item.summary}\n${item.detail}` : item.summary}
                >
                  {item.summary || item.tool}
                </span>
                <Diff added={item.added} removed={item.removed} />
                {item.state === 'failed' && (
                  <span className="tool-failed" title="Завершилось с ошибкой">
                    <X size={12} /> ошибка
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

const verbs: Partial<Record<ToolItem['kind'], string>> = {
  command: 'Выполняется',
  read: 'Читает',
  edit: 'Изменяет',
  create: 'Создаёт',
  write: 'Записывает',
  search: 'Ищет',
  web: 'Ищет в сети',
  agent: 'Субагент',
  mcp: 'Вызывает',
};

function elapsed(seconds: number) {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  return minutes ? `${minutes}м ${total % 60}с` : `${total}с`;
}

/** The working line under the answer: current tool and its running time. */
export function LiveStatus({ item, label }: { item?: ToolItem; label?: string }) {
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => window.clearInterval(timer);
  }, []);
  return (
    <div className="live-status" role="status">
      <span className="live-dots" aria-hidden="true">
        <i />
        <i />
        <i />
        <i />
      </span>
      {item ? (
        <>
          <span className="live-verb">{verbs[item.kind] ?? 'Выполняется'}</span>
          <span className={item.kind === 'command' ? 'live-target mono' : 'live-target'}>
            {item.kind === 'read' || item.kind === 'edit' || item.kind === 'write'
              ? fileName(item.summary)
              : item.summary || item.tool}
          </span>
          <span className="live-time">{elapsed(now - item.startedAt)}</span>
        </>
      ) : (
        <span className="live-verb">{label ?? 'Думает…'}</span>
      )}
    </div>
  );
}

export function TodoCard({ items }: { items: TodoEntry[] }) {
  const done = items.filter((item) => item.status === 'completed').length;
  return (
    <section className="todo-card" aria-label={`Цели: ${done} из ${items.length}`}>
      <ul>
        {items.map((item) => (
          <li key={item.key} className={`todo-${item.status}`}>
            {item.status === 'completed' ? (
              <CircleCheck size={17} className="todo-icon" />
            ) : item.status === 'in_progress' ? (
              <Circle size={17} className="todo-icon" />
            ) : (
              <CircleDashed size={17} className="todo-icon" />
            )}
            <span>{item.content}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
