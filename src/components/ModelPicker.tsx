import { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Search, Sparkles } from 'lucide-react';
import type { ModelInfo } from '../contracts';
import { effortLabels } from '../chat';

export function ReasoningSlider({
  model,
  value,
  change,
  disabled = false,
}: {
  model?: ModelInfo;
  value: string | null;
  change: (value: string | null) => void;
  disabled?: boolean;
}) {
  const levels = ['', ...(model?.reasoning_efforts ?? [])];
  const index = Math.max(0, levels.indexOf(value ?? ''));
  return (
    <div className="reasoning-slider">
      <div>
        <span>
          <Sparkles size={14} /> Обдумывание
        </span>
        <strong>
          {levels.length === 1
            ? 'Недоступно'
            : index
              ? (effortLabels[levels[index] ?? ''] ?? levels[index])
              : 'По умолчанию'}
        </strong>
      </div>
      <input
        type="range"
        aria-label="Уровень обдумывания"
        min={0}
        max={Math.max(1, levels.length - 1)}
        step={1}
        value={index}
        disabled={disabled || levels.length === 1}
        onChange={(e) => change(levels[Number(e.target.value)] || null)}
      />
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
}: {
  models: ModelInfo[];
  model: string;
  effort: string | null;
  disabled: boolean;
  change: (model: string, effort: string | null) => void;
  label?: string;
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
        <Sparkles size={15} />
        <span>{model === 'mock-stream-v1' ? 'Локальное демо' : (active?.name ?? model)}</span>
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
                    {m.description && <small>{m.description}</small>}
                  </span>
                  {m.id === model && <Check size={16} />}
                </button>
              ))}
          </div>
          <ReasoningSlider
            model={active}
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
