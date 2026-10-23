# Tauri SQL Manager

基于 **Tauri 2 + Vue 3 + TypeScript + Element Plus** 的跨平台数据库管理桌面应用，
由原 [WebSQLManager](../WebSQLManager)（SparkJava + 浏览器 + CDN 前端）重构而来。

## 重构要点

| 维度 | 原项目 WebSQLManager | 本项目 TauriSQLManager |
|------|---------------------|------------------------|
| 形态 | 浏览器访问的 Web 面板 | 原生桌面应用（无浏览器、无服务端） |
| 后端 | Java 21 + SparkJava + JDBC | Rust + Tauri 2 + sqlx / tiberius |
| 数据访问 | JDBC `DatabaseMetaData` | 各数据库系统表 / `information_schema` 查询 |
| 前端 | Vue 3 CDN + 全局组件 | Vue 3 + Vite + TS + SFC + SCSS |
| 依赖加载 | CDN（需联网） | 全部打包进应用，离线可用 |
| 编辑器 | CodeMirror 5（CDN） | CodeMirror 6（npm，按需打包） |
| 登录认证 | Token 会话 + 登录页 | 桌面单机应用，无需登录 |
| 会话恢复 | 无 | 启动时恢复上次的连接与页面 |
| 配置存储 | 进程工作目录 `connections.json` | 系统应用数据目录（原子写入） |
| 数据库地址 | JDBC URL 字符串 | 结构化的 host / port / database |
| 传输层 | HTTP 轮询 | Tauri `invoke` IPC |

## 支持的数据库

| 类型 | 驱动 | 说明 |
|------|------|------|
| SQLite | sqlx | 单文件数据库 |
| MySQL | sqlx | 8.x |
| MariaDB | sqlx | 复用 MySQL 协议 |
| PostgreSQL | sqlx | 12+ |
| SQL Server | tiberius | TDS 协议，纯 Rust 实现 |

> 原项目中的 **Oracle** 与 **H2** 在 Rust 生态缺少成熟的纯 Rust 驱动，本次重构未纳入。
> 如需保留可通过 Tauri sidecar 接入原生客户端，或在 `db/` 下新增驱动实现。

## 环境要求

- **Node.js** >= 20（开发阶段使用 24.x 验证）
- **Rust** >= 1.77（开发阶段使用 1.99 验证）
- **macOS**：Xcode Command Line Tools（`xcode-select --install`）
- **Windows**：WebView2 Runtime + MSVC 构建工具
- **Linux**：`webkit2gtk-4.1`、`libayatana-appindicator3`、`librsvg2` 等

## 开发

```bash
# 安装前端依赖
npm install

# 启动桌面开发模式（Vite HMR + Rust 热重载）
npm run tauri:dev

# 仅启动前端（浏览器里调样式，不连后端）
npm run dev
```

## 构建

### 本地构建（仅当前平台）

```bash
# 类型检查 + 前端构建
npm run build

# 打包安装程序（自动生成本平台可用的格式）
npm run tauri:build

# 或指定平台格式
npm run build:mac     # .app + .dmg
npm run build:win     # .msi + .exe (NSIS)
npm run build:linux   # .deb + .AppImage
```

产物位置：`src-tauri/target/release/bundle/`

> Tauri **不支持交叉编译**安装包，Windows 安装包必须在 Windows 上构建，
> Linux 包必须在 Linux 上构建。跨平台构建请使用下方的 CI。

## 自动化多平台构建（GitHub Actions）

仓库内置 `.github/workflows/build.yml`，在**推送代码**或**推送 tag** 时自动执行：

| 触发方式 | 行为 |
|----------|------|
| push / PR 到 `main` | 运行检查 + 构建全部平台安装包（作为 Actions 产物保留 30 天） |
| 推送 `v*` tag | 额外自动创建 GitHub Release 并附加所有安装包 |

### 产物对照

| 平台 | 格式 | 文件名示例 |
|------|------|-----------|
| macOS (Apple Silicon) | `.dmg` / `.app` | `Tauri SQL Manager_1.0.0_aarch64.dmg` |
| macOS (Intel) | `.dmg` / `.app` | `Tauri SQL Manager_1.0.0_x64.dmg` |
| Windows x64 | `.msi` / `.exe` | `Tauri SQL Manager_1.0.0_x64_en-US.msi` |
| Linux x64 | `.deb` / `.AppImage` | `tauri-sql-manager_1.0.0_amd64.deb` |

### 发布新版本

```bash
# 修改版本号后打 tag 推送
git tag v1.0.1
git push origin v1.0.1
```

CI 会自动构建四个平台并在 Releases 页面生成下载链接。

### 下载安装包

- **最新开发版**：仓库 `Actions` → 选择对应运行记录 → 页面底部 `Artifacts`
- **正式发布版**：仓库 `Releases` 页面

### 代码签名（可选，发布正式版建议配置）

| 平台 | 需要的 Secrets |
|------|---------------|
| macOS | `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID` |
| Windows | `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |

未配置签名时安装包仍可构建，但 macOS 首次打开需在「系统设置 → 隐私与安全性」中允许，
Windows 会提示 SmartScreen 警告。

## 测试

```bash
# Rust 单元测试（方言 SQL 生成、类型解析、SQL 拆分等）
cd src-tauri && cargo test --lib

# SQLite 集成测试（真实建库读写）
cd src-tauri && cargo test --test sqlite_integration

# 前端类型检查
npm run typecheck
```

## 目录结构

```
TauriSQLManager/
├── src/                        # 前端（Vue 3 + TS）
│   ├── api/
│   │   ├── index.ts            # Tauri invoke 封装（类型安全的命令调用）
│   │   └── types.ts            # 与 Rust 模型对应的 TS 类型
│   ├── components/
│   │   └── SqlCodeEditor.vue   # CodeMirror 6 SQL 编辑器
│   ├── stores/app.ts           # 全局响应式状态
│   ├── views/
│   │   ├── ConnectionManager.vue  # 连接管理
│   │   ├── TableExplorer.vue      # 表结构 / 索引
│   │   ├── DataBrowser.vue        # 数据浏览 / 增删改 / 导入导出
│   │   ├── SqlEditor.vue          # SQL 执行 + 结果 + 历史
│   │   └── AppSettings.vue        # 设置
│   ├── styles/app.scss
│   ├── App.vue                 # 布局骨架
│   └── main.ts
└── src-tauri/                  # 后端（Rust）
    ├── src/
    │   ├── model.rs            # 数据模型（serde camelCase）
    │   ├── store.rs            # 本地配置持久化
    │   ├── service.rs          # 业务服务层
    │   ├── commands.rs         # Tauri 命令（IPC 入口）
    │   └── db/
    │       ├── dialect.rs      # SQL 方言（引用、分页、DDL、DML）
    │       ├── conn.rs         # 连接池管理 + 连接串构造
    │       ├── exec.rs         # 统一查询执行 + 类型转换
    │       ├── meta.rs         # 表/列/索引元数据查询
    │       └── mssql.rs        # SQL Server 驱动（tiberius）
    ├── tests/sqlite_integration.rs
    ├── capabilities/default.json
    └── tauri.conf.json
```

## 架构说明

### 数据访问层

原项目依赖 JDBC 的 `DatabaseMetaData` 统一拿元数据；Rust 侧没有等价抽象，
因此改为**按数据库分别查询系统表**（`sqlite_master` / `information_schema` /
`pg_class` / `sys.columns`），再归一化到统一模型。这样既避免了引入重量级 ORM，
也能拿到各数据库特有的信息（PostgreSQL 的 comment、MySQL 的行数估算等）。

### SQL 方言

`db/dialect.rs` 集中处理方言差异，均有单元测试覆盖：

- 标识符引用：双引号 / 反引号 / 方括号
- 分页语法：`LIMIT` / `LIMIT offset,limit` / `OFFSET … FETCH NEXT`
- 自增列：`AUTO_INCREMENT` / `SERIAL` / `IDENTITY(1,1)` / `AUTOINCREMENT`
- 冲突处理：`INSERT OR IGNORE` / `INSERT IGNORE` / `ON CONFLICT`（SQL Server 无等价语法，逐行回退）

### 类型转换

sqlx 每种驱动有独立的类型系统。`exec.rs` 统一把结果集转换为 `serde_json::Value`：

- 整数 → JSON number；浮点 → number（NaN/Inf 转 NULL）
- 定点小数（DECIMAL/NUMERIC）→ 字符串，避免精度丢失
- 日期时间 → ISO 字符串
- 二进制 → Base64 字符串

### 安全

- 所有 DML 使用**参数绑定**，不拼接用户数据
- 表名列名一律走 `quote_identifier` 转义
- 排序字段做**白名单校验**（必须存在于表结构中）
- 连接密码只存本地，返回前端时清空

## 配置存储

配置保存在系统应用数据目录：

- macOS: `~/Library/Application Support/tauri-sql-manager/`
- Windows: `%APPDATA%\tauri-sql-manager\`
- Linux: `~/.local/share/tauri-sql-manager/`

包含 `connections.json`（连接，**含明文密码**，请注意权限）与 `settings.json`。
写入采用「临时文件 + 重命名」，避免中途崩溃损坏配置。

## 从 WebSQLManager 迁移

本项目**不自动读取**旧版 `connections.json`（JDBC URL 到结构化字段的映射存在歧义）。
迁移方式：在应用内新建连接即可。旧配置的 JDBC URL 拆解规律：

```
jdbc:mysql://host:3306/dbname?params  ->  host=host  port=3306  database=dbname
jdbc:sqlite:/path/to/file.db          ->  database=/path/to/file.db
```

## 已知限制

- **Oracle / H2** 未支持（见上文说明）
- **SQLite 修改列**：SQLite 不具备 `ALTER COLUMN`，界面会提示先删列再加列
- **SQL Server 重命名表/列**：使用 `sp_rename`，需要相应权限
- 导入采用**逐行执行**，大文件（十万行以上）速度受限；如需高速导入可改用驱动原生批量接口

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl/Cmd + Enter` | 执行 SQL（有选区时仅执行选中部分） |
| `Ctrl/Cmd + Space` | 触发补全（CodeMirror 内置） |
