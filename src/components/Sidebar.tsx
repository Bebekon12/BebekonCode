import {
  ChevronsLeft,
  ChevronsRight,
  Cpu,
  Folder,
  FolderOpen,
  Home,
  Info,
  MessageSquare,
  Network,
  Plus,
  Settings2,
  UsersRound,
} from 'lucide-react';
import type { Session, Snapshot, Workspace } from '../contracts';
import { displayPath, sessionTitle } from '../locale';

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
    {
      label: 'Аккаунты',
      icon: Cpu,
      action: () => showSettings('Провайдеры'),
      enabled: !!data,
    },
    { label: 'Файлы проекта', icon: FolderOpen, action: openFiles, enabled: !!workspace },
  ];
  return (
    <aside className={`sidebar ${collapsed ? 'collapsed' : ''}`} aria-label="Проекты и чаты">
      <button
        className="new-session-button"
        onClick={newSession}
        disabled={disabled}
        title="Новый чат (Ctrl+N)"
      >
        <Plus size={18} />
        <span className="sidebar-label">Новый чат</span>
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
        <span>Чаты</span>
      </div>
      <div className="workspace-list">
        {data?.sessions
          .filter((s) => !s.parent_session_id)
          .map((item) => (
            <button
              className={`session-button recent-chat ${sessionId === item.id ? 'selected' : ''}`}
              key={item.id}
              onClick={() => selectSession(item)}
              title={sessionTitle(item.title)}
            >
              {item.chat_mode === 'team' ? (
                <UsersRound size={18} />
              ) : item.chat_mode === 'auto' ? (
                <Network size={18} />
              ) : (
                <MessageSquare size={18} />
              )}
              <span className="session-copy sidebar-label">
                <span className="session-title">{sessionTitle(item.title)}</span>
              </span>
              {(item.status === 'running' || item.status === 'failed') && (
                <span className={`status-dot ${item.status}`} />
              )}
            </button>
          ))}
        <div className="sidebar-heading sidebar-label">
          <span>Проекты</span>
        </div>
        {data?.workspaces
          .filter((w) => w.id !== 'chat-scratch')
          .map((project) => {
            const current = workspace?.id === project.id;
            const sessions = data.sessions.filter(
              (session) => !session.parent_session_id && session.workspace_id === project.id,
            );
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
                {current && !collapsed && !sessions.length && (
                  <div className="project-sessions">
                    {!sessions.length && (
                      <button className="project-new" onClick={() => startSession(project.id)}>
                        <Plus size={13} /> Новый чат
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
        <button
          className="sidebar-footer-button"
          onClick={() => showSettings()}
          disabled={!data}
          title={collapsed ? 'Настройки' : undefined}
        >
          <Settings2 size={19} />
          <span className="sidebar-label">Настройки</span>
        </button>
        <button
          className="sidebar-footer-button"
          onClick={() => showSettings('О программе и обновления')}
          disabled={!data}
          title={collapsed ? 'О программе и обновления' : undefined}
        >
          <Info size={19} />
          <span className="sidebar-label">О программе и обновления</span>
        </button>
        {preview && <span className="sidebar-preview sidebar-label">Предпросмотр</span>}
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
