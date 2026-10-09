import type { AccountProfile, SessionStatus } from './contracts';

// Presentation only: keep persisted titles, profile IDs and protocol status values unchanged.
export const statusLabels: Record<SessionStatus, string> = {
  idle: 'Ожидает задачи',
  running: 'Выполняется',
  completed: 'Завершено',
  stopped: 'Остановлено',
  failed: 'Ошибка',
  interrupted: 'Прервано',
};
export const sessionTitle = (title: string) => (title === 'New session' ? 'Новый чат' : title);
export function accountLabel(account?: AccountProfile): string {
  if (!account) return '—';
  return account.id === 'mock-local' && account.label === 'Local demo'
    ? 'Локальное демо'
    : account.label;
}
export const formatDate = (seconds: number) => new Date(seconds * 1000).toLocaleString('ru-RU');
export const clock = (seconds: number) =>
  new Date(seconds * 1000).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
export const formatStart = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString('ru-RU', {
    hour: '2-digit',
    minute: '2-digit',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
export function duration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  if (total < 60) return `${total} с`;
  const minutes = Math.floor(total / 60);
  if (minutes < 60) return `${minutes} мин`;
  const hours = Math.floor(minutes / 60);
  return minutes % 60 ? `${hours} ч ${minutes % 60} мин` : `${hours} ч`;
}
const permissionLabels: Record<string, string> = {
  read_only: 'Только чтение',
  workspace_auto: 'Авто в проекте',
  full_access: 'Полный доступ',
};
export const permissionLabel = (profile: string) =>
  permissionLabels[profile] ?? 'По правилам провайдера';
export function counted(value: number, forms: readonly [string, string, string]): string {
  const lastTwo = Math.abs(value) % 100;
  const last = lastTwo % 10;
  const form =
    lastTwo >= 11 && lastTwo <= 14
      ? forms[2]
      : last === 1
        ? forms[0]
        : last >= 2 && last <= 4
          ? forms[1]
          : forms[2];
  return `${value} ${form}`;
}
// Display only: the core keeps canonical verbatim paths (\\?\C:\...) for boundary checks.
export function displayPath(path: string): string {
  if (path.startsWith('\\\\?\\UNC\\')) return `\\\\${path.slice(8)}`;
  return path.startsWith('\\\\?\\') ? path.slice(4) : path;
}
export const errorText = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

export function demoActivity(text: string): string {
  const legacy: Record<string, string> = {
    'Simulated planning step': 'Демонстрация планирования',
    'Demo activity only. No commands were executed and no project files were read.':
      'Демонстрация действия. Команды не выполнялись, файлы проекта не читались.',
    'No files were read or commands executed. This is a browser preview.':
      'Файлы не читались, команды не выполнялись. Это предпросмотр для разработки.',
    'The turn failed. Local storage or provider runtime is unavailable.':
      'Не удалось завершить ответ. Локальное хранилище или процесс провайдера недоступен.',
  };
  return legacy[text] ?? text;
}

export function demoResponse(text: string): string {
  const prefix = 'This is a local demo response to: “';
  const suffix =
    '”.\n\nThe workspace is ready for independent agent sessions. Each session keeps its provider, account and permission profile, and its timeline is stored locally in SQLite.\n\nThis simulator demonstrates streaming and cancellation. It does not inspect your repository, execute tools or modify files. Connect an official provider adapter in a later milestone to perform real coding tasks.';
  if (!text.startsWith(prefix) || !text.endsWith(suffix)) return text;
  const subject = text.slice(prefix.length, -suffix.length);
  return `Это ответ локального демо на задачу: «${subject}».\n\nРабочая область поддерживает независимые сессии. Каждая сессия привязана к своему провайдеру, аккаунту и профилю разрешений, а её история хранится локально в SQLite.\n\nСимулятор демонстрирует потоковый вывод и остановку. Он не читает репозиторий, не запускает инструменты и не меняет файлы. Для настоящих задач программирования потребуется официальный адаптер провайдера.`;
}
