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

有两种获取方式，对应不同的触发条件：

| 来源 | 触发条件 | 位置 | 保留期 |
|------|----------|------|--------|
| **Actions Artifacts** | 每次 push / PR / 手动触发 | `Actions` → 运行记录 → 页面底部 `Artifacts` | 30 天 |
| **Releases** | **仅推送 `v*` tag** | `Releases` 页面 | 永久 |

> ⚠️ 常见疑问：**只 push 代码到 main 时 Releases 页面不会出现安装包**，这是设计如此。
> 因为 `release` job 的条件是 `startsWith(github.ref, 'refs/tags/v')`。
> 想让安装包进入 Releases，必须打 tag 并推送：
>
> ```bash
> git tag -a v1.0.0 -m "v1.0.0"
> git push origin v1.0.0
> ```
>
> 推送后 CI 会完整跑一遍 4 平台构建，然后自动创建 Release 并附加全部安装包。

### macOS 签名说明（重要）

本项目默认使用 **ad-hoc 签名**（`signingIdentity: "-"`），这足以让 app 在不同机器上
正常安装和运行，**不会**出现「已损坏，无法打开」。

ad-hoc 签名解决的问题是：没有 `Contents/_CodeSignature/CodeResources` 的 app 被下载后会
带上 `com.apple.quarantine` 隔离属性，Gatekeeper 校验失败会直接报「已损坏」（而不是
可放行的「未验证开发者」提示）。

首次打开时可能出现 **「无法验证开发者」**，这是未使用 Apple 开发者证书的正常提示，
处理方式二选一：

```bash
# 方式 1（推荐）：右键点击 app → 打开 → 在弹窗中点击「打开」
# 方式 2：命令行移除隔离属性
xattr -dr com.apple.quarantine "/Applications/Tauri SQL Manager.app"
```

> 注意区分两种错误：
> - **「无法验证开发者」** = 正常现象，按上面方式放行即可
> - **「已损坏，无法打开」** = 签名无效（本项目已修复，即缺少 CodeResources 的情况）

### 正式分发（可选）

若要通过 Apple 公证（notarization）消除上述提示，需配置 Secrets：

| 平台 | 需要的 Secrets |
|------|---------------|
| macOS | `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID` |
| Windows | `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |

配置后 Tauri 会在构建时自动签名并公证，用户双击即可打开，无任何提示。

## 移动端构建（Android / iOS）

移动端工程由 Tauri 官方模板生成，**不入库**（`src-tauri/gen/android`、`src-tauri/gen/apple`
已在 `.gitignore` 中排除），由 CI 在构建前执行 `tauri android init` / `tauri ios init` 生成。

### 触发方式

工作流：`.github/workflows/mobile.yml`

| 触发 | 说明 |
|------|------|
| push / PR 到 `main` | 同时构建 Android 与 iOS |
| 推送 `v*` tag | 同上，产物随桌面端一起进入 Release |
| 手动触发（Actions → Mobile Build → Run workflow） | 可选择只构建 Android 或只构建 iOS |

### 产物对照

| 平台 | 产物 | 说明 |
|------|------|------|
| Android | `app-universal-release.apk` | 调试签名，可直接安装测试 |
| Android | `app-universal-release.aab` | 上架 Google Play 用 |
| iOS | `.ipa` | **需要 Apple 开发者证书**，否则只产出未签名产物 |

### Android 安装包说明

Release 中会提供两类 Android 产物，**用途完全不同**：

| 文件 | 能否安装 | 用途 |
|------|---------|------|
| `TauriSQLManager_<版本>-installable.apk` | ✅ **可以直接装到手机** | 测试分发、自己使用 |
| `TauriSQLManager_<版本>-release-unsigned.apk` | ❌ 系统拒绝安装 | 仅供配置正式签名后重新打包 |
| `app-universal-release.aab` | ❌ 不能安装 | 上传 Google Play 用 |

> ⚠️ 常见误区：`.aab` 是上架格式，**不能**直接安装到手机；
> 名字带 `unsigned` 的 APK 也装不上——Android 要求所有 APK 必须有签名。

`installable.apk` 使用**调试签名**（`debug.keystore`），所以能装，但：

- 不能上架 Google Play
- 与正式签名的应用不能共存（同一包名只能装一个）
- 后续换正式签名需要先卸载

### Android 上架配置（正式签名）

要在 Google Play 上架，需要用自己的密钥库签名。步骤：

1. 本地生成密钥库：

```bash
keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA \
  -keysize 2048 -validity 10000 -alias upload
```

2. 将密钥库 Base64 编码后存入仓库 Secrets：

```bash
base64 -i upload-keystore.jks | pbcopy   # macOS
```

3. 在仓库 `Settings → Secrets and variables → Actions` 中添加：

| Secret | 内容 |
|--------|------|
| `ANDROID_KEY_BASE64` | 密钥库的 Base64 |
| `ANDROID_KEY_ALIAS` | 例如 `upload` |
| `ANDROID_KEY_PASSWORD` | 密钥库密码 |

配置后 `release` 变体才会被正确签名，此时 `-release-unsigned.apk` 也会变成可安装的正式包。


要在 Google Play 上架，需要用自己的密钥库签名。步骤：

1. 本地生成密钥库：

```bash
keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA \
  -keysize 2048 -validity 10000 -alias upload
```

2. 将密钥库 Base64 编码后存入仓库 Secrets：

```bash
base64 -i upload-keystore.jks | pbcopy   # macOS
```

3. 在仓库 `Settings → Secrets and variables → Actions` 中添加：

| Secret | 内容 |
|--------|------|
| `ANDROID_KEY_BASE64` | 密钥库的 Base64 |
| `ANDROID_KEY_ALIAS` | 例如 `upload` |
| `ANDROID_KEY_PASSWORD` | 密钥库密码 |

未配置时使用调试签名，产物仅供测试，**不能上架**。

### iOS 构建说明

iOS 产物必须签名才能在真机安装。当前 CI 以 `--no-sign` 方式构建，
仅用于验证代码可编译通过，产物无法安装到设备。

要产出可安装的 `.ipa`，需要配置：

| Secret | 内容 |
|--------|------|
| `APPLE_CERTIFICATE` | `.p12` 证书的 Base64 |
| `APPLE_CERTIFICATE_PASSWORD` | 证书密码 |
| `APPLE_SIGNING_IDENTITY` | 例如 `iPhone Distribution: Your Name (TEAMID)` |
| `APPLE_TEAM_ID` | Apple 开发者 Team ID |
| `APPLE_PROVISIONING_PROFILE` | 描述文件 Base64（可选） |

配置后把工作流中的 `--no-sign` 去掉即可产出可安装的 ipa。

### 本地构建移动端

需要自备 Android SDK / NDK 或完整 Xcode：

```bash
# Android（需要 ANDROID_HOME + NDK_HOME）
npx tauri android init
npx tauri android build --apk

# iOS（需要完整 Xcode，非仅 Command Line Tools）
npx tauri ios init
npx tauri ios build
```

### 关于数据库功能的移动端可用性

| 数据库 | Android | iOS | 说明 |
|--------|---------|-----|------|
| SQLite | ✅ | ✅ | 使用 sqlx bundled 模式，自带 SQLite 源码编译 |
| PostgreSQL | ✅ | ✅ | 纯 Rust 驱动，无系统依赖 |
| MySQL / MariaDB | ✅ | ✅ | 同上 |
| SQL Server | ✅ | ✅ | tiberius + rustls，纯 Rust 实现 |

移动端连接远程数据库需要网络权限，Android 已在模板中声明 `INTERNET`，
iOS 首次连接时会弹出本地网络权限提示。

### 移动端使用须知（重要）

移动端是**功能受限版本**，与桌面端有本质差异：

| 差异 | 说明 |
|------|------|
| **SQLite 新建** | 移动端沙箱内无法浏览任意文件路径，连接时输入文件名即在该应用数据目录新建空库 |
| **SQLite 打开已有库** | 仅能通过文件选择器访问系统允许的目录（Android 的 Documents、iOS 的"文件"App） |
| **文件导出** | 走系统分享/文件选择对话框，路径由系统决定 |
| **界面** | 桌面布局已适配为单列 + 底部标签栏，但宽表格、SQL 编辑器在手机上操作体验有限 |
| **后台执行** | 移动端在应用切后台时可能被系统挂起，长时间查询建议保持前台 |

> 建议用途：移动端适合**快速查看数据、执行简单 SQL、应急排查**，
> 复杂的表结构管理与大数据量浏览仍推荐使用桌面端。


## 测试

```bash
# Rust 单元测试（方言 SQL 生成、类型解析、SQL 拆分等）
cd src-tauri && cargo test --lib

# SQLite 集成测试（真实建库读写）
cd src-tauri && cargo test --test sqlite_integration

# 前端类型检查
npm run typecheck
```

## 应用图标

图标以 SVG 为源，用 `tauri icon` 一键生成全平台尺寸，无需手工导出各尺寸 PNG。

| 文件 | 用途 |
|------|------|
| `app-icon.svg` | 桌面图标源（macOS / Windows / Linux），含圆角方形背景 |
| `app-icon-android-fg.svg` | Android 自适应图标前景，透明背景，图形位于 66% 安全区 |
| `app-icon.manifest.json` | 图标生成清单，声明上述文件与背景色 |

重新生成全部图标：

```bash
npx tauri icon app-icon.manifest.json
```

生成内容覆盖：

- macOS `icon.icns`
- Windows `icon.ico` + 各种 Square 徽标
- Linux 各尺寸 PNG
- Android 各密度 `ic_launcher*` + 自适应图标前景
- iOS 全套 `AppIcon-*`

> Android 自适应图标的背景色在 `src-tauri/icons/android/values/ic_launcher_background.xml`，
> 已设为品牌蓝 `#2563EB`，与图标主色一致。

### Android 原生层覆盖（图标 / 状态栏）

`tauri android init` 生成的 `src-tauri/gen/android` 有两处与 App 需求冲突，
CI 在初始化之后统一覆盖（见 `.github/workflows/mobile.yml` 的「应用 Android 定制」步骤）：

| 问题 | 原因 | 处理 |
|------|------|------|
| 启动器图标不是蓝色数据库图标 | 模板自带 `mipmap-*/ic_launcher.png`、`ic_launcher_foreground.png` 与 `drawable/ic_launcher_background.xml`，而 `tauri android init` 不会把 `src-tauri/icons/android` 复制进工程 | 显式 `cp` 覆盖 5 个密度的 PNG，重写 `mipmap-anydpi-v26/ic_launcher.xml` / `ic_launcher_round.xml`，删除模板机器人矢量图，最后用 `cmp` 校验 |
| 顶部内容被状态栏压住 | Android 15（API 35）起，`targetSdk >= 35` 会被系统强制 edge-to-edge；`windowOptOutEdgeToEdgeEnforcement` 从 API 36 起也被忽略 | 用 `src-tauri/mobile/android/MainActivity.kt` 覆盖模板的 `MainActivity.kt`（模板里硬编码了 `enableEdgeToEdge()`），**按 API 级别分流**：API ≥ 35 用 `setOnApplyWindowInsetsListener` 给内容视图加 `paddingTop`（状态栏区域填品牌蓝）；API < 35 系统本来就会让位，只设 `window.statusBarColor` 与浅色状态栏图标，**绝不能**也加 inset 监听——否则持续重排会触发 ANR |

> 这两处都是原生层行为，纯 CSS 改不动：Android WebView 的 `env(safe-area-inset-top)` 返回 `0`。
> 因此移动端顶栏固定为 `#2563EB`（不随深色主题变化），与状态栏底色连成一体。

> 写 `MainActivity.kt` 的坑：`onCreate` 阶段 `DecorView` 还没创建，此时访问 `window.insetsController`
> 会抛 `NullPointerException`（`PhoneWindow.getInsetsController` 直接解引用 `mDecor`，Kotlin 的 `?.` 挡不住）。
> 必须先 `val decor: View = window.decorView` 触发 `installDecor()`，再操作 inset。

### 本地补丁：wry（Android 冷启动崩溃）

`src-tauri/vendor/wry` 是 crates.io 上 `wry 0.57.0` 的副本，只改了一处，用来修一个上游 bug。

**症状**：Android 冷启动偶发闪退，logcat 里是

```
E RustPanic: called `Result::unwrap()` on an `Err` value: "SendError(..)"
F libc: Fatal signal 6 (SIGABRT)
```

**成因**：`wry/src/android/mod.rs` 的 `platform_webview_version()` 用 `rx.recv_timeout(MAIN_PIPE_TIMEOUT)`
等主线程回包，`MAIN_PIPE_TIMEOUT = 10s`。冷启动首次加载 WebView provider 一旦超过 10 秒
（低端机、模拟器、系统正忙时都会），接收端被 drop，而主线程侧的 `main_pipe.rs` 仍然执行
`tx.send(Ok(version)).unwrap()`，必然拿到 `SendError` → panic → 整个进程被杀。
上游 0.57.1 也没修（改动只有 rustfmt 风格迁移）。

**处理**：把这 4 处 `tx.send(..).unwrap()` 改成 `let _ = tx.send(..)`（发送失败忽略即可），
并在 `src-tauri/Cargo.toml` 用 `[patch.crates-io]` 指向本地副本。补丁处均有 `// [本地补丁]` 注释。

> 这份副本会随仓库一起提交，CI 无需额外步骤；升级 wry 时先确认上游已修，再删掉 `vendor/wry` 与 `[patch.crates-io]`。

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

### 配置目录变更说明（v1.0.2）

v1.0.1 及更早版本使用 `…/tauri-sql-manager/`，v1.0.2 起改用
**基于应用 identifier** 的目录（这是修复移动端存储所必需的）：

| 版本 | 目录 |
|------|------|
| ≤ v1.0.1 | `~/Library/Application Support/tauri-sql-manager/` |
| ≥ v1.0.2 | `~/Library/Application Support/top.szzz666.taurisqlmanager/` |

升级后首次启动会**自动迁移**旧配置（仅复制，不删除原文件）。
若迁移失败，也可手动复制：

```bash
cp -r "$HOME/Library/Application Support/tauri-sql-manager/"* \
      "$HOME/Library/Application Support/top.szzz666.taurisqlmanager/"
```

强制指定目录（测试或多实例）：设置环境变量 `TSM_DATA_DIR`。

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