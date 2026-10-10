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
      ? 'Claude Fast оплачивается из дополнительных usage credits, даже если лимит подписки ещё не исчерпан. Аккаунту нужен доступ к этой функции.'
      : 'Fast расходует включённые лимиты ChatGPT быстрее: сейчас 2,5× обычного режима. Доступ и фактическую скорость определяет провайдер.';
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
            ? `Fast ${enabled ? 'включён (запрошено)' : 'выключен'}. ${cost}`
            : 'Скоростной режим недоступен для этой модели в текущей интеграции'
        }
        onClick={() => (enabled ? change(false) : setConfirm(true))}
      >
        <Zap size={18} />
      </button>
      {confirm && (
        <Dialog title="Включить скоростной режим" close={() => setConfirm(false)}>
          <p className="dialog-description">{cost}</p>
          <p className="muted small">
            Настройка относится только к выбранному агенту. Уровень обдумывания остаётся прежним.
            Фактическое ускорение не гарантируется.
          </p>
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
