import { describe, expect, it } from 'vitest';
import { richBlock } from './components/RichBlocks';

describe('rich answer blocks', () => {
  it('renders valid chart data', () => {
    const block = richBlock(
      'chart',
      JSON.stringify({ title: 'Оценки', max: 10, items: [{ label: 'Свет', value: 4 }] }),
    );
    expect(block).not.toBeNull();
  });

  it('falls back to a code block for broken or unknown data', () => {
    expect(richBlock('chart', '{"items": [')).toBeNull();
    expect(richBlock('chart', JSON.stringify({ items: [{ label: 'Без значения' }] }))).toBeNull();
    expect(richBlock('unknown', JSON.stringify({ items: [{ label: 'a', value: 1 }] }))).toBeNull();
  });

  it('only accepts plain #RRGGBB colours', () => {
    const unsafe = JSON.stringify({
      colors: [{ name: 'Опасный', hex: 'red;background:url(x)' }],
    });
    expect(richBlock('palette', unsafe)).toBeNull();
    const safe = JSON.stringify({ colors: [{ name: 'Ночь', hex: '#101923' }] });
    expect(richBlock('palette', safe)).not.toBeNull();
  });
});
