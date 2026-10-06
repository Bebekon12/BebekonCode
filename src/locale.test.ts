import { describe, expect, it } from 'vitest';
import { counted, displayPath, duration } from './locale';

describe('Russian formatting', () => {
  it('formats durations compactly', () => {
    expect(duration(4.4)).toBe('4 с');
    expect(duration(185)).toBe('3 мин');
    expect(duration(3900)).toBe('1 ч 5 мин');
    expect(duration(7200)).toBe('2 ч');
    expect(duration(-3)).toBe('0 с');
  });
  it('declines counters', () => {
    expect(counted(1, ['файл', 'файла', 'файлов'])).toBe('1 файл');
    expect(counted(3, ['файл', 'файла', 'файлов'])).toBe('3 файла');
    expect(counted(12, ['файл', 'файла', 'файлов'])).toBe('12 файлов');
  });
});

describe('path display', () => {
  it('hides Windows verbatim prefixes without touching normal paths', () => {
    expect(displayPath(String.raw`\\?\C:\Projects\app`)).toBe(String.raw`C:\Projects\app`);
    expect(displayPath(String.raw`\\?\UNC\server\share\app`)).toBe(String.raw`\\server\share\app`);
    expect(displayPath(String.raw`D:\Work`)).toBe(String.raw`D:\Work`);
  });
});
