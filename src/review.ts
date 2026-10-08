import type { ReviewDocument } from './contracts';

export interface ReviewBlock {
  id: string;
  text: string;
  heading?: boolean;
}
export interface ReviewSheet {
  id: string;
  name: string;
  rows: string[][];
}
export interface ParsedReview {
  blocks: ReviewBlock[];
  sheets: ReviewSheet[];
  truncated: boolean;
}
const elements = (node: Document | Element, name: string) =>
  Array.from(node.getElementsByTagNameNS('*', name));
function xml(text: string): Document {
  if (/<!DOCTYPE|<!ENTITY/i.test(text)) throw new Error('Небезопасный XML документа');
  const document = new DOMParser().parseFromString(text, 'application/xml');
  if (document.getElementsByTagName('parsererror').length)
    throw new Error('Повреждённый XML документа');
  return document;
}
function wordText(node: Element): string {
  return Array.from(node.getElementsByTagName('*'))
    .map((n) =>
      n.localName === 't'
        ? (n.textContent ?? '')
        : n.localName === 'tab'
          ? '\t'
          : n.localName === 'br'
            ? '\n'
            : '',
    )
    .join('');
}
export function parseDelimited(
  text: string,
  delimiter: string,
): { rows: string[][]; truncated: boolean } {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let quoted = false;
  let cells = 0;
  for (let i = 0; i <= text.length; i++) {
    const c = text[i];
    if (c === '"' && (quoted || cell === '')) {
      if (quoted && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else quoted = !quoted;
    } else if (!quoted && (c === delimiter || c === '\n' || c === '\r' || c === undefined)) {
      if (row.length >= 100 || ++cells > 20000) return { rows: [...rows, row], truncated: true };
      row.push(cell);
      cell = '';
      if (c !== delimiter) {
        rows.push(row);
        row = [];
        if (rows.length >= 1000) return { rows, truncated: i < text.length };
        if (c === '\r' && text[i + 1] === '\n') i++;
        if (i === text.length - 1) break;
      }
    } else if (c !== undefined) cell += c;
  }
  if (quoted) throw new Error('Незакрытая кавычка в CSV');
  return { rows, truncated: false };
}
export function columnName(index: number): string {
  let result = '';
  for (let n = index + 1; n; n = Math.floor((n - 1) / 26))
    result = String.fromCharCode(65 + ((n - 1) % 26)) + result;
  return result;
}
export function parseReview(input: ReviewDocument): ParsedReview {
  const result: ParsedReview = { blocks: [], sheets: [], truncated: false };
  const part = (name: string) => {
    const p = input.parts.find((p) => p.name === name);
    if (!p) throw new Error(`Отсутствует часть документа: ${name}`);
    return xml(p.content);
  };
  if (input.kind === 'docx') {
    const body = elements(part('word/document.xml'), 'body')[0];
    if (!body) throw new Error('Нет текста документа');
    for (const [i, child] of Array.from(body.children).entries()) {
      if (i >= 3000) {
        result.truncated = true;
        break;
      }
      if (child.localName === 'p')
        result.blocks.push({
          id: `p:${i}`,
          text: wordText(child),
          heading: elements(child, 'pStyle').some((s) =>
            /heading|title/i.test(s.getAttributeNS(s.namespaceURI, 'val') ?? ''),
          ),
        });
      if (child.localName === 'tbl') {
        let count = 0;
        const rows = elements(child, 'tr')
          .slice(0, 1000)
          .map((tr) =>
            elements(tr, 'tc')
              .slice(0, 100)
              .map((tc) => {
                if (++count > 20000) {
                  result.truncated = true;
                  return '';
                }
                return wordText(tc);
              }),
          );
        result.sheets.push({ id: `table:${i}`, name: `Таблица ${result.sheets.length + 1}`, rows });
      }
    }
  } else if (input.kind === 'pptx') {
    const presentation = part('ppt/presentation.xml');
    const relationships = elements(part('ppt/_rels/presentation.xml.rels'), 'Relationship');
    for (const [index, slide] of elements(presentation, 'sldId').slice(0, 64).entries()) {
      const rid = Array.from(slide.attributes).find(
        (a) => a.localName === 'id' && a.namespaceURI?.includes('relationships'),
      )?.value;
      const rel = relationships.find((r) => r.getAttribute('Id') === rid);
      const target = rel?.getAttribute('Target') ?? '';
      if (
        rel?.getAttribute('TargetMode') === 'External' ||
        !/^(?:\/ppt\/|(?:\.\/)?)(?:slides\/slide\d+\.xml)$/.test(target)
      )
        throw new Error('Внешние или нестандартные ссылки на слайды недоступны');
      const name = target.startsWith('/ppt/')
        ? target.slice(1)
        : `ppt/${target.replace(/^\.\//, '')}`;
      result.blocks.push({ id: `slide:${index}`, heading: true, text: `Слайд ${index + 1}` });
      const paragraphs = elements(part(name), 'p');
      if (paragraphs.length > 200) result.truncated = true;
      for (const [paragraphIndex, paragraph] of paragraphs.slice(0, 200).entries()) {
        result.blocks.push({
          id: `slide:${index}:p:${paragraphIndex}`,
          text: elements(paragraph, 't')
            .map((node) => node.textContent ?? '')
            .join(''),
        });
      }
    }
    result.truncated ||= elements(presentation, 'sldId').length > 64;
  } else if (input.kind === 'xlsx') {
    const workbook = part('xl/workbook.xml');
    const rels = part('xl/_rels/workbook.xml.rels');
    const shared = input.parts.some((p) => p.name === 'xl/sharedStrings.xml')
      ? elements(part('xl/sharedStrings.xml'), 'si').map((s) =>
          elements(s, 't')
            .map((t) => t.textContent ?? '')
            .join(''),
        )
      : [];
    for (const sheet of elements(workbook, 'sheet').slice(0, 64)) {
      const rid = Array.from(sheet.attributes).find((a) => a.localName === 'id')?.value;
      const rel = elements(rels, 'Relationship').find((r) => r.getAttribute('Id') === rid);
      const target = rel?.getAttribute('Target') ?? '';
      if (
        rel?.getAttribute('TargetMode') === 'External' ||
        !/^(?:\/xl\/|(?:\.\/)?)(?:worksheets\/sheet\d+\.xml)$/.test(target)
      )
        throw new Error('Внешние или нестандартные ссылки на листы не поддерживаются');
      const name = target.startsWith('/xl/')
        ? target.slice(1)
        : `xl/${target.replace(/^\.\//, '')}`;
      const rows: string[][] = [];
      let count = 0;
      for (const c of elements(part(name), 'c')) {
        const ref = /^([A-Z]{1,3})([1-9]\d{0,6})$/.exec(c.getAttribute('r') ?? '');
        if (!ref) continue;
        const col = Array.from(ref[1]!).reduce((n, c) => n * 26 + c.charCodeAt(0) - 64, 0) - 1;
        const row = Number(ref[2]) - 1;
        if (row >= 1000 || col >= 100 || ++count > 20000) {
          result.truncated = true;
          continue;
        }
        let value = elements(c, 'v')[0]?.textContent ?? '';
        if (c.getAttribute('t') === 's') value = shared[Number(value)] ?? '';
        else if (c.getAttribute('t') === 'inlineStr')
          value = elements(c, 't')
            .map((t) => t.textContent ?? '')
            .join('');
        else if (!value && elements(c, 'f').length)
          value = `=${elements(c, 'f')[0]?.textContent ?? ''}`;
        while (rows.length <= row) rows.push([]);
        const values = rows[row]!;
        while (values.length <= col) values.push('');
        values[col] = value;
      }
      result.sheets.push({ id: rid ?? name, name: sheet.getAttribute('name') ?? 'Лист', rows });
    }
  } else if (['csv', 'tsv'].includes(input.kind)) {
    const text = input.text ?? '';
    let delimiter = input.kind === 'tsv' ? '\t' : ',';
    if (input.kind === 'csv') {
      const counts = new Map([
        [',', 0],
        [';', 0],
        ['\t', 0],
      ]);
      let quoted = false;
      for (const c of text.split(/\r?\n/, 1)[0] ?? '') {
        if (c === '"') quoted = !quoted;
        else if (!quoted && counts.has(c)) counts.set(c, counts.get(c)! + 1);
      }
      delimiter = [...counts].sort((a, b) => b[1] - a[1])[0]![0];
    }
    const parsed = parseDelimited(text, delimiter);
    result.truncated = parsed.truncated;
    result.sheets.push({ id: 'csv', name: 'Таблица', rows: parsed.rows });
  } else {
    const paragraphs = (input.text ?? '').split(/\n\s*\n/);
    result.truncated = paragraphs.length > 3000;
    result.blocks = paragraphs
      .slice(0, 3000)
      .map((text, i) => ({ id: `p:${i}`, text, heading: /^#{1,6} /.test(text) }));
  }
  for (const sheet of result.sheets) {
    const width = Math.max(1, ...sheet.rows.map((r) => r.length));
    const maxRows = Math.floor(20000 / width);
    if (sheet.rows.length > maxRows) {
      sheet.rows = sheet.rows.slice(0, maxRows);
      result.truncated = true;
    }
  }
  return result;
}
