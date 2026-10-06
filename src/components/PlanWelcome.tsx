import { ArrowUpRight } from 'lucide-react';
import { Dialog } from './Dialog';

/** Shown once per account after the first sign-in with ChatGPT plan usage enabled. */
export function PlanWelcome({
  close,
  manageUsage,
}: {
  close: () => void;
  manageUsage: () => void;
}) {
  return (
    <Dialog title="Вы используете свой план ChatGPT" close={close}>
      <p className="dialog-description">
        Запросы Codex в BebekonCode расходуют ваш план ChatGPT или баланс кредитов. Расход и
        ограничения для этого приложения можно посмотреть и изменить в настройках ChatGPT.
      </p>
      <div className="dialog-footer">
        <button className="secondary-button" onClick={manageUsage}>
          Управлять использованием <ArrowUpRight size={14} />
        </button>
        <button className="primary-button" autoFocus onClick={close}>
          Понятно
        </button>
      </div>
    </Dialog>
  );
}
