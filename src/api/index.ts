/**
 * Tauri 后端调用封装
 * 统一通过 invoke 调用 Rust 命令，并把错误转换成可读的 Error
 */
import { invoke } from '@tauri-apps/api/core'
import type {
  AffectedResult,
  AppInfo,
  AppSettings,
  ColumnDefinition,
  ColumnInfo,
  ConnectionConfig,
  ConnectionInput,
  ConnectionStatus,
  DbType,
  ImportResult,
  IndexInfo,
  QueryResult,
  TableData,
  TableDataParams,
  TableInfo,
  TestResult,
} from './types'

/**
 * 统一调用入口：后端返回 Result<T, String>，失败时这里包装成 Error
 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    // Rust 侧 Err(String) 会作为字符串抛出
    const message = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
    throw new Error(message || '操作失败')
  }
}

/** 应用信息与设置 */
export const app = {
  info: () => call<AppInfo>('app_info'),
  getSettings: () => call<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => call<void>('save_settings', { settings }),
  /** 数据目录（连接配置保存位置） */
  dataDir: () => call<string>('data_dir'),
  /** 保存会话：最后连接与页面，用于下次启动恢复 */
  saveSession: (lastConnectionId: string | null, lastMenu: string | null) =>
    call<void>('save_session', { lastConnectionId, lastMenu }),
}

/** 连接管理 */
export const connections = {
  list: () => call<ConnectionConfig[]>('list_connections'),
  get: (id: string) => call<ConnectionConfig>('get_connection', { id }),
  create: (input: ConnectionInput) => call<ConnectionConfig>('create_connection', { input }),
  update: (id: string, input: ConnectionInput) => call<ConnectionConfig>('update_connection', { id, input }),
  remove: (id: string) => call<void>('delete_connection', { id }),
  /** 测试未保存的连接配置 */
  test: (input: ConnectionInput) => call<TestResult>('test_connection', { input }),
  /** 测试已保存的连接 */
  testSaved: (id: string) => call<TestResult>('test_saved_connection', { id }),
  status: (id: string) => call<ConnectionStatus>('connection_status', { id }),
  disconnect: (id: string) => call<void>('disconnect', { id }),
  /** 读取 SQLite 文件绝对路径（用于文件选择器默认目录） */
  pickSqliteFile: () => call<string | null>('pick_sqlite_file'),
}

/** 表结构管理 */
export const tables = {
  list: (connId: string) => call<TableInfo[]>('list_tables', { connId }),
  get: (connId: string, table: string) => call<TableInfo>('get_table_info', { connId, table }),
  indexes: (connId: string, table: string) => call<IndexInfo[]>('list_indexes', { connId, table }),
  create: (connId: string, name: string, columns: ColumnDefinition[], comment?: string) =>
    call<void>('create_table', { connId, name, columns, comment: comment ?? null }),
  drop: (connId: string, table: string) => call<void>('drop_table', { connId, table }),
  rename: (connId: string, table: string, newName: string) =>
    call<void>('rename_table', { connId, table, newName }),
  truncate: (connId: string, table: string) => call<void>('truncate_table', { connId, table }),
  alter: (connId: string, table: string, columns: ColumnDefinition[]) =>
    call<string[]>('alter_table', { connId, table, columns }),
  addColumn: (connId: string, table: string, col: ColumnDefinition) =>
    call<void>('add_column', { connId, table, col }),
  modifyColumn: (connId: string, table: string, col: ColumnDefinition) =>
    call<void>('modify_column', { connId, table, col }),
  dropColumn: (connId: string, table: string, column: string) =>
    call<void>('drop_column', { connId, table, column }),
  /** 预览建表 SQL，不执行 */
  previewCreateSql: (connId: string, name: string, columns: ColumnDefinition[], comment?: string) =>
    call<string>('preview_create_sql', { connId, name, columns, comment: comment ?? null }),
}

/** 数据 CRUD */
export const data = {
  query: (connId: string, table: string, params: TableDataParams) =>
    call<TableData>('query_table_data', { connId, table, params }),
  insert: (connId: string, table: string, row: Record<string, unknown>) =>
    call<AffectedResult>('insert_row', { connId, table, row }),
  updateByPk: (connId: string, table: string, values: Record<string, unknown>, pk: Record<string, unknown>) =>
    call<AffectedResult>('update_row_by_pk', { connId, table, values, pk }),
  deleteByPk: (connId: string, table: string, pk: Record<string, unknown>) =>
    call<AffectedResult>('delete_row_by_pk', { connId, table, pk }),
  import: (connId: string, table: string, format: string, content: string, mode: string) =>
    call<ImportResult>('import_data', { connId, table, format, content, mode }),
}

/** SQL 执行 */
export const sql = {
  execute: (connId: string, sqlText: string) => call<QueryResult>('execute_sql', { connId, sqlText }),
  batch: (connId: string, sqlText: string) => call<QueryResult[]>('execute_batch', { connId, sqlText }),
  queryPaged: (connId: string, sqlText: string, offset: number, limit: number) =>
    call<QueryResult>('execute_query_paged', { connId, sqlText, offset, limit }),
  /** 导出结果并写入用户选择的文件；返回写入的绝对路径 */
  exportToFile: (connId: string, sqlText: string, format: string, tableName?: string) =>
    call<string | null>('export_query_to_file', { connId, sqlText, format, tableName: tableName ?? null }),
  /** 导出结果内容（不落盘），用于复制到剪贴板等场景 */
  exportContent: (connId: string, sqlText: string, format: string, tableName?: string) =>
    call<string>('export_query_content', { connId, sqlText, format, tableName: tableName ?? null }),
}

/** 数据库类型元信息 */
export const dialects = {
  /** 各数据库的常用列类型，用于建表时下拉提示 */
  columnTypes: (type: DbType) => call<string[]>('dialect_column_types', { dbType: type }),
  defaultPort: (type: DbType) => call<number>('dialect_default_port', { dbType: type }),
}

export type {
  AffectedResult,
  AppInfo,
  AppSettings,
  ColumnDefinition,
  ConnectionConfig,
  ConnectionInput,
  ConnectionStatus,
  DbType,
  ImportResult,
  IndexInfo,
  QueryResult,
  TableData,
  TableDataParams,
  TableInfo,
  TestResult,
}
