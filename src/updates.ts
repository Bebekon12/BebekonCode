import { invoke } from '@tauri-apps/api/core';
import { check, type Update } from '@tauri-apps/plugin-updater';
import type { AppUpdate, UpdateProgress } from './contracts';

let pending: Update | null = null;
let busy = false;

export async function checkUpdate(): Promise<AppUpdate | null> {
  if (busy) throw new Error('Операция обновления уже выполняется.');
  busy = true;
  try {
    const previous = pending;
    pending = null;
    await previous?.close();
    const update = await check({ timeout: 30_000 });
    pending = update;
    return update
      ? {
          version: update.version,
          currentVersion: update.currentVersion,
          notes: update.body ?? '',
          date: update.date ?? null,
        }
      : null;
  } finally {
    busy = false;
  }
}

export async function installUpdate(
  version: string,
  onProgress: (progress: UpdateProgress) => void,
): Promise<void> {
  if (busy) throw new Error('Операция обновления уже выполняется.');
  const update = pending;
  if (!update || update.version !== version)
    throw new Error('Проверьте наличие обновления ещё раз.');
  busy = true;
  let downloaded = 0;
  let total: number | null = null;
  try {
    // The official download call verifies the signature before returning. Never install on failure.
    await update.download(
      (event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? null;
        if (event.event === 'Progress') downloaded += event.data.chunkLength;
        onProgress({
          downloaded,
          total,
          phase: event.event === 'Finished' ? 'verifying' : 'downloading',
        });
      },
      { timeout: 120_000 },
    );
    onProgress({ downloaded, total, phase: 'installing' });
    // Windows exits here. NSIS /UPDATE /R installs to the existing directory and restarts the app.
    await invoke('prepare_update');
    await update.install({ restartAfterInstall: true });
  } finally {
    pending = null;
    await update.close().catch(() => {});
    busy = false;
  }
}
