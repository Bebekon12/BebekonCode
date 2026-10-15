import { useState } from 'react';
import { Zap } from 'lucide-react';
import { Dialog } from './Dialog';

export function FastModeToggle({
  provider,
  available,
  enabled,
  disabled,
  change,
}: {
  provider: string;
  available: boolean;
  enabled: boolean;
  disabled: boolean;
  change: (enabled: boolean) => void;
}) {
  const [confirm, setConfirm] = useState(false);
  const cost =
    provider === 'anthropic'
      ? 'Оплачивается дополнительными usage credits, даже при остатке лимита подписки.'
      : 'Расходует лимит ChatGPT в 2,5 раза быстрее.';
  return (
    <>
      <button
        type="button"
        className={`icon-button fast-mode-toggle ${enabled ? 'active' : ''}`}
        aria-label="Скоростной режим"
        aria-pressed={enabled}
        disabled={disabled || (!available && !enabled)}
        title={
          available || enabled
            ? `Fast ${enabled ? 'включён' : 'выключен'}. ${cost}`
            : 'Fast недоступен для этой модели'
        }
        onClick={() => (enabled ? change(false) : setConfirm(true))}
      >
        <Zap size={18} />
      </button>
      {confirm && (
        <Dialog title="Включить скоростной режим" close={() => setConfirm(false)}>
          <p className="dialog-description">{cost}</p>
          <div className="dialog-footer">
            <button className="secondary-button" onClick={() => setConfirm(false)}>
              Отмена
            </button>
            <button
              className="primary-button"
              disabled={disabled || !available}
              onClick={() => {
                change(true);
                setConfirm(false);
              }}
            >
              Включить Fast
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
