import { useEffect, useId, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';
export function Dialog({
  title,
  close,
  children,
  wide = false,
  closeDisabled = false,
}: {
  title: string;
  close: () => void;
  children: ReactNode;
  wide?: boolean;
  closeDisabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // Unique per dialog: stacked dialogs must not share a label.
  const titleId = useId();
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const element = ref.current;
    const focusable = () => [
      ...(element?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input, select, textarea, a[href], [tabindex="0"]',
      ) ?? []),
    ];
    focusable()[0]?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        closeRef.current();
      }
      if (event.key === 'Tab') {
        const items = focusable();
        const first = items[0];
        const last = items.at(-1);
        if (event.shiftKey && document.activeElement === first) {
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
      previous?.focus();
    };
  }, []);
  return (
    <div
      className="dialog-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <div
        className={`dialog ${wide ? 'dialog-wide' : ''}`}
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
