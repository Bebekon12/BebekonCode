import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Eye, ShieldAlert, ShieldCheck, ShieldQuestion } from 'lucide-react';

export const accessOptions = [
  {
    id: 'read_only',
    label: 'Только чтение',
    description: 'Изучать проект и отвечать. Изменять файлы нельзя.',
    Icon: Eye,
  },
  {
    id: 'standard',
    label: 'По правилам провайдера',
    description:
      'Claude спрашивает перед правками. Для Codex действует авторежим песочницы; отдельное подтверждение каждой правки пока недоступно.',
    Icon: ShieldQuestion,
  },
  {
    id: 'workspace_auto',
    label: 'Авто в проекте',
    description:
      'Правки в проекте автоматически. Codex также выполняет команды в песочнице. Дополнительный доступ требует подтверждения.',
    Icon: ShieldCheck,
  },
  {
    id: 'full_access',
    label: 'Полный доступ',
    description:
      'Без песочницы и без обычных подтверждений: файлы и команды на этом компьютере. Для Codex и Claude, включается для этого чата.',
    Icon: ShieldAlert,
  },
] as const;

/** Providers whose official CLI documents a full-access mode used by the core. */
export const fullAccessProviders = ['openai', 'anthropic', 'mock'];

export const fullAccessWarning =
  'Агент сможет без обычных подтверждений читать и изменять файлы, запускать команды и выходить в сеть от вашего имени. Ошибка агента может повредить данные вне проекта. Учётные данные запрещены; экран, мышь и клавиатура требуют подтверждения каждого действия. Обязательные ограничения провайдера сохраняются. Включайте только для задач, которым доверяете, и возвращайте обычный режим после работы.';

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
              <div className="access-popover-title">Что разрешено агенту?</div>
              {accessOptions.map(({ id, label, description, Icon }) => {
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
                        {full && !fullAvailable
                          ? 'Полный доступ недоступен для этого провайдера.'
                          : description}
                      </small>
                    </span>
                    {id === value && <Check size={17} />}
                  </button>
                );
              })}
              <p>
                {value === 'full_access'
                  ? 'Обычные правки и команды выполняются без подтверждений. Вызовы MCP и обязательные проверки провайдера могут требовать подтверждения.'
                  : 'Доступ за пределы песочницы требует отдельного подтверждения. В режиме чтения он запрещён.'}
              </p>
            </>
          )}
        </div>
      )}
    </div>
  );
}
