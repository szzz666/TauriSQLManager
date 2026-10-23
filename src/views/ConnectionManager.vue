<script setup lang="ts">
/**
 * 连接管理：数据库连接的增删改查与测试
 * 对应原项目 components/connection-manager.js
 * 配置项由 JDBC URL 改为结构化字段（host / port / database）
 */
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import {
  Connection,
  Delete,
  Edit,
  Link,
  MoreFilled,
  Plus,
  Refresh,
  SwitchButton,
  Upload,
  User,
} from '@element-plus/icons-vue'
import { connections as connApi, dialects } from '@/api'
import type { ConnectionConfig, ConnectionInput, DbType } from '@/api/types'
import { DB_TYPES, DB_TYPE_META } from '@/stores/app'

const emit = defineEmits<{
  (e: 'select', conn: ConnectionConfig): void
  (e: 'disconnect', connId: string): void
}>()

const list = ref<ConnectionConfig[]>([])
const loading = ref(false)
const statusMap = ref<Record<string, { active: boolean; version?: string }>>({})

const dialogVisible = ref(false)
const editingId = ref('')
const saving = ref(false)
const testing = ref(false)
const formRef = ref<FormInstance>()

const columnTypes = ref<string[]>([])

const form = reactive<ConnectionInput>({
  name: '',
  type: 'mysql',
  host: 'localhost',
  port: 3306,
  database: '',
  username: '',
  password: '',
})

const isSqlite = computed(() => form.type === 'sqlite')

const rules: FormRules = {
  name: [{ required: true, message: '请输入连接名称', trigger: 'blur' }],
  type: [{ required: true, message: '请选择数据库类型', trigger: 'change' }],
  host: [{ required: true, message: '请输入主机地址', trigger: 'blur' }],
  database: [{ required: true, message: '请输入数据库名', trigger: 'blur' }],
}

const placeholder = computed(() => {
  switch (form.type) {
    case 'sqlite':
      return '点击右侧按钮选择 .db / .sqlite 文件'
    case 'mysql':
    case 'mariadb':
      return '例如：my_database'
    case 'postgresql':
      return '例如：postgres'
    case 'mssql':
      return '例如：master'
    default:
      return ''
  }
})

async function loadList() {
  loading.value = true
  try {
    list.value = await connApi.list()
    // 并行读取每个连接的运行状态
    const entries = await Promise.all(
      list.value.map(async (c) => {
        try {
          const s = await connApi.status(c.id)
          return [c.id, { active: s.active, version: s.version }] as const
        } catch {
          return [c.id, { active: false }] as const
        }
      }),
    )
    statusMap.value = Object.fromEntries(entries)
  } catch (e) {
    ElMessage.error('加载连接列表失败: ' + (e as Error).message)
  } finally {
    loading.value = false
  }
}

function resetForm() {
  Object.assign(form, {
    name: '',
    type: 'mysql',
    host: 'localhost',
    port: 3306,
    database: '',
    username: '',
    password: '',
  })
}

function openCreate() {
  editingId.value = ''
  resetForm()
  applyDefaultPort()
  dialogVisible.value = true
}

function openEdit(conn: ConnectionConfig) {
  editingId.value = conn.id
  Object.assign(form, {
    name: conn.name,
    type: conn.type,
    host: conn.host,
    port: conn.port,
    database: conn.database,
    // 密码不回显，留空表示不修改
    username: conn.username,
    password: '',
  })
  dialogVisible.value = true
}

/** 切换数据库类型时同步默认端口 */
async function onTypeChange(type: DbType) {
  try {
    form.port = await dialects.defaultPort(type)
    columnTypes.value = await dialects.columnTypes(type)
  } catch {
    // 忽略：端口有本地兜底
  }
}

function applyDefaultPort() {
  const defaults: Record<DbType, number> = {
    sqlite: 0,
    mysql: 3306,
    mariadb: 3306,
    postgresql: 5432,
    mssql: 1433,
  }
  form.port = defaults[form.type] ?? 0
}

async function pickSqliteFile() {
  try {
    const path = await connApi.pickSqliteFile()
    if (path) {
      form.database = path
      if (!form.name) {
        form.name = path.split('/').pop()?.replace(/\.(db|sqlite3?)$/i, '') ?? 'SQLite'
      }
    }
  } catch (e) {
    ElMessage.error('选择文件失败: ' + (e as Error).message)
  }
}

async function save() {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  saving.value = true
  try {
    if (editingId.value) {
      await connApi.update(editingId.value, { ...form })
      ElMessage.success('连接已更新')
    } else {
      await connApi.create({ ...form })
      ElMessage.success('连接已创建')
    }
    dialogVisible.value = false
    await loadList()
  } catch (e) {
    ElMessage.error((e as Error).message)
  } finally {
    saving.value = false
  }
}

async function testConnection() {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  testing.value = true
  try {
    const result = await connApi.test({ ...form })
    if (result.success) {
      ElMessage.success(`连接成功！耗时 ${result.elapsed}ms${result.version ? ' · ' + result.version : ''}`)
    } else {
      ElMessage.error('连接失败: ' + result.message)
    }
  } catch (e) {
    ElMessage.error((e as Error).message)
  } finally {
    testing.value = false
  }
}

async function handleCommand(cmd: string, conn: ConnectionConfig) {
  if (cmd === 'test') {
    try {
      const result = await connApi.testSaved(conn.id)
      if (result.success) {
        ElMessage.success(`连接成功！耗时 ${result.elapsed}ms`)
      } else {
        ElMessage.error('连接失败: ' + result.message)
      }
    } catch (e) {
      ElMessage.error((e as Error).message)
    }
  } else if (cmd === 'edit') {
    openEdit(conn)
  } else if (cmd === 'disconnect') {
    try {
      await connApi.disconnect(conn.id)
      ElMessage.success('连接已断开')
      emit('disconnect', conn.id)
      await loadList()
    } catch (e) {
      ElMessage.error((e as Error).message)
    }
  } else if (cmd === 'remove') {
    try {
      await ElMessageBox.confirm(`确定要删除连接「${conn.name}」吗？`, '确认删除', {
        type: 'warning',
        confirmButtonText: '删除',
        cancelButtonText: '取消',
      })
    } catch {
      return
    }
    try {
      await connApi.remove(conn.id)
      ElMessage.success('连接已删除')
      emit('disconnect', conn.id)
      await loadList()
    } catch (e) {
      ElMessage.error((e as Error).message)
    }
  }
}

function formatTime(ts: number) {
  if (!ts) return '-'
  return new Date(ts).toLocaleString('zh-CN')
}

function connectionSummary(conn: ConnectionConfig) {
  if (conn.type === 'sqlite') return conn.database
  return `${conn.host}:${conn.port}/${conn.database}`
}

onMounted(async () => {
  await loadList()
  try {
    columnTypes.value = await dialects.columnTypes('mysql')
  } catch {
    columnTypes.value = []
  }
})
</script>

<template>
  <div class="page-container">
    <div class="page-toolbar">
      <h2 class="page-title">数据库连接</h2>
      <div class="flex-1" />
      <el-button :icon="Refresh" circle :loading="loading" @click="loadList" />
      <el-button type="primary" :icon="Plus" @click="openCreate">新建连接</el-button>
    </div>

    <div v-loading="loading">
      <el-empty v-if="!loading && list.length === 0" description="暂无数据库连接，点击「新建连接」开始">
        <el-button type="primary" :icon="Plus" @click="openCreate">新建连接</el-button>
      </el-empty>

      <el-row v-else :gutter="14">
        <el-col v-for="conn in list" :key="conn.id" :xs="24" :sm="12" :md="8" :xl="6" class="card-col">
          <el-card class="conn-card" shadow="hover" :body-style="{ padding: '14px' }">
            <template #header>
              <div class="flex-between">
                <div class="flex-center gap-8 flex-1" style="overflow: hidden">
                  <el-icon :size="16" :color="DB_TYPE_META[conn.type].color">
                    <Connection />
                  </el-icon>
                  <span class="conn-name ellipsis" :title="conn.name">{{ conn.name }}</span>
                  <el-tooltip
                    :content="statusMap[conn.id]?.active ? '已连接' : '未连接'"
                    placement="top"
                  >
                    <span
                      class="status-dot"
                      :class="{ active: statusMap[conn.id]?.active }"
                    />
                  </el-tooltip>
                </div>
                <el-dropdown trigger="click" @command="(cmd: string) => handleCommand(cmd, conn)">
                  <el-button text :icon="MoreFilled" circle size="small" />
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item command="test" :icon="Link">测试连接</el-dropdown-item>
                      <el-dropdown-item command="edit" :icon="Edit">编辑</el-dropdown-item>
                      <el-dropdown-item command="disconnect" :icon="SwitchButton" divided>
                        断开连接
                      </el-dropdown-item>
                      <el-dropdown-item command="remove" :icon="Delete" style="color: var(--el-color-danger)">
                        删除
                      </el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </div>
            </template>

            <div class="conn-meta">
              <div class="meta-row">
                <el-tag size="small" :type="DB_TYPE_META[conn.type].tag as any">
                  {{ DB_TYPE_META[conn.type].label }}
                </el-tag>
                <el-tag v-if="statusMap[conn.id]?.version" size="small" type="info" class="ellipsis">
                  {{ statusMap[conn.id]?.version }}
                </el-tag>
              </div>
              <div class="meta-row mono ellipsis" :title="connectionSummary(conn)">
                {{ connectionSummary(conn) }}
              </div>
              <div v-if="conn.username" class="meta-row">
                <el-icon><User /></el-icon>
                <span class="ellipsis">{{ conn.username }}</span>
              </div>
              <div class="meta-row text-xs text-secondary">
                <el-icon><Refresh /></el-icon>
                {{ formatTime(conn.updatedAt) }}
              </div>
            </div>

            <el-button type="primary" class="enter-btn" @click="emit('select', conn)">
              进入管理
            </el-button>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 新建 / 编辑对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="editingId ? '编辑连接' : '新建连接'"
      width="560px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <el-form ref="formRef" :model="form" :rules="rules" label-width="92px">
        <el-form-item label="连接名称" prop="name">
          <el-input v-model="form.name" placeholder="例如：本地 MySQL" />
        </el-form-item>

        <el-form-item label="数据库类型" prop="type">
          <el-select v-model="form.type" style="width: 100%" @change="onTypeChange">
            <el-option
              v-for="t in DB_TYPES"
              :key="t"
              :label="DB_TYPE_META[t].label"
              :value="t"
            />
          </el-select>
        </el-form-item>

        <el-form-item v-if="!isSqlite" label="主机" prop="host">
          <div class="flex gap-8" style="width: 100%">
            <el-input v-model="form.host" placeholder="localhost" style="flex: 1" />
            <el-input-number
              v-model="form.port"
              :min="0"
              :max="65535"
              controls-position="right"
              style="width: 130px"
            />
          </div>
        </el-form-item>

        <el-form-item :label="isSqlite ? '数据库文件' : '数据库名'" prop="database">
          <div class="flex gap-8" style="width: 100%">
            <el-input v-model="form.database" :placeholder="placeholder" style="flex: 1" />
            <el-button v-if="isSqlite" :icon="Upload" @click="pickSqliteFile">选择</el-button>
          </div>
        </el-form-item>

        <template v-if="!isSqlite">
          <el-form-item label="用户名">
            <el-input v-model="form.username" placeholder="数据库用户名" />
          </el-form-item>
          <el-form-item label="密码">
            <el-input
              v-model="form.password"
              type="password"
              show-password
              :placeholder="editingId ? '留空则不修改密码' : '数据库密码'"
            />
          </el-form-item>
        </template>
      </el-form>

      <template #footer>
        <div class="flex gap-8" style="justify-content: flex-end">
          <el-button :icon="Link" :loading="testing" @click="testConnection">测试连接</el-button>
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" :loading="saving" @click="save">保存</el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped lang="scss">
.card-col {
  margin-bottom: 14px;
}

.conn-card {
  transition: transform 0.15s ease, box-shadow 0.15s ease;

  &:hover {
    transform: translateY(-2px);
  }

  :deep(.el-card__header) {
    padding: 10px 14px;
  }
}

.conn-name {
  font-weight: 600;
  font-size: 14px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--el-border-color-darker);
  flex-shrink: 0;

  &.active {
    background: var(--el-color-success);
    box-shadow: 0 0 0 3px var(--el-color-success-light-8);
  }
}

.conn-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-regular);
  min-height: 74px;
}

.meta-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.mono {
  font-family: 'SF Mono', Menlo, Monaco, monospace;
  color: var(--el-text-color-secondary);
}

.enter-btn {
  width: 100%;
  margin-top: 12px;
}
</style>
