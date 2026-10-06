import { Folder, Minus, PanelRightClose, Plus, Search, ShieldCheck, Square, X } from 'lucide-react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Workspace } from '../contracts';
import { displayPath } from '../locale';
import product from '../../product.json';

export function Titlebar({
  tabs,
  activeId,
  selectTab,
  closeTab,
  addProject,
  search,
  toggleContext,
  onError,
}: {
  tabs: Workspace[];
  activeId?: string;
  selectTab: (id: string) => void;
  closeTab: (id: string) => void;
  addProject?: () => void;
  search: () => void;
  toggleContext: () => void;
  onError: (error: unknown) => void;
}) {
  const windowAction = (action: 'minimize' | 'toggleMaximize' | 'close') => {
    if (isTauri()) void getCurrentWindow()[action]().catch(onError);
  };
  return (
    <header className="desktop-titlebar">
      <div className="titlebar-brand" data-tauri-drag-region>
        <img src="/brand/snowman.png" alt="" />
        <strong data-tauri-drag-region>{product.name}</strong>
      </div>
      <div className="project-tabs" role="tablist" aria-label="Открытые проекты">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className={`project-tab ${tab.id === activeId ? 'active' : ''}`}
            title={displayPath(tab.root)}
          >
            <button
              role="tab"
              aria-selected={tab.id === activeId}
              className="project-tab-open"
              onClick={() => selectTab(tab.id)}
            >
              <Folder size={14} />
              <span>{tab.name}</span>
            </button>
            <button
              className="project-tab-close"
              aria-label={`Закрыть вкладку ${tab.name}`}
              onClick={() => closeTab(tab.id)}
            >
              <X size={13} />
            </button>
          </div>
        ))}
        <button
          className="icon-button tab-add"
          aria-label="Открыть проект"
          disabled={!addProject}
          onClick={addProject}
        >
          <Plus size={15} />
        </button>
      </div>
      <div className="titlebar-space" data-tauri-drag-region />
      <button className="global-search" onClick={search}>
        <Search size={15} />
        <span>Поиск по проектам, чатам, командам…</span>
        <kbd>Ctrl + K</kbd>
      </button>
      <div className="titlebar-space" data-tauri-drag-region />
      <span className="local-badge" title="Данные и история хранятся только на этом компьютере">
        <ShieldCheck size={14} /> Локально
      </span>
      <button
        className="icon-button"
        aria-label="Показать или скрыть контекст"
        onClick={toggleContext}
      >
        <PanelRightClose size={17} />
      </button>
      {isTauri() && (
        <div className="window-controls">
          <button aria-label="Свернуть окно" onClick={() => windowAction('minimize')}>
            <Minus size={15} />
          </button>
          <button
            aria-label="Развернуть или восстановить окно"
            onClick={() => windowAction('toggleMaximize')}
          >
            <Square size={12} />
          </button>
          <button
            className="window-close"
            aria-label="Закрыть окно"
            onClick={() => windowAction('close')}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </header>
  );
}
