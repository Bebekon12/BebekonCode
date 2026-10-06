import {
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Cpu,
  Folder,
  FolderOpen,
  FolderPlus,
  Home,
  Plus,
  Settings2,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import type { Session, Snapshot, Workspace } from '../contracts';
import { accountLabel, displayPath, sessionTitle, statusLabels } from '../locale';

export function Sidebar({
  data,
  workspace,
  sessionId,
  home,
  collapsed,
  disabled,
  toggleCollapsed,
  newSession,
  addProject,
  goHome,
  showSettings,
  openFiles,
  selectProject,
  selectSession,
  startSession,
  preview,
}: {
  data: Snapshot | null;
  workspace?: Workspace;
  sessionId: string;
  home: boolean;
  collapsed: boolean;
  disabled: boolean;
  toggleCollapsed: () => void;
  newSession: () => void;
  addProject: () => void;
  goHome: () => void;
  showSettings: (tab?: string) => void;
  openFiles: () => void;
  selectProject: (id: string) => void;
  selectSession: (session: Session) => void;
  startSession: (projectId: string) => void;
  preview: boolean;
}) {
  const nav = [
    { label: 'Главная', icon: Home, action: goHome, active: home, enabled: true },
    { label: 'Мои проекты', icon: FolderPlus, action: addProject, enabled: !disabled },
    { label: 'Агенты', icon: Sparkles, action: newSession, enabled: !disabled },
    {
      label: 'Провайдеры',
      icon: Cpu,
      action: () => showSettings('Провайдеры'),
      enabled: !!data,
    },
    { label: 'Файлы проекта', icon: FolderOpen, action: openFiles, enabled: !!workspace },
    { label: 'Настройки', icon: Settings2, action: () => showSettings(), enabled: !!data },
  ];
  return (
    <aside className={`sidebar ${collapsed ? 'collapsed' : ''}`} aria-label="Проекты и сессии">
      <button
        className="new-session-button"
        onClick={newSession}
        disabled={disabled}
        title="Новая сессия (Ctrl+N)"
      >
        <Plus size={18} />
        <span className="sidebar-label">Новая сессия</span>
        <ChevronRight size={16} className="sidebar-label" />
      </button>
      <nav className="primary-nav" aria-label="Навигация">
        {nav.map(({ label, icon: Icon, action, active, enabled }) => (
          <button
            key={label}
            className={active ? 'active' : ''}
            aria-current={active ? 'page' : undefined}
            onClick={action}
            disabled={!enabled}
            title={collapsed ? label : undefined}
          >
            <Icon size={18} /> <span className="sidebar-label">{label}</span>
          </button>
        ))}
      </nav>
      <div className="sidebar-heading sidebar-label">
        <span>РАБОЧИЕ ОБЛАСТИ</span>
      </div>
      <div className="workspace-list">
        {data?.workspaces.map((project) => {
          const current = workspace?.id === project.id;
          const sessions = data.sessions.filter((session) => session.workspace_id === project.id);
          return (
            <section className="workspace-group" key={project.id}>
              <button
                className={`project-button ${current ? 'current-project' : ''}`}
                aria-expanded={current}
                onClick={() => selectProject(project.id)}
                title={collapsed ? project.name : displayPath(project.root)}
              >
                <Folder size={16} />
                <span className="sidebar-label project-name">{project.name}</span>
                {current && <span className="project-active-dot" aria-label="Открыт" />}
              </button>
              {current && !collapsed && (
                <div className="project-sessions">
                  {sessions.map((item) => (
                    <button
                      className={`session-button ${sessionId === item.id ? 'selected' : ''}`}
                      key={item.id}
                      onClick={() => selectSession(item)}
                      title={sessionTitle(item.title)}
                    >
                      <span
                        className={`status-dot ${item.status}`}
                        role="img"
                        aria-label={statusLabels[item.status]}
                      />
                      <span className="session-copy">
                        <span className="session-title">{sessionTitle(item.title)}</span>
                        <span className="session-meta">
                          {item.model} ·{' '}
                          {accountLabel(
                            data.accounts.find((account) => account.id === item.account_profile_id),
                          )}
                        </span>
                      </span>
                    </button>
                  ))}
                  {!sessions.length && (
                    <button className="project-new" onClick={() => startSession(project.id)}>
                      <Plus size={13} /> Начать сессию
                    </button>
                  )}
                </div>
              )}
            </section>
          );
        })}
        <button
          className="project-button add-project"
          onClick={addProject}
          disabled={disabled}
          title={collapsed ? 'Добавить проект' : undefined}
          aria-label="Добавить проект"
        >
          <Plus size={16} /> <span className="sidebar-label">Добавить проект</span>
        </button>
      </div>
      <div className="sidebar-bottom">
        <div className="local-card sidebar-label">
          <div className="local-card-title">
            <ShieldCheck size={18} />
            <strong>Локальный режим</strong>
          </div>
          <p>Проекты и история сессий хранятся только на этом компьютере.</p>
          <button
            className="secondary-button"
            onClick={() => showSettings('О программе и обновления')}
            disabled={!data}
          >
            О программе и обновления
          </button>
        </div>
        <div className="local-status sidebar-label">
          <span className="status-dot completed" /> Локальное ядро
          <span>{preview ? 'Предпросмотр' : 'Приложение'}</span>
        </div>
        <button
          className="collapse-button"
          onClick={toggleCollapsed}
          aria-label={collapsed ? 'Развернуть меню' : 'Свернуть меню'}
          title={collapsed ? 'Развернуть меню' : undefined}
        >
          {collapsed ? <ChevronsRight size={17} /> : <ChevronsLeft size={17} />}
          <span className="sidebar-label">Свернуть меню</span>
        </button>
      </div>
    </aside>
  );
}
