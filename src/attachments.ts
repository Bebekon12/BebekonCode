import type { Attachment } from './contracts';

export const MAX_ATTACHMENTS = 8;
export const MAX_ATTACHMENT_BYTES = 20 * 1024 * 1024;
export const MAX_TOTAL_BYTES = 40 * 1024 * 1024;
export interface DraftAttachment extends Attachment {
  id: string;
  size: number;
}

export async function readAttachments(
  files: File[],
  existing: DraftAttachment[],
): Promise<DraftAttachment[]> {
  if (files.length + existing.length > MAX_ATTACHMENTS)
    throw new Error('Можно прикрепить до 8 файлов.');
  if (files.some((file) => file.size > MAX_ATTACHMENT_BYTES))
    throw new Error('Один файл может занимать до 20 МиБ.');
  if (
    files.reduce(
      (size, file) => size + file.size,
      existing.reduce((size, file) => size + file.size, 0),
    ) > MAX_TOTAL_BYTES
  )
    throw new Error('Общий размер вложений — до 40 МиБ.');
  const added = await Promise.all(
    files.map(async (file) => {
      const extension = file.name.split('.').pop()?.toLowerCase();
      const images: Record<string, string> = {
        png: 'image/png',
        jpg: 'image/jpeg',
        jpeg: 'image/jpeg',
        gif: 'image/gif',
        webp: 'image/webp',
      };
      const detected = images[extension ?? ''] ?? (file.type || 'application/octet-stream');
      const mime =
        detected.startsWith('image/') && !Object.values(images).includes(detected)
          ? 'application/octet-stream'
          : detected;
      if (mime.startsWith('image/') && file.size > 4 * 1024 * 1024)
        throw new Error('Изображение для ИИ должно занимать до 4 МиБ. Уменьшите его размер.');
      const data = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onerror = () => reject(new Error(`Не удалось прочитать ${file.name}.`));
        reader.onload = () => resolve(String(reader.result).split(',')[1] ?? '');
        reader.readAsDataURL(file);
      });
      return { id: crypto.randomUUID(), name: file.name, mime, data, size: file.size };
    }),
  );
  if (
    [...existing, ...added]
      .filter((file) => file.mime.startsWith('image/'))
      .reduce((size, file) => size + file.size, 0) >
    6 * 1024 * 1024
  )
    throw new Error('Общий размер изображений для ИИ — до 6 МиБ.');
  return added;
}

export function attachmentHint(file: Pick<Attachment, 'name' | 'mime'>): string {
  if (file.mime.startsWith('image/')) return 'Изображение для ИИ';
  if (/\.(docx|xlsx|pptx)$/i.test(file.name)) return 'Текст извлекается локально';
  if (file.mime.startsWith('video/') || /\.(mp4|mov|webm|avi|mkv)$/i.test(file.name))
    return 'Видео как файл · без анализа кадров';
  if (file.mime.startsWith('audio/')) return 'Аудио как файл · без расшифровки';
  if (/\.(txt|md|csv|tsv|json|xml|html|css|js|ts|py|rs|log|yaml|yml)$/i.test(file.name))
    return 'Текстовый файл';
  return 'Файл · чтение зависит от инструментов ИИ';
}

export function attachmentSize(size: number): string {
  return size < 1024 * 1024
    ? `${Math.max(1, Math.round(size / 1024))} КБ`
    : `${(size / 1024 / 1024).toFixed(1)} МБ`;
}
