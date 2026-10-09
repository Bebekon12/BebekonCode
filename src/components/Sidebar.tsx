import { useState } from 'react';
import {
  ChevronDown,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Cpu,
  Folder,
  Home,
  Info,
  MessageSquare,
  Network,
  Plus,
  Puzzle,
  Search,
  Settings2,
  Trash2,
  UsersRound,
} from 'lucide-react';
import type { CreateChat, Session, Snapshot, Workspace } from '../contracts';
import { displayPath, sessionTitle } from '../locale';
import type { AccountState } from '../accounts';
import { SubscriptionLimits } from './SubscriptionLimits';
import { usePreference } from '../preferences';

const chatGroups = [
  { mode: 'single', label: 'Обычные чаты', create: 'Создать обычный чат', icon: MessageSquare },
  { mode: 'team', label: 'Команды агентов', create: 'Создать команду агентов', icon: UsersRound },
  { mode: 'auto', label: 'Авторазбиение', create: 'Создать чат с авторазбиением', icon: Network },
] as const;

export function Sidebar({
  data,
  workspace,
  sessionId,
  home,
  collapsed,
  disabled,
  toggleCollapsed,
  newSession,
  createChat,
  addProject,
  goHome,
  search,
  showSettings,
  openPlugins,
  selectProject,
  selectSession,
  startSession,
  preview,
  accountState,
  openUsage,
  deleteChat,
}: {
  data: Snapshot | null;
  workspace?: Workspace;
  sessionId: string;
  home: boolean;
  collapsed: boolean;
  disabled: boolean;
  toggleCollapsed: () => void;
  newSession: () => void;
  createChat: (mode: CreateChat['mode']) => void;
  addProject: () => void;
  goHome: () => void;
  search: () => void;
  showSettings: (tab?: string) => void;
  openPlugins: () => void;
  selectProject: (id: string) => void;
  selectSession: (session: Session) => void;
  startSession: (projectId: string) => void;
  preview: boolean;
  accountState: AccountState;
  openUsage: (provider: string) => void;
  /** Asks to delete a chat; the caller confirms first. */
  deleteChat: (session: Session) => void;
}) {
  const [projectStates, setProjectStates] = usePreference<Record<string, boolean>>(
    'project-folders',
    {},
  );
  const [closedGroups, setClosedGroups] = useState<string[]>([]);
  const projects = data?.workspaces.filter((w) => w.id !== 'chat-scratch') ?? [];
  const projectIds = new Set(projects.map((p) => p.id));
  const roots = data?.sessions.filter((s) => !s.parent_session_id) ?? [];
  const loose = roots.filter((s) => !projectIds.has(s.workspace_id));
  const toggleGroup = (id: string) =>
    setClosedGroups((ids) => (ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id]));
  const chat = (item: Session) => {
    const Icon =
      item.chat_mode === 'team' ? UsersRound : item.chat_mode === 'auto' ? Network : MessageSquare;
    return (
      <div className="chat-row" key={item.id}>
        <button
          className={`session-button recent-chat ${sessionId === item.id ? 'selected' : ''}`}
          data-session-id={item.id}
          onClick={() => selectSession(item)}
          title={`${sessionTitle(item.title)} · ${chatGroups.find((group) => group.mode === item.chat_mode)?.label ?? 'Чат'}`}
          aria-current={sessionId === item.id ? 'page' : undefined}
        >
          <Icon size={17} />
          <span className="session-copy sidebar-label">
            <span className="session-title">{sessionTitle(item.title)}</span>
          </span>
          {(item.status === 'running' || item.status === 'failed') && (
            <span className={`status-dot ${item.status}`} />
          )}
        </button>
        {!collapsed && (
          <button
            className="icon-button chat-delete"
            aria-label="Удалить чат"
            title={item.status === 'running' ? 'Сначала остановите выполнение' : 'Удалить чат'}
            disabled={disabled || item.status === 'running'}
            onClick={() => deleteChat(item)}
          >
            <Trash2 size={15} />
          </button>
        )}
      </div>
    );
  };
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
        <button onClick={search} disabled={!data} title="Поиск чатов и команд (Ctrl+K)">
          <Search size={18} />
          <span className="sidebar-label">Поиск</span>
        </button>
        <button onClick={goHome} aria-current={home ? 'page' : undefined} title="Главная">
          <Home size={18} />
          <span className="sidebar-label">Главная</span>
        </button>
      </nav>
      <div className="workspace-list">
        <section className="sidebar-section" aria-label="Проекты">
          <div className="sidebar-section-heading">
            <button
              onClick={() => toggleGroup('projects')}
              aria-expanded={!closedGroups.includes('projects')}
              title="Проекты"
            >
              <ChevronDown
                size={13}
                className={closedGroups.includes('projects') ? 'closed' : ''}
              />
              <span className="sidebar-label">Проекты</span>
            </button>
            <button
              className="section-create"
              onClick={addProject}
              disabled={disabled}
              aria-label="Добавить проект"
              title="Добавить папку проекта"
            >
              <Plus size={16} />
            </button>
          </div>
          {!closedGroups.includes('projects') &&
            projects.map((project) => {
              const open = projectStates?.[project.id] ?? workspace?.id === project.id;
              const sessions = roots.filter((s) => s.workspace_id === project.id);
              return (
                <section
                  className="workspace-group"
                  key={project.id}
                  aria-label={`Проект ${project.name}`}
                >
                  <div className="project-tree-row">
                    <button
                      className="project-chevron"
                      aria-label={`${open ? 'Свернуть' : 'Развернуть'} проект ${project.name}`}
                      aria-expanded={open}
                      onClick={() =>
                        setProjectStates((states) => ({ ...states, [project.id]: !open }))
                      }
                    >
                      <ChevronRight size={14} className={open ? 'expanded' : ''} />
                    </button>
                    <button
                      className={`project-button ${workspace?.id === project.id ? 'current-project' : ''}`}
                      onClick={() => {
                        setProjectStates((states) => ({ ...states, [project.id]: true }));
                        selectProject(project.id);
                      }}
                      title={displayPath(project.root)}
                    >
                      <Folder size={17} />
                      <span className="sidebar-label project-name">{project.name}</span>
                    </button>
                    <button
                      className="section-create project-create"
                      onClick={() => {
                        setProjectStates((states) => ({ ...states, [project.id]: true }));
                        startSession(project.id);
                      }}
                      disabled={disabled}
                      aria-label={`Новый чат в проекте ${project.name}`}
                      title="Новый чат в проекте"
                    >
                      <Plus size={15} />
                    </button>
                  </div>
                  {open && !collapsed && (
                    <div className="project-sessions">
                      {sessions.map(chat)}
                      {!sessions.length && (
                        <button
                          className="project-new"
                          onClick={() => startSession(project.id)}
                          disabled={disabled}
                        >
                          <Plus size={14} /> Новый чат
                        </button>
                      )}
                    </div>
                  )}
                </section>
              );
            })}
          {!projects.length && !collapsed && !closedGroups.includes('projects') && (
            <button className="sidebar-empty" onClick={addProject} disabled={disabled}>
              Добавить папку проекта
            </button>
          )}
        </section>
        {chatGroups.map(({ mode, label, create, icon: Icon }) => {
          const sessions = loose.filter((s) => s.chat_mode === mode);
          const open = !closedGroups.includes(mode);
          return (
            <section className="sidebar-section" key={mode} aria-label={label}>
              <div className="sidebar-section-heading">
                <button onClick={() => toggleGroup(mode)} aria-expanded={open} title={label}>
                  <Icon size={16} />
                  <span className="sidebar-label">{label}</span>
                </button>
                <button
                  className="section-create"
                  aria-label={create}
                  title={create}
                  disabled={disabled}
                  onClick={() => createChat(mode)}
                >
                  <Plus size={16} />
                </button>
              </div>
              {open && sessions.map(chat)}
              {open && !sessions.length && !collapsed && (
                <span className="sidebar-empty">Пока нет чатов</span>
              )}
            </section>
          );
        })}
      </div>
      <div className="sidebar-bottom">
        <SubscriptionLimits
          accounts={data?.accounts ?? []}
          state={accountState}
          collapsed={collapsed}
          openUsage={openUsage}
        />
        <button
          className="sidebar-footer-button"
          onClick={() => showSettings('Провайдеры')}
          disabled={!data}
          title="Аккаунты"
        >
          <Cpu size={18} />
          <span className="sidebar-label">Аккаунты</span>
        </button>
        <button
          className="sidebar-footer-button"
          onClick={openPlugins}
          disabled={!data}
          title="Плагины"
        >
          <Puzzle size={18} />
          <span className="sidebar-label">Плагины</span>
        </button>
        <button
          className="sidebar-footer-button"
          onClick={() => showSettings()}
          disabled={!data}
          title="Настройки"
        >
          <Settings2 size={18} />
          <span className="sidebar-label">Настройки</span>
        </button>
        <div className="sidebar-utilities">
          <button
            className="collapse-button"
            onClick={toggleCollapsed}
            aria-label={collapsed ? 'Развернуть меню' : 'Свернуть меню'}
            title={collapsed ? 'Развернуть меню' : 'Свернуть меню'}
          >
            {collapsed ? <ChevronsRight size={17} /> : <ChevronsLeft size={17} />}
            <span className="sidebar-label">Свернуть</span>
          </button>
          <button
            className="icon-button"
            onClick={() => showSettings('О программе и обновления')}
            disabled={!data}
            aria-label="О программе и обновления"
            title="О программе и обновления"
          >
            <Info size={18} />
          </button>
        </div>
        {preview && <span className="sidebar-preview sidebar-label">Предпросмотр</span>}
      </div>
    </aside>
  );
}
