import { Plus, X } from 'lucide-react';

export function WelcomeHero({
  start,
  providers,
  hide,
  disabled,
}: {
  start: () => void;
  providers: () => void;
  hide: () => void;
  disabled: boolean;
}) {
  return (
    <section className="welcome-hero" aria-label="Добро пожаловать">
      <button className="icon-button hero-hide" aria-label="Скрыть приветствие" onClick={hide}>
        <X size={14} />
      </button>
      <div className="hero-copy">
        <img className="welcome-mark" src="/brand/snowman.png" alt="" />
        <h1>С чего начнём?</h1>
        <p>Задайте вопрос, разберитесь в проекте или создайте что-то новое.</p>
        <div className="welcome-actions">
          <button className="primary-button" onClick={start} disabled={disabled}>
            <Plus size={20} /> Новый чат
          </button>
          <button className="secondary-button" onClick={providers} disabled={disabled}>
            Подключить аккаунт
          </button>
        </div>
      </div>
    </section>
  );
}
