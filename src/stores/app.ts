/**
 * 全局应用状态（轻量响应式 store，避免为单一状态引入额外依赖）
 */
import { reactive, computed } from 'vue'
import type { ConnectionConfig, DbType } from '@/api/types'

export type MenuKey = 'connections' | 'tables' | 'data' | 'sql' | 'settings'

interface AppState {
  /** 当前激活的连接 */
  activeConnection: ConnectionConfig | null
  /** 当前菜单 */
  activeMenu: MenuKey
  /** 侧边栏是否折叠 */
  sidebarCollapsed: boolean
  /** 数据页要打开的表 */
  browseTable: string
  /** 数据库类型 -> 连接状态 */
  connectionStatus: Record<string, { active: boolean; version?: string }>
  /** 全局加载态 */
  loading: boolean
  /** 主题 */
  theme: 'light' | 'dark'
}

export const state = reactive<AppState>({
  activeConnection: null,
  activeMenu: 'connections',
  sidebarCollapsed: false,
  browseTable: '',
  connectionStatus: {},
  loading: false,
  theme: 'light',
})

/** 当前是否已选中连接 */
export const hasConnection = computed(() => state.activeConnection !== null)

export function selectConnection(conn: ConnectionConfig) {
  state.activeConnection = conn
  state.activeMenu = 'tables'
  state.browseTable = ''
}

export function clearConnection() {
  state.activeConnection = null
  state.browseTable = ''
  if (state.activeMenu !== 'connections' && state.activeMenu !== 'settings') {
    state.activeMenu = 'connections'
  }
}

/** 断开连接后，如果是当前连接则回到连接管理 */
export function onDisconnected(connId: string) {
  delete state.connectionStatus[connId]
  if (state.activeConnection?.id === connId) {
    clearConnection()
  }
}

export function browseData(table: string) {
  state.browseTable = table
  state.activeMenu = 'data'
}

export function setMenu(menu: MenuKey) {
  state.activeMenu = menu
}

/** 各数据库类型的展示信息 */
export const DB_TYPE_META: Record<DbType, { label: string; color: string; tag: string }> = {
  mysql: { label: 'MySQL', color: '#00758f', tag: 'primary' },
  mariadb: { label: 'MariaDB', color: '#c0765a', tag: 'primary' },
  postgresql: { label: 'PostgreSQL', color: '#336791', tag: 'success' },
  sqlite: { label: 'SQLite', color: '#0f80cc', tag: 'success' },
  mssql: { label: 'SQL Server', color: '#cc2927', tag: 'danger' },
}

export const DB_TYPES: DbType[] = ['mysql', 'mariadb', 'postgresql', 'mssql', 'sqlite']
