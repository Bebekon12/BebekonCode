import { useState } from 'react';
import { ArrowUpRight, Check, Download, RefreshCw, ShieldCheck } from 'lucide-react';
import { Dialog } from './Dialog';
import type {
  ClientTransport,
  ProviderInfo,
  ReleaseCheck,
  Settings as AppSettings,
} from '../contracts';
import product from '../../product.json';
import pkg from '../../package.json';
export function Settings({
  client,
  settings,
  providers,
  updateSettings,
  updateProviders,
  release,
  updateRelease,
  close,
}: {
  client: ClientTransport;
  settings: AppSettings;
  providers: ProviderInfo[];
  updateSettings: (value: AppSettings) => void;
  updateProviders: (value: ProviderInfo[]) => void;
  release: ReleaseCheck | null;
  updateRelease: (value: ReleaseCheck) => void;
  close: () => void;
}) {
  const [tab, setTab] = useState('General');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title="Settings" close={close} wide>
      <div className="settings-layout">
        <nav aria-label="Settings categories">
          {['General', 'Providers', 'Permissions', 'Capabilities', 'About & updates'].map(
            (category) => (
              <button
                key={category}
                className={tab === category ? 'selected' : ''}
                onClick={() => setTab(category)}
              >
                {category}
              </button>
            ),
          )}
        </nav>
        <div className="settings-content">
          {tab === 'General' && (
            <>
              <h3>Your local workspace</h3>
              <p className="muted">Projects and session history stay on this computer.</p>
              <div className="setting-row">
                <div>
                  <strong>Appearance</strong>
                  <p>Graphite dark · system typography</p>
                </div>
                <span className="badge">Dark</span>
              </div>
              <div className="setting-row">
                <div>
                  <strong>Check for updates at startup</strong>
                  <p>Contact GitHub once at launch. Installation stays manual.</p>
                </div>
                <input
                  aria-label="Check for updates at startup"
                  type="checkbox"
                  checked={settings.check_updates_on_start}
                  disabled={busy}
                  onChange={(event) => {
                    const next = { check_updates_on_start: event.target.checked };
                    void run(async () => {
                      await client.saveSettings(next);
                      updateSettings(next);
                    });
                  }}
                />
              </div>
              <div className="setting-row">
                <div>
                  <strong>Remote access & telemetry</strong>
                  <p>Unavailable in this version. No listener or analytics.</p>
                </div>
                <ShieldCheck size={19} />
              </div>
            </>
          )}
          {tab === 'Providers' && (
            <>
              <div className="row-between">
                <h3>Agent engines</h3>
                <button
                  className="secondary-button"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => updateProviders(await client.refreshProviders()))
                  }
                >
                  <RefreshCw size={14} /> Refresh
                </button>
              </div>
              <p className="muted">Detection does not read or copy provider credentials.</p>
              {providers.map((provider) => (
                <div className="provider-card" key={provider.id}>
                  <div className="row-between">
                    <strong>{provider.name}</strong>
                    <span className={`badge ${provider.available ? 'success' : ''}`}>
                      {provider.available
                        ? 'Ready'
                        : provider.detected_path
                          ? 'Detected · not connected'
                          : 'Not integrated'}
                    </span>
                  </div>
                  <p>{provider.detail}</p>
                  {provider.detected_path && <code className="path">{provider.detected_path}</code>}
                </div>
              ))}
              <p className="small muted">
                Account login and multiple isolated accounts will be added with the official
                adapters. No credentials are needed for the local demo.
              </p>
            </>
          )}
          {tab === 'Permissions' && (
            <>
              <h3>Permission profiles</h3>
              <p className="muted">
                Provider safety remains enabled. The local demo does not run tools.
              </p>
              <div className="provider-card">
                <strong>Standard</strong>
                <p>
                  Workspace read/write and Git read allowed. External writes, shell execution,
                  network, delete, commit and push require approval.
                </p>
              </div>
              <div className="provider-card">
                <strong>Read only</strong>
                <p>Workspace read and Git read allowed. Writes and execution denied.</p>
              </div>
              <div className="notice">
                Credential access and system settings are denied by default. Interactive tool
                approvals will arrive with real provider adapters.
              </div>
            </>
          )}
          {tab === 'Capabilities' && (
            <>
              <h3>Provider capabilities</h3>
              <p className="muted">
                Bindings are reserved in the core schema for a later milestone.
              </p>
              <div className="provider-card">
                <strong>Cross-agent delegation</strong>
                <p>
                  Not integrated. Requires explicit provider/account binding and visible
                  cross-provider approval.
                </p>
              </div>
              <div className="provider-card">
                <strong>Image generation</strong>
                <p>
                  Image generation provider is not configured. A separate official API provider will
                  be required.
                </p>
              </div>
            </>
          )}
          {tab === 'About & updates' && (
            <>
              <div className="about-brand">
                <img src="/brand/snowman.png" alt="" />
                <div>
                  <h3>{product.name}</h3>
                  <p>Version {pkg.version} · Windows desktop</p>
                </div>
              </div>
              <p className="muted">{product.description}</p>
              <div className="notice">
                Early development release. Working local demo; OpenAI and Anthropic adapters are not
                connected yet.
              </div>
              <div className="update-actions">
                <button
                  className="primary-button"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => updateRelease(await client.checkReleases(true)))
                  }
                >
                  <RefreshCw size={15} className={busy ? 'spin' : ''} />{' '}
                  {busy ? 'Checking…' : 'Check for updates'}
                </button>
                <button
                  className="secondary-button"
                  onClick={() => void run(() => client.openReleases())}
                >
                  Release history <ArrowUpRight size={14} />
                </button>
              </div>
              {release && (
                <div className="release-result">
                  <div className="context-title">
                    {release.available ? <Download size={17} /> : <Check size={17} />}{' '}
                    {release.available
                      ? `Version ${release.latest_version} is available`
                      : release.latest_version
                        ? `You’re up to date · ${release.latest_version}`
                        : 'No stable release available'}
                  </div>
                  <p className="small muted">
                    Checked {new Date(release.checked_at * 1000).toLocaleString()}
                  </p>
                  <pre className="release-notes">
                    {release.notes || 'No release notes provided.'}
                  </pre>
                  {release.available && (
                    <button
                      className="secondary-button"
                      onClick={() => void run(() => client.openReleases())}
                    >
                      Open release and download <ArrowUpRight size={14} />
                    </button>
                  )}
                </div>
              )}
              <p className="small muted">
                Checks use the GitHub Releases API. No installer is downloaded or executed by the
                app. Signed Tauri automatic updates are planned.
              </p>
            </>
          )}
          {error && (
            <div role="alert" className="notice error">
              {error}
            </div>
          )}
        </div>
      </div>
    </Dialog>
  );
}
