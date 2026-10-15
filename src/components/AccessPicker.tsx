import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Eye, ShieldAlert, ShieldCheck, ShieldQuestion } from 'lucide-react';

export const accessOptions = [
  {
    id: 'plan',
    label: 'Plan · Claude',
    description: 'Только план, без правок',
    Icon: Eye,
  },
  {
    id: 'claude_auto',
    label: 'Auto · Claude',
    description: 'Claude сам оценивает риск команд',
    Icon: ShieldCheck,
  },
  {
    id: 'read_only',
    label: 'Только чтение',
    description: 'Без изменений файлов',
    Icon: Eye,
  },
  {
    id: 'standard',
    label: 'По правилам провайдера',
    description: 'Спрашивает перед правками и командами',
    Icon: ShieldQuestion,
  },
  {
    id: 'workspace_auto',
    label: 'Авто в проекте',
    description: 'Правки и команды в проекте без вопросов',
    Icon: ShieldCheck,
  },
  {
    id: 'full_access',
    label: 'Полный доступ',
    description: 'Без песочницы и подтверждений',
    Icon: ShieldAlert,
  },
] as const;

/** Providers whose official CLI documents a full-access mode used by the core. */
export const fullAccessProviders = ['openai', 'anthropic', 'mock'];

export const fullAccessWarning =
  'Агент будет менять файлы, запускать команды и выходить в сеть без вопросов. Ошибка может задеть данные вне проекта. Учётные данные остаются закрыты.';

export function AccessPicker({
  value,
  change,
  provider,
  disabled = false,
}: {
  value: string;
  change: (value: string) => void;
  provider: string;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [maxHeight, setMaxHeight] = useState<number>();
  const ref = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const active = accessOptions.find((option) => option.id === value) ?? accessOptions[1];
  const fullAvailable = fullAccessProviders.includes(provider);
  // The popover opens upward: fit it into the space above the trigger, minus its 12px gap.
  useLayoutEffect(() => {
    if (!open) return;
    const fit = () => {
      const top = trigger.current?.getBoundingClientRect().top ?? window.innerHeight;
      setMaxHeight(Math.max(160, Math.floor(top - 12 - 8)));
    };
    fit();
    window.addEventListener('resize', fit);
    return () => window.removeEventListener('resize', fit);
  }, [open]);
  useEffect(() => {
    if (!open) {
      setConfirming(false);
      return;
    }
    const outside = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  const choose = (id: string) => {
    change(id);
    setOpen(false);
    trigger.current?.focus();
  };
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
        className={`model-trigger ${value === 'full_access' ? 'access-danger' : ''}`}
        ref={trigger}
        type="button"
        aria-label="Доступ в чате"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen(!open)}
      >
        <active.Icon size={16} />
        <span>{active.label}</span>
        <ChevronDown size={13} />
      </button>
      {open && (
        <div className="access-popover" aria-label="Уровни доступа" style={{ maxHeight }}>
          {confirming ? (
            <div className="full-access-confirm" role="alertdialog" aria-label="Полный доступ">
              <strong>
                <ShieldAlert size={18} /> Включить полный доступ для этого чата?
              </strong>
              <p>{fullAccessWarning}</p>
              <div className="dialog-footer">
                <button
                  type="button"
                  className="secondary-button"
                  autoFocus
                  onClick={() => {
                    setConfirming(false);
                    trigger.current?.focus();
                  }}
                >
                  Отмена
                </button>
                <button
                  type="button"
                  className="primary-button danger-solid"
                  onClick={() => choose('full_access')}
                >
                  Включить полный доступ
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="access-popover-title">Доступ агента</div>
              {accessOptions
                .filter(
                  (option) =>
                    provider === 'anthropic' || !['plan', 'claude_auto'].includes(option.id),
                )
                .map(({ id, label, description, Icon }) => {
                  const full = id === 'full_access';
                  return (
                    <button
                      key={id}
                      type="button"
                      className={full ? 'access-danger' : undefined}
                      aria-pressed={id === value}
                      disabled={disabled || (full && !fullAvailable)}
                      onClick={() => (full && id !== value ? setConfirming(true) : choose(id))}
                    >
                      <Icon size={20} />
                      <span>
                        <strong>{label}</strong>
                        <small>
                          {full && !fullAvailable ? 'Недоступно для этого провайдера' : description}
                        </small>
                      </span>
                      {id === value && <Check size={17} />}
                    </button>
                  );
                })}
            </>
          )}
        </div>
      )}
    </div>
  );
}
