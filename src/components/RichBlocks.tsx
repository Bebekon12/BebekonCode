import type { ReactNode } from 'react';

/**
 * Visual blocks an agent may put in a final answer as fenced JSON, for example
 * ```bebekon-chart {"title":"…","max":10,"items":[{"label":"Свет","value":4}]} ```.
 * Data is validated strictly; anything unexpected falls back to the plain code block.
 */
export const richBlockKinds = ['chart', 'metrics', 'palette', 'priorities'] as const;

type Item = Record<string, unknown>;
const text = (value: unknown, limit = 200) =>
  typeof value === 'string' && value.trim() ? value.trim().slice(0, limit) : undefined;
const number = (value: unknown) =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined;
const items = (data: Item, limit: number) =>
  Array.isArray(data.items)
    ? data.items.filter((item): item is Item => !!item && typeof item === 'object').slice(0, limit)
    : [];

export function richBlock(kind: string, source: string): ReactNode | null {
  let data: Item;
  try {
    const parsed: unknown = JSON.parse(source);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    data = parsed as Item;
  } catch {
    return null;
  }
  const title = text(data.title, 120);
  const note = text(data.note, 400);
  const body =
    kind === 'chart'
      ? chart(data)
      : kind === 'metrics'
        ? metrics(data)
        : kind === 'palette'
          ? palette(data)
          : kind === 'priorities'
            ? priorities(data)
            : null;
  if (!body) return null;
  return (
    <figure className={`rich-block rich-${kind}`}>
      {title && <figcaption>{title}</figcaption>}
      {body}
      {note && <p className="rich-note">{note}</p>}
    </figure>
  );
}

function chart(data: Item) {
  const bars = items(data, 16)
    .map((item) => ({ label: text(item.label, 40), value: number(item.value) }))
    .filter(
      (bar): bar is { label: string; value: number } => !!bar.label && bar.value !== undefined,
    );
  if (!bars.length) return null;
  const max = Math.max(number(data.max) ?? 0, ...bars.map((bar) => bar.value), 1);
  const unit = text(data.unit, 12) ?? '';
  return (
    <div className="rich-bars" role="list">
      {bars.map((bar) => (
        <div
          className="rich-bar"
          role="listitem"
          key={bar.label}
          title={`${bar.label}: ${bar.value}${unit}`}
        >
          <span className="rich-bar-value">
            {bar.value}
            {unit}
          </span>
          <span className="rich-bar-track">
            <span style={{ height: `${Math.max(2, (Math.max(0, bar.value) / max) * 100)}%` }} />
          </span>
          <span className="rich-bar-label">{bar.label}</span>
        </div>
      ))}
    </div>
  );
}

function metrics(data: Item) {
  const cards = items(data, 6)
    .map((item) => ({
      label: text(item.label, 60),
      value:
        text(item.value, 24) ?? (number(item.value) !== undefined ? String(item.value) : undefined),
      caption: text(item.caption, 120),
      tone: text(item.tone, 10),
    }))
    .filter((card) => card.label && card.value);
  if (!cards.length) return null;
  return (
    <div className="rich-metrics">
      {cards.map((card) => (
        <div
          key={card.label}
          className={`rich-metric ${card.tone === 'good' || card.tone === 'bad' ? card.tone : ''}`}
        >
          <small>{card.label}</small>
          <strong>{card.value}</strong>
          {card.caption && <span>{card.caption}</span>}
        </div>
      ))}
    </div>
  );
}

function palette(data: Item) {
  const colors = (Array.isArray(data.colors) ? data.colors : [])
    .filter((item): item is Item => !!item && typeof item === 'object')
    .slice(0, 10)
    .map((item) => ({ name: text(item.name, 30), hex: text(item.hex, 7) }))
    // Only plain #RRGGBB values reach inline styles.
    .filter(
      (color): color is { name: string; hex: string } =>
        !!color.name && !!color.hex && /^#[0-9a-f]{6}$/i.test(color.hex),
    );
  if (!colors.length) return null;
  return (
    <div className="rich-palette">
      {colors.map((color) => (
        <div key={`${color.name}${color.hex}`}>
          <span style={{ background: color.hex }} />
          <strong>{color.name}</strong>
          <code>{color.hex.toUpperCase()}</code>
        </div>
      ))}
    </div>
  );
}

const levels: Record<string, string> = {
  critical: 'Критично',
  high: 'Высокий',
  medium: 'Средний',
  low: 'Низкий',
};

function priorities(data: Item) {
  const rows = items(data, 12)
    .map((item) => ({
      title: text(item.title, 100),
      text: text(item.text, 600),
      result: text(item.result, 200),
      level: text(item.level, 10),
    }))
    .filter((row) => row.title);
  if (!rows.length) return null;
  return (
    <ol className="rich-priorities">
      {rows.map((row, index) => (
        <li key={`${index}-${row.title}`}>
          <div className="row-between">
            <strong>{row.title}</strong>
            {row.level && levels[row.level] && (
              <span className={`rich-level ${row.level}`}>{levels[row.level]}</span>
            )}
          </div>
          {row.text && <p>{row.text}</p>}
          {row.result && <small>Результат: {row.result}</small>}
        </li>
      ))}
    </ol>
  );
}
