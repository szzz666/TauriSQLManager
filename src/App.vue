<script setup lang="ts">
/**
 * 应用根组件：顶栏 + 侧边栏 + 主内容区
 * 对应原项目 app.js 的布局结构，去掉了 Web 登录页
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Connection,
  DataBoard,
  EditPen,
  Expand,
  Fold,
  Grid,
  Menu,
  Setting,
  SwitchButton,
} from '@element-plus/icons-vue'
import { app as appApi, connections as connApi } from '@/api'
import type { AppInfo } from '@/api/types'
import {
  DB_TYPE_META,
  browseData,
  clearConnection,
  initViewportWatcher,
  onDisconnected,
  selectConnection,
  setMenu,
  state,
  toggleMobileDrawer,
} from '@/stores/app'
import ConnectionManager from '@/views/ConnectionManager.vue'
import TableExplorer from '@/views/TableExplorer.vue'
import DataBrowser from '@/views/DataBrowser.vue'
import SqlEditor from '@/views/SqlEditor.vue'
import AppSettings from '@/views/AppSettings.vue'

const info = ref<AppInfo | null>(null)

const activeConn = computed(() => state.activeConnection)
const typeMeta = computed(() =>
  activeConn.value ? DB_TYPE_META[activeConn.value.type] : null,
)

let disposeViewport: (() => void) | null = null

onMounted(async () => {
  disposeViewport = initViewportWatcher()
  try {
    info.value = await appApi.info()
  } catch (e) {
    ElMessage.error('读取应用信息失败: ' + (e as Error).message)
  }
  await restoreSession()
})

onBeforeUnmount(() => {
  disposeViewport?.()
})

/**
 * 恢复上次的会话：自动重新选中连接并回到上次的页面，
 * 避免每次启动都要重新点一遍。
 */
async function restoreSession() {
  try {
    const [settings, list] = await Promise.all([appApi.getSettings(), connApi.list()])
    const lastId = settings.lastConnectionId
    if (!lastId) return
    const conn = list.find((c) => c.id === lastId)
    if (!conn) return
    state.activeConnection = conn
    const menu = settings.lastMenu
    // tables/data/sql 依赖连接；connections/settings 无需连接也可恢复
    const needsConnection = ['tables', 'data', 'sql']
    const allowed = ['connections', 'tables', 'data', 'sql', 'settings']
    if (!menu || !allowed.includes(menu)) {
      state.activeMenu = 'tables'
    } else if (needsConnection.includes(menu)) {
      state.activeMenu = menu as typeof state.activeMenu
    } else {
      state.activeMenu = menu as typeof state.activeMenu
    }
  } catch {
    // 恢复失败不影响正常使用
  }
}

function toggleSidebar() {
  state.sidebarCollapsed = !state.sidebarCollapsed
}

// 连接或页面变化时记录会话，供下次启动恢复
watch(
  () => [state.activeConnection?.id ?? null, state.activeMenu] as const,
  ([id, menu]) => {
    appApi.saveSession(id, menu).catch(() => {
      /* 记录失败不影响使用 */
    })
  },
)

function handleSelectConnection(conn: Parameters<typeof selectConnection>[0]) {
  selectConnection(conn)
}

function handleDisconnect(connId: string) {
  onDisconnected(connId)
}

function goConnections() {
  setMenu('connections')
}

function toggleTheme() {
  state.theme = state.theme === 'light' ? 'dark' : 'light'
  document.documentElement.classList.toggle('dark', state.theme === 'dark')
}
</script>

<template>
  <div class="app-layout">
    <!-- 顶部栏 -->
    <header class="app-header">
      <div class="flex-center gap-12">
        <el-button
          text
          :icon="state.isMobile ? Menu : state.sidebarCollapsed ? Expand : Fold"
          @click="state.isMobile ? toggleMobileDrawer() : toggleSidebar()"
        />
        <div class="logo" @click="goConnections">
          <span class="logo-icon">🗄️</span>
          <span class="logo-text">Tauri SQL Manager</span>
        </div>
      </div>

      <div class="flex-center gap-12">
        <el-tag
          v-if="activeConn && typeMeta"
          :type="typeMeta.tag as any"
          effect="light"
          closable
          @close="clearConnection"
        >
          {{ activeConn.name }} · {{ typeMeta.label }}
        </el-tag>
        <el-tooltip :content="state.theme === 'light' ? '切换到深色' : '切换到浅色'">
          <el-button text circle @click="toggleTheme">
            {{ state.theme === 'light' ? '🌙' : '☀️' }}
          </el-button>
        </el-tooltip>
        <el-tooltip v-if="!state.isMobile" :content="'应用版本 ' + (info?.version ?? '-')">
          <span class="version text-xs text-secondary">v{{ info?.version ?? '1.0.0' }}</span>
        </el-tooltip>
      </div>
    </header>

    <div class="app-body">
      <!-- 侧边栏 -->
      <!-- 移动端遮罩 -->
      <div
        v-if="state.isMobile && state.mobileDrawerOpen"
        class="mobile-overlay"
        @click="state.mobileDrawerOpen = false"
      />
      <aside
        class="app-sidebar"
        :class="{
          collapsed: state.sidebarCollapsed && !state.isMobile,
          'mobile-open': state.isMobile && state.mobileDrawerOpen,
        }"
      >
        <el-menu
          :default-active="state.activeMenu"
          :collapse="state.sidebarCollapsed && !state.isMobile"
          :collapse-transition="false"
          class="sidebar-menu"
          @select="(key: string) => setMenu(key as any)"
        >
          <el-menu-item index="connections">
            <el-icon><Connection /></el-icon>
            <template #title>连接管理</template>
          </el-menu-item>

          <template v-if="activeConn">
            <el-menu-item index="tables">
              <el-icon><Grid /></el-icon>
              <template #title>数据表</template>
            </el-menu-item>
            <el-menu-item index="data">
              <el-icon><DataBoard /></el-icon>
              <template #title>数据浏览</template>
            </el-menu-item>
            <el-menu-item index="sql">
              <el-icon><EditPen /></el-icon>
              <template #title>SQL 执行</template>
            </el-menu-item>
          </template>

          <el-menu-item index="settings">
            <el-icon><Setting /></el-icon>
            <template #title>设置</template>
          </el-menu-item>
        </el-menu>

        <div v-if="!state.sidebarCollapsed" class="sidebar-footer">
          <template v-if="activeConn">
            <div class="ellipsis text-xs" :title="activeConn.name">
              {{ activeConn.name }}
            </div>
            <div class="text-xs text-secondary ellipsis">
              {{ activeConn.type === 'sqlite' ? activeConn.database : `${activeConn.host}:${activeConn.port}` }}
            </div>
          </template>
          <div v-else class="text-xs text-secondary">未选择连接</div>
        </div>
      </aside>

      <!-- 主内容区 -->
      <main class="app-main">
        <ConnectionManager
          v-if="state.activeMenu === 'connections'"
          @select="handleSelectConnection"
          @disconnect="handleDisconnect"
        />
        <TableExplorer
          v-else-if="state.activeMenu === 'tables' && activeConn"
          :key="activeConn.id"
          :conn-id="activeConn.id"
          @browse="browseData"
        />
        <DataBrowser
          v-else-if="state.activeMenu === 'data' && activeConn"
          :conn-id="activeConn.id"
          :initial-table="state.browseTable"
        />
        <SqlEditor v-else-if="state.activeMenu === 'sql' && activeConn" :conn-id="activeConn.id" />
        <AppSettings v-else-if="state.activeMenu === 'settings'" />
        <el-empty v-else description="请先在「连接管理」中选择一个数据库" />
      </main>
    </div>

    <!-- 移动端底部标签栏 -->
    <nav v-if="state.isMobile" class="mobile-tabbar">
      <button
        class="tab-item"
        :class="{ active: state.activeMenu === 'connections' }"
        @click="setMenu('connections')"
      >
        <el-icon><Connection /></el-icon>
        <span>连接</span>
      </button>
      <button
        class="tab-item"
        :class="{ active: state.activeMenu === 'tables' }"
        :disabled="!activeConn"
        @click="setMenu('tables')"
      >
        <el-icon><Grid /></el-icon>
        <span>表</span>
      </button>
      <button
        class="tab-item"
        :class="{ active: state.activeMenu === 'data' }"
        :disabled="!activeConn"
        @click="setMenu('data')"
      >
        <el-icon><DataBoard /></el-icon>
        <span>数据</span>
      </button>
      <button
        class="tab-item"
        :class="{ active: state.activeMenu === 'sql' }"
        :disabled="!activeConn"
        @click="setMenu('sql')"
      >
        <el-icon><EditPen /></el-icon>
        <span>SQL</span>
      </button>
      <button
        class="tab-item"
        :class="{ active: state.activeMenu === 'settings' }"
        @click="setMenu('settings')"
      >
        <el-icon><Setting /></el-icon>
        <span>设置</span>
      </button>
    </nav>
  </div>
</template>

<style scoped lang="scss">
.app-layout {
  /* 用 100dvh 适配移动端浏览器地址栏/工具栏的伸缩 */
  height: 100vh;
  height: 100dvh;
  display: flex;
  flex-direction: column;
  background: var(--el-bg-color-page);
}

.app-header {
  height: var(--app-header-height);
  flex-shrink: 0;
  padding: 0 12px 0 8px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--el-bg-color);
  border-bottom: 1px solid var(--el-border-color-light);
  /* 移动端：为状态栏/刘海留出空间，避免标题被遮挡 */
  padding-top: env(safe-area-inset-top, 0);
  /* 顶部安全区占位后，高度需要相应增加 */
  box-sizing: content-box;
}

.logo {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;

  .logo-icon {
    font-size: 18px;
    line-height: 1;
  }

  .logo-text {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.2px;
  }
}

.version {
  user-select: none;
}

.app-body {
  flex: 1;
  /* flex 子项默认 min-height:auto，会导致内容撑破容器、遮挡底部标签栏 */
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.app-sidebar {
  width: var(--app-sidebar-width);
  flex-shrink: 0;
  background: var(--el-bg-color);
  border-right: 1px solid var(--el-border-color-light);
  display: flex;
  flex-direction: column;
  transition: width 0.2s ease;

  &.collapsed {
    width: var(--app-sidebar-collapsed);
  }
}

.sidebar-menu {
  flex: 1;
  border-right: none;
  overflow-y: auto;
  overflow-x: hidden;
  padding-top: 6px;

  &.el-menu--collapse {
    width: var(--app-sidebar-collapsed);
  }
}

.sidebar-footer {
  flex-shrink: 0;
  padding: 10px 14px;
  border-top: 1px solid var(--el-border-color-lighter);
  line-height: 1.6;
  min-height: 52px;
}

.app-main {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  background: var(--el-bg-color-page);
}

/* ===== 移动端适配 ===== */

/* 遮罩层 */
.mobile-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  z-index: 1000;
}

/* 移动端底部标签栏 */
.mobile-tabbar {
  flex-shrink: 0;
  position: relative;
  z-index: 10;
  display: flex;
  align-items: stretch;
  height: 54px;
  background: var(--el-bg-color);
  border-top: 1px solid var(--el-border-color-light);
  /* iOS 底部横条 / Android 手势条：用 content-box + padding 撑开高度，
     否则按钮会被系统手势区盖住 */
  padding-bottom: env(safe-area-inset-bottom, 0);
  box-sizing: content-box;
}

.tab-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  border: none;
  background: transparent;
  color: var(--el-text-color-secondary);
  font-size: 11px;
  cursor: pointer;
  padding: 0;
  transition: color 0.15s;

  .el-icon {
    font-size: 19px;
  }

  &.active {
    color: var(--el-color-primary);
  }

  &:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  &:active:not(:disabled) {
    background: var(--el-fill-color-light);
  }
}

@media (max-width: 768px) {
  /* 移动端：侧边栏变成抽屉 */
  .app-sidebar {
    position: fixed;
    top: 0;
    bottom: 0;
    left: 0;
    z-index: 1001;
    width: 250px !important;
    transform: translateX(-100%);
    transition: transform 0.25s ease;
    box-shadow: 2px 0 12px rgba(0, 0, 0, 0.15);
    /* 抽屉覆盖全屏，需要自己避让状态栏 */
    padding-top: env(safe-area-inset-top, 0);
    padding-bottom: env(safe-area-inset-bottom, 0);
    box-sizing: border-box;

    &.mobile-open {
      transform: translateX(0);
    }
  }

  /* 移动端：顶栏固定品牌蓝。
     必须和 MainActivity.kt 里的状态栏底色（#2563EB）完全一致：
     Android 15 起系统强制 edge-to-edge，状态栏那一条由原生层用这个蓝色填充，
     两者不同色就会露出一条明显的色带。
     所以这里不做深色主题切换（深色下顶栏仍是品牌蓝）。 */
  .app-header {
    padding: 0 8px;
    /* iOS 上 WebView 会伸到状态栏下方，这里补回安全区；
       Android 由 MainActivity 在原生层补 padding，env() 返回 0，不会重复。 */
    padding-top: env(safe-area-inset-top, 0);
    background: #2563EB;
    border-bottom-color: transparent;
    color: #fff;

    :deep(.el-button) {
      color: #fff;
    }

    :deep(.el-icon) {
      color: #fff;
    }

    :deep(.logo-text) {
      color: #fff;
    }
  }

  .logo-text {
    font-size: 14px;
  }

  /* 主内容区底部留出标签栏高度，避免内容被遮挡 */
  .app-main {
    padding-bottom: 0;
    overflow: auto;
  }

  /* 移动端紧凑间距 */
  .page-container {
    padding: 12px 10px;
  }
}

</style>