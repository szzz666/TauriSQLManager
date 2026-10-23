/**
 * 与 Rust 后端共享的数据类型定义
 * 命名与 src-tauri/src/model.rs 中的结构体保持一致（serde rename_all = "camelCase"）
 */

/** 支持的数据库类型 */
export type DbType = 'sqlite' | 'mysql' | 'mariadb' | 'postgresql' | 'mssql'

/** 数据库连接配置 */
export interface ConnectionConfig {
  id: string
  name: string
  type: DbType
  /** 主机 */
  host: string
  /** 端口 */
  port: number
  /** 数据库名 / SQLite 文件路径 */
  database: string
  username: string
  /** 密码：读取时始终为空，写入时留空表示不修改 */
  password: string
  /** 是否自动重连 */
  autoReconnect: boolean
  createdAt: number
  updatedAt: number
}

/** 新建/更新连接的载荷 */
export interface ConnectionInput {
  id?: string
  name: string
  type: DbType
  host: string
  port: number
  database: string
  username: string
  password?: string
  autoReconnect?: boolean
}

/** 连接测试结果 */
export interface TestResult {
  success: boolean
  message: string
  elapsed: number
  version?: string
}

/** 连接运行状态 */
export interface ConnectionStatus {
  exists: boolean
  name?: string
  type?: string
  active: boolean
  version?: string
  error?: string
}

/** 表信息 */
export interface TableInfo {
  name: string
  comment: string
  tableType: string
  rowCount: number
  dataSize: number
  columns: ColumnInfo[]
  primaryKeys: string[]
  createTime: string
  updateTime: string
}

/** 列信息 */
export interface ColumnInfo {
  name: string
  type: string
  length: number
  scale: number
  nullable: boolean
  autoIncrement: boolean
  primaryKey: boolean
  unique: boolean
  defaultValue: string
  comment: string
  ordinal: number
}

/** 索引信息 */
export interface IndexInfo {
  name: string
  columnName: string
  unique: boolean
  primaryKey: boolean
  ascOrDesc: string
  ordinal: number
}

/** 建表/改表的列定义 */
export interface ColumnDefinition {
  name: string
  type: string
  length: number
  scale: number
  nullable: boolean
  autoIncrement: boolean
  primaryKey: boolean
  unique: boolean
  defaultValue?: string
  comment?: string
  /** 改表操作：ADD / MODIFY / DROP / RENAME */
  action?: string
  oldName?: string
}

/** SQL 执行结果 */
export interface QueryResult {
  query: boolean
  columns: string[]
  columnTypes: string[]
  rows: unknown[][]
  affectedRows: number
  elapsed: number
  sql: string
  truncated: boolean
  returnedRows: number
  message: string
}

/** 分页表数据 */
export interface TableData {
  rows: Record<string, unknown>[]
  columns: string[]
  columnTypes: string[]
  total: number
  page: number
  pageSize: number
  totalPages: number
}

/** 表数据查询参数 */
export interface TableDataParams {
  page: number
  pageSize: number
  sortColumn?: string
  sortOrder?: 'ASC' | 'DESC'
  where?: string
}

/** 导入结果统计 */
export interface ImportResult {
  totalRows: number
  affectedRows: number
  format: string
  table: string
}

/** 应用设置 */
export interface AppSettings {
  maxQueryRows: number
  queryTimeout: number
  pageSize: number
  /** 上次使用的连接 id（启动时恢复） */
  lastConnectionId?: string | null
  /** 上次停留在的页面（启动时恢复） */
  lastMenu?: string | null
}

/** “影响行数”类操作的结果 */
export interface AffectedResult {
  affected: number
}

/** 应用信息 */
export interface AppInfo {
  name: string
  version: string
  supportedTypes: string[]
  /** 与 supportedTypes 对应的展示名称 */
  supportedTypeLabels: string[]
}
