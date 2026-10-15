import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { Check, ChevronDown, Search, Sparkles, RotateCcw, Zap } from 'lucide-react';
import type { ModelInfo } from '../contracts';
import { effortLabels } from '../chat';
import { FastModeToggle } from './FastModeToggle';

export function ReasoningSlider({
  model,
  value,
  change,
  disabled = false,
  provider,
  fastMode = false,
  changeFastMode,
}: {
  model?: ModelInfo;
  value: string | null;
  change: (value: string | null) => void;
  disabled?: boolean;
  provider?: string;
  fastMode?: boolean;
  changeFastMode?: (enabled: boolean) => void;
}) {
  const levels = ['', ...(model?.reasoning_efforts ?? [])];
  const index = Math.max(0, levels.indexOf(value ?? ''));
  return (
    <div className="reasoning-slider">
      <div>
        {provider && changeFastMode ? (
          <FastModeToggle
            provider={provider}
            available={!!model?.fast_mode_available}
            enabled={fastMode}
            disabled={disabled}
            change={changeFastMode}
          />
        ) : (
          <Zap size={18} className="effort-icon" aria-hidden="true" />
        )}
        <strong>
          {levels.length === 1
            ? 'Недоступно'
            : index
              ? (effortLabels[levels[index] ?? ''] ?? levels[index])
              : model?.default_reasoning_effort
                ? `${effortLabels[model.default_reasoning_effort] ?? model.default_reasoning_effort} · авто`
                : 'Авто'}
        </strong>
        <button
          type="button"
          className="icon-button effort-reset"
          aria-label="Сбросить уровень обдумывания"
          disabled={disabled || levels.length === 1 || value === null}
          onClick={() => change(null)}
        >
          <RotateCcw size={16} />
        </button>
      </div>
      <p
        className="effort-model"
        title="Запрошенный уровень. Провайдер может ограничить его настройками аккаунта."
      >
        {model?.name ?? 'Выберите модель'}
      </p>
      <div className="effort-track">
        <input
          type="range"
          aria-label="Уровень обдумывания"
          min={0}
          max={Math.max(1, levels.length - 1)}
          step={1}
          value={index}
          aria-valuetext={
            index ? (effortLabels[levels[index] ?? ''] ?? levels[index]) : 'По умолчанию'
          }
          style={
            {
              '--range-progress': `${levels.length > 1 ? (index / (levels.length - 1)) * 100 : 0}%`,
            } as CSSProperties
          }
          disabled={disabled || levels.length === 1}
          onChange={(e) => change(levels[Number(e.target.value)] || null)}
        />
        <div className="effort-dots" aria-hidden="true">
          {levels.map((level) => (
            <i key={level} />
          ))}
        </div>
      </div>
      <div className="slider-caption">
        <span>Быстрее</span>
        <span>Глубже</span>
      </div>
    </div>
  );
}

export function ModelPicker({
  models,
  model,
  effort,
  disabled,
  change,
  label = 'Модель в чате',
  provider,
  fastMode = false,
  changeFastMode,
}: {
  models: ModelInfo[];
  model: string;
  effort: string | null;
  disabled: boolean;
  change: (model: string, effort: string | null) => void;
  label?: string;
  provider?: string;
  fastMode?: boolean;
  changeFastMode?: (enabled: boolean) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const ref = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const active = models.find((m) => m.id === model);
  useEffect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  return (
    <div
      className="model-picker"
      ref={ref}
      onKeyDown={(e) => {
        if (e.key === 'Escape' && open) {
          e.stopPropagation();
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <button
        ref={trigger}
        className="model-trigger"
        aria-label={label}
        aria-expanded={open}
        disabled={disabled || !models.length}
        onClick={() => {
          setQuery('');
          setOpen(!open);
        }}
      >
        {fastMode ? <Zap size={15} className="fast-mode-mark" /> : <Sparkles size={15} />}
        <span>{model === 'mock-stream-v1' ? 'Локальное демо' : (active?.name ?? model)}</span>
        {active?.reasoning_efforts?.length ? (
          <small className="trigger-effort">
            {effort ? (effortLabels[effort] ?? effort) : 'Авто'}
          </small>
        ) : null}
        <ChevronDown size={13} />
      </button>
      {open && (
        <div className="model-popover" aria-label="Выбор модели и обдумывания">
          <label className="model-search">
            <Search size={15} />
            <input
              autoFocus
              aria-label="Поиск модели"
              placeholder="Найти модель…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
          <div className="model-options">
            {models
              .filter((m) => `${m.name} ${m.id}`.toLowerCase().includes(query.toLowerCase()))
              .map((m) => (
                <button
                  key={m.id}
                  className={m.id === model ? 'selected' : ''}
                  aria-pressed={m.id === model}
                  onClick={() => change(m.id, m.id === model ? effort : null)}
                >
                  <span>
                    <strong>{m.name}</strong>
                    <code className="model-id">{m.id}</code>
                    {m.description && <small>{m.description}</small>}
                  </span>
                  {m.id === model && <Check size={16} />}
                </button>
              ))}
          </div>
          {!models.some((m) => `${m.name} ${m.id}`.toLowerCase().includes(query.toLowerCase())) && (
            <p className="muted model-empty">Моделей по этому запросу нет.</p>
          )}
          <ReasoningSlider
            model={active}
            provider={provider}
            fastMode={fastMode}
            changeFastMode={changeFastMode}
            value={effort}
            disabled={disabled}
            change={(e) => change(model, e)}
          />
          <button
            className="text-button model-done"
            onClick={() => {
              setOpen(false);
              trigger.current?.focus();
            }}
          >
            Готово
          </button>
        </div>
      )}
    </div>
  );
}
