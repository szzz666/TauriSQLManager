<script setup lang="ts">
/**
 * 表结构管理：表列表、列结构、索引，以及建表/改表/删表
 * 对应原项目 components/table-explorer.js
 */
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Delete,
  Edit,
  Grid,
  Key,
  Plus,
  Refresh,
  Search,
  View,
} from '@element-plus/icons-vue'
import { dialects, tables as tableApi } from '@/api'
import type { ColumnDefinition, ColumnInfo, IndexInfo, TableInfo } from '@/api/types'
import { state } from '@/stores/app'

const props = defineProps<{ connId: string }>()
const emit = defineEmits<{ (e: 'browse', table: string): void }>()

const tables = ref<TableInfo[]>([])
const loading = ref(false)
const keyword = ref('')

const filtered = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  if (!kw) return tables.value
  return tables.value.filter((t) => t.name.toLowerCase().includes(kw))
})

// ===== 表结构抽屉 =====
const drawerVisible = ref(false)
const structureLoading = ref(false)
const activeTab = ref('columns')
const current = ref<TableInfo | null>(null)
const indexes = ref<IndexInfo[]>([])
const currentTable = ref('')

// ===== 建表对话框 =====
const createVisible = ref(false)
const creating = ref(false)
const newTable = reactive<{ name: string; comment: string; columns: ColumnDefinition[] }>({
  name: '',
  comment: '',
  columns: [],
})
const columnTypes = ref<string[]>([])

// ===== 加列 / 改列对话框 =====
const columnVisible = ref(false)
const columnSaving = ref(false)
const columnEditing = ref(false)
const columnForm = reactive<ColumnDefinition>(blankColumn())

function blankColumn(): ColumnDefinition {
  return {
    name: '',
    type: 'VARCHAR',
    length: 255,
    scale: 0,
    nullable: true,
    autoIncrement: false,
    primaryKey: false,
    unique: false,
    defaultValue: '',
    comment: '',
  }
}

async function loadTables() {
  loading.value = true
  try {
    tables.value = await tableApi.list(props.connId)
  } catch (e) {
    ElMessage.error('加载表列表失败: ' + (e as Error).message)
  } finally {
    loading.value = false
  }
}

async function openStructure(table: string) {
  currentTable.value = table
  drawerVisible.value = true
  activeTab.value = 'columns'
  structureLoading.value = true
  try {
    const [info, idx] = await Promise.all([
      tableApi.get(props.connId, table),
      tableApi.indexes(props.connId, table).catch(() => []),
    ])
    current.value = info
    indexes.value = idx
  } catch (e) {
    ElMessage.error('加载表结构失败: ' + (e as Error).message)
  } finally {
    structureLoading.value = false
  }
}

async function reloadStructure() {
  if (currentTable.value) {
    await openStructure(currentTable.value)
  }
  await loadTables()
}

// ===== 建表 =====
async function openCreate() {
  newTable.name = ''
  newTable.comment = ''
  newTable.columns = [
    { ...blankColumn(), name: 'id', type: 'INT', length: 0, primaryKey: true, autoIncrement: true, nullable: false },
    { ...blankColumn(), name: 'name', type: 'VARCHAR', length: 255 },
  ]
  try {
    columnTypes.value = await dialects.columnTypes(state.activeConnection?.type ?? 'mysql')
  } catch {
    columnTypes.value = []
  }
  createVisible.value = true
}

function addColumnRow() {
  newTable.columns.push(blankColumn())
}

async function submitCreate() {
  if (!newTable.name.trim()) {
    ElMessage.warning('请输入表名')
    return
  }
  if (newTable.columns.length === 0) {
    ElMessage.warning('至少需要定义一列')
    return
  }
  const invalid = newTable.columns.find((c) => !c.name.trim())
  if (invalid) {
    ElMessage.warning('存在未命名的列')
    return
  }
  creating.value = true
  try {
    await tableApi.create(props.connId, newTable.name.trim(), newTable.columns as ColumnDefinition[], newTable.comment || undefined)
    ElMessage.success('表创建成功')
    createVisible.value = false
    await loadTables()
  } catch (e) {
    ElMessage.error('创建失败: ' + (e as Error).message)
  } finally {
    creating.value = false
  }
}

// ===== 删表 / 清空 =====
async function confirmDrop(table: string) {
  try {
    await ElMessageBox.confirm(
      `确定要删除表「${table}」吗？该操作不可恢复。`,
      '删除表',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' },
    )
  } catch {
    return
  }
  try {
    await tableApi.drop(props.connId, table)
    ElMessage.success('表已删除')
    await loadTables()
  } catch (e) {
    ElMessage.error('删除失败: ' + (e as Error).message)
  }
}

async function confirmTruncate(table: string) {
  try {
    await ElMessageBox.confirm(`确定要清空表「${table}」的所有数据吗？`, '清空表', {
      type: 'warning',
      confirmButtonText: '清空',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await tableApi.truncate(props.connId, table)
    ElMessage.success('表已清空')
    await loadTables()
  } catch (e) {
    ElMessage.error('清空失败: ' + (e as Error).message)
  }
}

async function renameTable() {
  if (!current.value) return
  let newName = ''
  try {
    const result = await ElMessageBox.prompt('请输入新表名', '重命名表', {
      inputValue: current.value.name,
      inputValidator: (v: string) => (v && v.trim() ? true : '表名不能为空'),
      confirmButtonText: '确定',
      cancelButtonText: '取消',
    })
    newName = result.value.trim()
  } catch {
    return
  }
  try {
    await tableApi.rename(props.connId, current.value.name, newName)
    ElMessage.success('表已重命名')
    currentTable.value = newName
    await reloadStructure()
  } catch (e) {
    ElMessage.error('重命名失败: ' + (e as Error).message)
  }
}

// ===== 列操作 =====
async function openAddColumn() {
  Object.assign(columnForm, blankColumn())
  columnEditing.value = false
  try {
    columnTypes.value = await dialects.columnTypes(state.activeConnection?.type ?? 'mysql')
  } catch {
    /* ignore */
  }
  columnVisible.value = true
}

function openModifyColumn(col: ColumnInfo) {
  columnEditing.value = true
  Object.assign(columnForm, {
    name: col.name,
    type: col.type,
    length: col.length,
    scale: col.scale,
    nullable: col.nullable,
    autoIncrement: col.autoIncrement,
    primaryKey: col.primaryKey,
    unique: col.unique,
    defaultValue: col.defaultValue || '',
    comment: col.comment || '',
  })
  columnTypes.value = columnTypes.value.length ? columnTypes.value : []
  columnVisible.value = true
}

async function submitColumn() {
  if (!columnForm.name.trim()) {
    ElMessage.warning('请输入列名')
    return
  }
  columnSaving.value = true
  try {
    if (columnEditing.value) {
      await tableApi.modifyColumn(props.connId, currentTable.value, { ...columnForm })
      ElMessage.success('列已修改')
    } else {
      await tableApi.addColumn(props.connId, currentTable.value, { ...columnForm })
      ElMessage.success('列已添加')
    }
    columnVisible.value = false
    await reloadStructure()
  } catch (e) {
    ElMessage.error((e as Error).message)
  } finally {
    columnSaving.value = false
  }
}

async function confirmDropColumn(col: ColumnInfo) {
  try {
    await ElMessageBox.confirm(`确定要删除列「${col.name}」吗？该列数据将丢失。`, '删除列', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await tableApi.dropColumn(props.connId, currentTable.value, col.name)
    ElMessage.success('列已删除')
    await reloadStructure()
  } catch (e) {
    ElMessage.error('删除失败: ' + (e as Error).message)
  }
}

function formatNumber(n?: number) {
  if (n == null) return '0'
  return Number(n).toLocaleString('zh-CN')
}

/** 列的完整类型展示，例如 VARCHAR(255) */
function fullType(col: ColumnInfo): string {
  let s = (col.type || '').toUpperCase()
  if (col.length > 0) {
    s += `(${col.length}`
    if (col.scale > 0) s += `,${col.scale}`
    s += ')'
  }
  return s
}

onMounted(loadTables)
</script>

<template>
  <div class="page-container">
    <div class="page-toolbar">
      <h2 class="page-title">数据表</h2>
      <el-input
        v-model="keyword"
        placeholder="搜索表名"
        :prefix-icon="Search"
        clearable
        style="width: 220px"
      />
      <div class="flex-1" />
      <el-button :icon="Refresh" circle :loading="loading" @click="loadTables" />
      <el-button type="primary" :icon="Plus" @click="openCreate">新建表</el-button>
    </div>

    <el-table
      v-loading="loading"
      :data="filtered"
      border
      stripe
      height="calc(100vh - 160px)"
      @row-dblclick="(row: TableInfo) => openStructure(row.name)"
    >
      <el-table-column prop="name" label="表名" min-width="240" show-overflow-tooltip>
        <template #default="{ row }">
          <div class="flex-center gap-8">
            <el-icon :color="row.tableType === 'VIEW' ? 'var(--el-color-warning)' : 'var(--el-color-primary)'">
              <View v-if="row.tableType === 'VIEW'" />
              <Grid v-else />
            </el-icon>
            <span class="table-name">{{ row.name }}</span>
          </div>
        </template>
      </el-table-column>

      <el-table-column label="类型" width="90" align="center">
        <template #default="{ row }">
          <el-tag size="small" :type="row.tableType === 'VIEW' ? 'warning' : 'success'">
            {{ row.tableType === 'VIEW' ? '视图' : '表' }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column label="行数" width="110" align="right">
        <template #default="{ row }">{{ formatNumber(row.rowCount) }}</template>
      </el-table-column>

      <el-table-column prop="comment" label="备注" min-width="160" show-overflow-tooltip>
        <template #default="{ row }">
          <span :class="{ 'text-secondary': !row.comment }">{{ row.comment || '-' }}</span>
        </template>
      </el-table-column>

      <el-table-column label="操作" width="270" align="center" fixed="right">
        <template #default="{ row }">
          <el-button size="small" @click="emit('browse', row.name)">数据</el-button>
          <el-button size="small" @click="openStructure(row.name)">结构</el-button>
          <el-button size="small" type="warning" plain @click="confirmTruncate(row.name)">清空</el-button>
          <el-button size="small" type="danger" plain @click="confirmDrop(row.name)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 表结构抽屉 -->
    <el-drawer v-model="drawerVisible" size="72%" destroy-on-close>
      <template #header>
        <div class="flex-center gap-12">
          <span style="font-size: 15px; font-weight: 600">表结构</span>
          <el-tag size="small" type="info">{{ currentTable }}</el-tag>
        </div>
      </template>

      <div v-loading="structureLoading" class="structure-body">
        <div class="flex gap-8" style="margin-bottom: 12px">
          <el-button size="small" :icon="Edit" @click="renameTable">重命名表</el-button>
          <div class="flex-1" />
          <el-button v-if="activeTab === 'columns'" size="small" type="primary" :icon="Plus" @click="openAddColumn">
            添加列
          </el-button>
        </div>

        <el-tabs v-model="activeTab">
          <el-tab-pane label="列信息" name="columns">
            <el-table :data="current?.columns ?? []" border stripe size="small" height="calc(100vh - 280px)">
              <el-table-column prop="ordinal" label="#" width="52" align="center" />
              <el-table-column label="列名" min-width="160" show-overflow-tooltip>
                <template #default="{ row }">
                  <div class="flex-center gap-4">
                    <el-icon v-if="row.primaryKey" color="var(--el-color-warning)" :size="13">
                      <Key />
                    </el-icon>
                    <span :style="{ fontWeight: row.primaryKey ? 600 : 400 }">{{ row.name }}</span>
                  </div>
                </template>
              </el-table-column>
              <el-table-column label="类型" width="180" show-overflow-tooltip>
                <template #default="{ row }">
                  <code class="type-code">{{ fullType(row) }}</code>
                </template>
              </el-table-column>
              <el-table-column label="可空" width="70" align="center">
                <template #default="{ row }">
                  <el-tag size="small" :type="row.nullable ? 'info' : 'danger'">
                    {{ row.nullable ? '是' : '否' }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="自增" width="70" align="center">
                <template #default="{ row }">
                  <el-tag v-if="row.autoIncrement" size="small" type="warning">是</el-tag>
                  <span v-else class="text-secondary">-</span>
                </template>
              </el-table-column>
              <el-table-column prop="defaultValue" label="默认值" width="150" show-overflow-tooltip>
                <template #default="{ row }">
                  <span :class="{ 'text-secondary': !row.defaultValue }">
                    {{ row.defaultValue || '-' }}
                  </span>
                </template>
              </el-table-column>
              <el-table-column prop="comment" label="注释" min-width="120" show-overflow-tooltip>
                <template #default="{ row }">
                  <span :class="{ 'text-secondary': !row.comment }">{{ row.comment || '-' }}</span>
                </template>
              </el-table-column>
              <el-table-column label="操作" width="120" align="center" fixed="right">
                <template #default="{ row }">
                  <el-button link type="primary" size="small" @click="openModifyColumn(row)">修改</el-button>
                  <el-button link type="danger" size="small" @click="confirmDropColumn(row)">删除</el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>

          <el-tab-pane label="索引" name="indexes">
            <el-table :data="indexes" border stripe size="small" height="calc(100vh - 280px)">
              <el-table-column prop="name" label="索引名" min-width="180" show-overflow-tooltip />
              <el-table-column prop="columnName" label="列名" min-width="150" show-overflow-tooltip />
              <el-table-column label="类型" width="100" align="center">
                <template #default="{ row }">
                  <el-tag v-if="row.primaryKey" size="small" type="warning">主键</el-tag>
                  <el-tag v-else-if="row.unique" size="small" type="success">唯一</el-tag>
                  <el-tag v-else size="small" type="info">普通</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="ascOrDesc" label="排序" width="80" align="center">
                <template #default="{ row }">{{ row.ascOrDesc === 'D' ? '降序' : '升序' }}</template>
              </el-table-column>
              <el-table-column prop="ordinal" label="序号" width="80" align="center" />
            </el-table>
            <el-empty v-if="indexes.length === 0" description="该表没有显式索引" />
          </el-tab-pane>

          <el-tab-pane label="表信息" name="info">
            <el-descriptions :column="2" border>
              <el-descriptions-item label="表名">{{ current?.name }}</el-descriptions-item>
              <el-descriptions-item label="类型">{{ current?.tableType }}</el-descriptions-item>
              <el-descriptions-item label="行数">
                {{ formatNumber(current?.rowCount) }}
              </el-descriptions-item>
              <el-descriptions-item label="列数">
                {{ current?.columns?.length ?? 0 }}
              </el-descriptions-item>
              <el-descriptions-item label="主键">
                {{ (current?.primaryKeys ?? []).join(', ') || '无' }}
              </el-descriptions-item>
              <el-descriptions-item label="备注">{{ current?.comment || '无' }}</el-descriptions-item>
              <el-descriptions-item label="创建时间">
                {{ current?.createTime || '-' }}
              </el-descriptions-item>
              <el-descriptions-item label="更新时间">
                {{ current?.updateTime || '-' }}
              </el-descriptions-item>
            </el-descriptions>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-drawer>

    <!-- 新建表 -->
    <el-dialog v-model="createVisible" title="新建表" width="900px" :close-on-click-modal="false">
      <el-form label-width="82px">
        <el-form-item label="表名" required>
          <el-input v-model="newTable.name" placeholder="例如：users" style="width: 320px" />
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="newTable.comment" placeholder="表注释（可选）" style="width: 480px" />
        </el-form-item>
        <el-form-item label="列定义" required>
          <div style="width: 100%">
            <el-table :data="newTable.columns" border size="small">
              <el-table-column label="列名" min-width="130">
                <template #default="{ row }">
                  <el-input v-model="row.name" size="small" placeholder="column_name" />
                </template>
              </el-table-column>
              <el-table-column label="类型" width="140">
                <template #default="{ row }">
                  <el-select v-model="row.type" size="small" filterable allow-create default-first-option>
                    <el-option v-for="t in columnTypes" :key="t" :label="t" :value="t" />
                  </el-select>
                </template>
              </el-table-column>
              <el-table-column label="长度" width="90">
                <template #default="{ row }">
                  <el-input-number
                    v-model="row.length"
                    size="small"
                    :min="0"
                    controls-position="right"
                    style="width: 100%"
                  />
                </template>
              </el-table-column>
              <el-table-column label="可空" width="56" align="center">
                <template #default="{ row }">
                  <el-checkbox v-model="row.nullable" />
                </template>
              </el-table-column>
              <el-table-column label="主键" width="56" align="center">
                <template #default="{ row }">
                  <el-checkbox v-model="row.primaryKey" />
                </template>
              </el-table-column>
              <el-table-column label="自增" width="56" align="center">
                <template #default="{ row }">
                  <el-checkbox v-model="row.autoIncrement" />
                </template>
              </el-table-column>
              <el-table-column label="默认值" width="110">
                <template #default="{ row }">
                  <el-input v-model="row.defaultValue" size="small" />
                </template>
              </el-table-column>
              <el-table-column label="操作" width="60" align="center">
                <template #default="{ $index }">
                  <el-button
                    link
                    type="danger"
                    :icon="Delete"
                    @click="newTable.columns.splice($index, 1)"
                  />
                </template>
              </el-table-column>
            </el-table>
            <el-button size="small" :icon="Plus" style="width: 100%; margin-top: 8px" @click="addColumnRow">
              添加列
            </el-button>
          </div>
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="createVisible = false">取消</el-button>
        <el-button type="primary" :loading="creating" @click="submitCreate">创建</el-button>
      </template>
    </el-dialog>

    <!-- 添加 / 修改列 -->
    <el-dialog
      v-model="columnVisible"
      :title="columnEditing ? '修改列' : '添加列'"
      width="520px"
      :close-on-click-modal="false"
    >
      <el-form label-width="86px">
        <el-form-item label="列名" required>
          <el-input v-model="columnForm.name" :disabled="columnEditing" />
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="columnForm.type" filterable allow-create default-first-option style="width: 100%">
            <el-option v-for="t in columnTypes" :key="t" :label="t" :value="t" />
          </el-select>
        </el-form-item>
        <el-form-item label="长度 / 小数">
          <div class="flex gap-8">
            <el-input-number v-model="columnForm.length" :min="0" controls-position="right" />
            <el-input-number v-model="columnForm.scale" :min="0" controls-position="right" />
          </div>
        </el-form-item>
        <el-form-item label="默认值">
          <el-input v-model="columnForm.defaultValue" placeholder="留空表示无默认值" />
        </el-form-item>
        <el-form-item label="注释">
          <el-input v-model="columnForm.comment" placeholder="列注释（MySQL 支持）" />
        </el-form-item>
        <el-form-item label="属性">
          <div class="flex gap-16">
            <el-checkbox v-model="columnForm.nullable">允许 NULL</el-checkbox>
            <el-checkbox v-model="columnForm.primaryKey">主键</el-checkbox>
            <el-checkbox v-model="columnForm.autoIncrement">自增</el-checkbox>
            <el-checkbox v-model="columnForm.unique">唯一</el-checkbox>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="columnVisible = false">取消</el-button>
        <el-button type="primary" :loading="columnSaving" @click="submitColumn">确定</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped lang="scss">
.table-name {
  font-weight: 500;
}

.type-code {
  font-family: 'SF Mono', Menlo, Monaco, monospace;
  font-size: 12px;
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  padding: 1px 6px;
  border-radius: 4px;
}

.structure-body {
  height: 100%;
}
</style>
