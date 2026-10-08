import { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Eye, ShieldCheck, ShieldQuestion } from 'lucide-react';

export const accessOptions = [
  {
    id: 'read_only',
    label: 'Только чтение',
    description: 'Изучать проект и отвечать. Изменять файлы нельзя.',
    Icon: Eye,
  },
  {
    id: 'standard',
    label: 'С подтверждением',
    description:
      'Claude спрашивает перед правками. Codex запрашивает дополнительные разрешения по своим правилам.',
    Icon: ShieldQuestion,
  },
  {
    id: 'workspace_auto',
    label: 'Автоправки в проекте',
    description:
      'Claude принимает правки файлов проекта автоматически. Codex сохраняет свои запросы разрешений.',
    Icon: ShieldCheck,
  },
] as const;

export function AccessPicker({
  value,
  change,
  disabled = false,
}: {
  value: string;
  change: (value: string) => void;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const active = accessOptions.find((option) => option.id === value) ?? accessOptions[1];
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  return (
    <div
      className="access-picker"
      ref={ref}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && open) {
          event.stopPropagation();
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <button
        className="model-trigger"
        ref={trigger}
        type="button"
        aria-label="Доступ в чате"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen(!open)}
      >
        <active.Icon size={16} />
        <span>{value === 'standard' ? 'С подтверждением' : active.label}</span>
        <ChevronDown size={13} />
      </button>
      {open && (
        <div className="access-popover" aria-label="Уровни доступа">
          <div className="access-popover-title">Что разрешено агенту?</div>
          {accessOptions.map(({ id, label, description, Icon }) => (
            <button
              key={id}
              type="button"
              aria-pressed={id === value}
              disabled={disabled}
              onClick={() => {
                change(id);
                setOpen(false);
                trigger.current?.focus();
              }}
            >
              <Icon size={20} />
              <span>
                <strong>{label}</strong>
                <small>{description}</small>
              </span>
              {id === value && <Check size={17} />}
            </button>
          ))}
          <p>Доступ за пределы проекта и отключение защит в этих режимах не предоставляются.</p>
        </div>
      )}
    </div>
  );
}
