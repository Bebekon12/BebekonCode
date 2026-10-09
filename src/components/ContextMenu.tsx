import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Copy, TextSelect } from 'lucide-react';

export interface ContextAction {
  label: string;
  icon: ReactNode;
  danger?: boolean;
  run: () => void;
}

interface MenuState {
  x: number;
  y: number;
  actions: ContextAction[];
}

/**
 * Replaces the browser context menu (reload, inspect) with app actions. Text fields keep the
 * native menu for paste and spelling. Elements may add their own actions via `extraActions`.
 */
export function ContextMenu({
  extraActions,
}: {
  /** Actions for the element under the pointer, such as deleting a chat in the sidebar. */
  extraActions: (target: HTMLElement) => ContextAction[];
}) {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const extra = useRef(extraActions);
  extra.current = extraActions;
  useEffect(() => {
    const open = (event: MouseEvent) => {
      const target = event.target instanceof HTMLElement ? event.target : null;
      if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
      event.preventDefault();
      const selection = window.getSelection()?.toString() ?? '';
      const actions: ContextAction[] = [];
      if (selection.trim())
        actions.push({
          label: 'Копировать',
          icon: <Copy size={15} />,
          run: () => void navigator.clipboard.writeText(selection).catch(() => undefined),
        });
      const message = target?.closest<HTMLElement>('.prose');
      if (message && !selection.trim())
        actions.push({
          label: 'Выделить сообщение',
          icon: <TextSelect size={15} />,
          run: () => window.getSelection()?.selectAllChildren(message),
        });
      if (target) actions.push(...extra.current(target));
      setMenu(actions.length ? { x: event.clientX, y: event.clientY, actions } : null);
    };
    const close = () => setMenu(null);
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    const outside = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) close();
    };
    document.addEventListener('contextmenu', open);
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', key);
    window.addEventListener('blur', close);
    window.addEventListener('resize', close);
    document.addEventListener('scroll', close, true);
    return () => {
      document.removeEventListener('contextmenu', open);
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', key);
      window.removeEventListener('blur', close);
      window.removeEventListener('resize', close);
      document.removeEventListener('scroll', close, true);
    };
  }, []);
  useEffect(() => {
    ref.current?.querySelector('button')?.focus();
  }, [menu]);
  if (!menu) return null;
  // Keep the menu inside the window near the edges.
  const left = Math.min(menu.x, window.innerWidth - 230);
  const top = Math.min(menu.y, window.innerHeight - 12 - menu.actions.length * 38);
  return (
    <div className="context-menu" role="menu" ref={ref} style={{ left, top }}>
      {menu.actions.map((action) => (
        <button
          key={action.label}
          role="menuitem"
          className={action.danger ? 'danger' : undefined}
          onClick={() => {
            setMenu(null);
            action.run();
          }}
        >
          {action.icon}
          {action.label}
        </button>
      ))}
    </div>
  );
}
