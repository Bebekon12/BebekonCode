import { Layers3, Plus, X } from 'lucide-react';

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
      <div className="hero-planet" aria-hidden="true" />
      <button className="icon-button hero-hide" aria-label="Скрыть приветствие" onClick={hide}>
        <X size={14} />
      </button>
      <div className="hero-copy">
        <div className="eyebrow">ДОБРО ПОЖАЛОВАТЬ В BEBEKONCODE</div>
        <h1>
          Воплощайте идеи в код
          <br />
          <span>вместе с AI-агентами</span>
        </h1>
        <p>Создавайте, исследуйте, тестируйте — всё в одном пространстве.</p>
        <div className="welcome-actions">
          <button className="primary-button" onClick={start} disabled={disabled}>
            <Plus size={17} /> Новый чат
          </button>
          <button className="secondary-button" onClick={providers} disabled={disabled}>
            <Layers3 size={16} /> Подключить аккаунт
          </button>
        </div>
      </div>
      <div className="hero-quote" aria-hidden="true">
        <span>”</span>
        <p>
          Хороший код
          <br />
          начинается с хорошей идеи.
        </p>
        <small>BebekonCode</small>
      </div>
    </section>
  );
}
