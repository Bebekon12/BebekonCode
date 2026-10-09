import { useEffect, useId, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';

// Open dialogs, innermost last: only the top one reacts to Escape and Tab.
const openDialogs: symbol[] = [];

export function Dialog({
  title,
  close,
  children,
  wide = false,
  closeDisabled = false,
  workspace = false,
}: {
  title: string;
  close: () => void;
  children: ReactNode;
  wide?: boolean;
  closeDisabled?: boolean;
  workspace?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // Unique per dialog: stacked dialogs must not share a label.
  const titleId = useId();
  const closeRef = useRef(close);
  closeRef.current = close;
  const lockedRef = useRef(closeDisabled);
  lockedRef.current = closeDisabled;
  useEffect(() => {
    const token = Symbol('dialog');
    openDialogs.push(token);
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const element = ref.current;
    const focusable = () =>
      [
        ...(element?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input, select, textarea, a[href], [tabindex="0"]',
        ) ?? []),
      ].filter((item) => !item.hasAttribute('disabled') && item.getClientRects().length > 0);
    // Respect a field that already took focus via autoFocus (search, confirmation buttons).
    if (!element?.contains(document.activeElement)) {
      const items = focusable();
      (items.find((item) => !item.closest('.dialog-heading')) ?? items[0])?.focus();
    }
    const key = (event: KeyboardEvent) => {
      if (openDialogs.at(-1) !== token) return;
      if (event.key === 'Escape') {
        event.preventDefault();
        if (!lockedRef.current) closeRef.current();
      }
      if (event.key === 'Tab') {
        const items = focusable();
        const first = items[0];
        const last = items.at(-1);
        if (!element?.contains(document.activeElement)) {
          event.preventDefault();
          first?.focus();
        } else if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last?.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener('keydown', key);
    return () => {
      document.removeEventListener('keydown', key);
      openDialogs.splice(openDialogs.indexOf(token), 1);
      previous?.focus();
    };
  }, []);
  return (
    <div
      className="dialog-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !closeDisabled) close();
      }}
    >
      <div
        className={`dialog ${wide ? 'dialog-wide' : ''} ${workspace ? 'dialog-workspace' : ''}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        ref={ref}
      >
        <div className="dialog-heading">
          <h2 id={titleId}>{title}</h2>
          <button
            className="icon-button"
            aria-label="Закрыть окно"
            disabled={closeDisabled}
            onClick={close}
          >
            <X size={18} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
