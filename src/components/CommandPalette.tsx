import { useState, type ReactNode } from 'react';
import { ChevronRight, Command, Search, Sparkles } from 'lucide-react';
import type { Session } from '../contracts';
import { sessionTitle } from '../locale';
import { Dialog } from './Dialog';

export interface PaletteCommand {
  label: string;
  icon: ReactNode;
  action: () => void;
}
export function CommandPalette({
  commands,
  sessions,
  close,
  run,
  select,
}: {
  commands: PaletteCommand[];
  sessions: Session[];
  close: () => void;
  run: (command: () => void) => void;
  select: (session: Session) => void;
}) {
  const [query, setQuery] = useState('');
  const all = [
    ...commands,
    ...sessions.map((session) => ({
      label: `Перейти к сессии: ${sessionTitle(session.title)}`,
      icon: <Sparkles size={16} />,
      action: () => select(session),
    })),
  ];
  const matches = all.filter((command) => fuzzyMatch(query, command.label));
  return (
    <Dialog title="Команды" close={close}>
      <div className="palette-search">
        <Search size={18} />
        <input
          autoFocus
          aria-label="Поиск команд"
          placeholder="Найти команду или сессию…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && matches[0]) run(matches[0].action);
          }}
        />
        <Command size={15} />
      </div>
      <div className="palette-list">
        {matches.map((command, index) => (
          <button key={`${command.label}-${index}`} onClick={() => run(command.action)}>
            {command.icon}
            <span>{command.label}</span>
            <ChevronRight size={14} />
          </button>
        ))}
        {!matches.length && <p className="muted">Подходящих команд нет.</p>}
      </div>
    </Dialog>
  );
}
function fuzzyMatch(query: string, text: string): boolean {
  let position = 0;
  const haystack = text.toLowerCase();
  for (const character of query.trim().toLowerCase()) {
    position = haystack.indexOf(character, position);
    if (position < 0) return false;
    position++;
  }
  return true;
}
