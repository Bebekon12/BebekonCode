import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ check: vi.fn(), invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: mocks.check }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));

function packageUpdate() {
  return {
    version: '1.0.0',
    currentVersion: '0.3.2',
    body: 'Изменения',
    date: null,
    close: vi.fn().mockResolvedValue(undefined),
    install: vi.fn().mockResolvedValue(undefined),
    download: vi.fn().mockImplementation(async (callback) => {
      callback({ event: 'Started', data: { contentLength: 100 } });
      callback({ event: 'Progress', data: { chunkLength: 70 } });
      callback({ event: 'Progress', data: { chunkLength: 30 } });
      callback({ event: 'Finished' });
    }),
  };
}
beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  mocks.invoke.mockResolvedValue(undefined);
});
describe('signed updater lifecycle', () => {
  it('installs only after successful verified download and prepares the core before restarting', async () => {
    const update = packageUpdate();
    mocks.check.mockResolvedValue(update);
    const api = await import('./updates');
    const progress = vi.fn();
    await api.checkUpdate();
    await api.installUpdate('1.0.0', progress);
    expect(progress.mock.calls.at(-1)?.[0]).toEqual({
      downloaded: 100,
      total: 100,
      phase: 'installing',
    });
    expect(mocks.invoke).toHaveBeenCalledWith('prepare_update');
    expect(update.install).toHaveBeenCalledWith({ restartAfterInstall: true });
    expect(update.download.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.invoke.mock.invocationCallOrder[0]!,
    );
    expect(mocks.invoke.mock.invocationCallOrder[0]!).toBeLessThan(
      update.install.mock.invocationCallOrder[0]!,
    );
  });
  it('never prepares or installs on signature/download failure and requires a new check', async () => {
    const update = packageUpdate();
    update.download.mockRejectedValue(new Error('Signature mismatch'));
    mocks.check.mockResolvedValue(update);
    const api = await import('./updates');
    await api.checkUpdate();
    await expect(api.installUpdate('1.0.0', vi.fn())).rejects.toThrow('Signature mismatch');
    expect(update.install).not.toHaveBeenCalled();
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(update.close).toHaveBeenCalledOnce();
    await expect(api.installUpdate('1.0.0', vi.fn())).rejects.toThrow('Проверьте');
  });
  it('rejects a stale confirmation and releases replaced resources', async () => {
    const first = packageUpdate();
    const second = { ...packageUpdate(), version: '1.0.1' };
    mocks.check.mockResolvedValueOnce(first).mockResolvedValueOnce(second);
    const api = await import('./updates');
    await api.checkUpdate();
    await api.checkUpdate();
    expect(first.close).toHaveBeenCalledOnce();
    await expect(api.installUpdate('1.0.0', vi.fn())).rejects.toThrow('Проверьте');
    expect(second.download).not.toHaveBeenCalled();
  });
  it('blocks overlapping checks and installation while downloading', async () => {
    const update = packageUpdate();
    let finish!: () => void;
    update.download.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    mocks.check.mockResolvedValue(update);
    const api = await import('./updates');
    await api.checkUpdate();
    const installation = api.installUpdate('1.0.0', vi.fn());
    await expect(api.checkUpdate()).rejects.toThrow('уже выполняется');
    await expect(api.installUpdate('1.0.0', vi.fn())).rejects.toThrow('уже выполняется');
    finish();
    await installation;
    expect(update.install).toHaveBeenCalledOnce();
  });
  it('does not install if the core reports an active session after the download', async () => {
    const update = packageUpdate();
    mocks.check.mockResolvedValue(update);
    mocks.invoke.mockRejectedValue(new Error('Остановите активные сессии'));
    const api = await import('./updates');
    await api.checkUpdate();
    await expect(api.installUpdate('1.0.0', vi.fn())).rejects.toThrow('Остановите');
    expect(update.install).not.toHaveBeenCalled();
  });
});
