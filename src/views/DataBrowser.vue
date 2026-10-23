<script setup lang="ts">
/**
 * 数据浏览：分页查看、条件过滤、增删改、导入导出
 * 对应原项目 components/data-browser.js
 */
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Download,
  Edit,
  Filter,
  Plus,
  Refresh,
  Search,
  Upload,
} from '@element-plus/icons-vue'
import { data as dataApi, sql as sqlApi, tables as tableApi } from '@/api'
import type { TableData, TableInfo } from '@/api/types'

const props = defineProps<{ connId: string; initialTable?: string }>()

const tables = ref<TableInfo[]>([])
const selectedTable = ref('')
const loading = ref(false)
const tableInfo = ref<TableInfo | null>(null)

const rows = ref<Record<string, unknown>[]>([])
const columns = ref<string[]>([])
const total = ref(0)
const page = ref(1)
const pageSize = ref(20)

const whereInput = ref('')
const appliedWhere = ref('')
const sortColumn = ref('')
const sortOrder = ref<'ASC' | 'DESC'>('ASC')

const selectedRows = ref<Record<string, unknown>[]>([])

// ===== 行编辑对话框 =====
const formVisible = ref(false)
const formMode = ref<'insert' | 'update'>('insert')
const recordForm = reactive<Record<string, unknown>>({})
const saving = ref(false)
const originalRow = ref<Record<string, unknown>>({})
/** 当前表的主键列，无主键时回退到全部列 */
const pkColumns = computed(() => tableInfo.value?.primaryKeys ?? [])

// ===== 导入 =====
const importVisible = ref(false)
const importFormat = ref<'csv' | 'json'>('csv')
const importMode = ref<'insert' | 'ignore' | 'replace'>('ignore')
const importContent = ref('')
const importing = ref(false)
const fileName = ref('')

// ===== 导出 =====
const exporting = ref(false)

/** 导入框的占位示例文案 */
const importPlaceholder = computed(() =>
  importFormat.value === 'csv'
    ? '第一行为列名，例如：\nid,name\n1,Alice'
    : '[{"id": 1, "name": "Alice"}]',
)

async function loadTables() {
  try {
    tables.value = await tableApi.list(props.connId)
    if (props.initialTable && tables.value.some((t) => t.name === props.initialTable)) {
      selectedTable.value = props.initialTable
    } else if (tables.value.length > 0 && !selectedTable.value) {
      selectedTable.value = tables.value[0].name
    }
    if (selectedTable.value) {
      await loadData()
    }
  } catch (e) {
    ElMessage.error('加载表列表失败: ' + (e as Error).message)
  }
}

async function ensureTableInfo() {
  if (!selectedTable.value) return null
  if (tableInfo.value?.name === selectedTable.value) return tableInfo.value
  try {
    tableInfo.value = await tableApi.get(props.connId, selectedTable.value)
    return tableInfo.value
  } catch (e) {
    ElMessage.error('加载表结构失败: ' + (e as Error).message)
    return null
  }
}

async function loadData() {
  if (!props.connId || !selectedTable.value) return
  loading.value = true
  selectedRows.value = []
  try {
    const result: TableData = await dataApi.query(props.connId, selectedTable.value, {
      page: page.value,
      pageSize: pageSize.value,
      where: appliedWhere.value || undefined,
      sortColumn: sortColumn.value || undefined,
      sortOrder: sortColumn.value ? sortOrder.value : undefined,
    })
    rows.value = result.rows
    columns.value = result.columns
    total.value = result.total
    page.value = result.page
    await ensureTableInfo()
  } catch (e) {
    ElMessage.error('加载数据失败: ' + (e as Error).message)
    rows.value = []
    columns.value = []
    total.value = 0
  } finally {
    loading.value = false
  }
}

function onTableChange() {
  page.value = 1
  appliedWhere.value = ''
  whereInput.value = ''
  sortColumn.value = ''
  tableInfo.value = null
  loadData()
}

function applyWhere() {
  appliedWhere.value = whereInput.value.trim()
  page.value = 1
  loadData()
}

function resetWhere() {
  whereInput.value = ''
  appliedWhere.value = ''
  page.value = 1
  loadData()
}

function onSortChange({ prop, order }: { prop: string; order: string | null }) {
  if (!order) {
    sortColumn.value = ''
  } else {
    sortColumn.value = prop
    sortOrder.value = order === 'ascending' ? 'ASC' : 'DESC'
  }
  page.value = 1
  loadData()
}

function onPageChange(p: number) {
  page.value = p
  loadData()
}

function onSizeChange(size: number) {
  pageSize.value = size
  page.value = 1
  loadData()
}

// ===== 新增 / 编辑 =====
async function openInsert() {
  const info = await ensureTableInfo()
  if (!info) return
  formMode.value = 'insert'
  Object.keys(recordForm).forEach((k) => delete recordForm[k])
  info.columns.forEach((col) => {
    // 自增列与带 SQL 表达式的默认值不预填，交由数据库处理
    const dv = col.defaultValue || ''
    const isExpr = /^(CURRENT_TIMESTAMP|CURRENT_DATE|CURRENT_TIME|NOW\(|UUID\(|RAND\(|nextval)/i.test(dv)
    recordForm[col.name] = col.autoIncrement || isExpr ? '' : ''
  })
  formVisible.value = true
}

async function openEdit(row: Record<string, unknown>) {
  const info = await ensureTableInfo()
  if (!info) return
  formMode.value = 'update'
  Object.keys(recordForm).forEach((k) => delete recordForm[k])
  Object.assign(recordForm, row)
  originalRow.value = { ...row }
  formVisible.value = true
}

function buildPk(row: Record<string, unknown>) {
  const pk: Record<string, unknown> = {}
  const cols = pkColumns.value.length > 0 ? pkColumns.value : Object.keys(row)
  cols.forEach((c) => {
    pk[c] = row[c]
  })
  return pk
}

async function saveRecord() {
  saving.value = true
  try {
    if (formMode.value === 'insert') {
      // 只提交非空字段，让数据库使用默认值
      const payload: Record<string, unknown> = {}
      for (const [k, v] of Object.entries(recordForm)) {
        if (v !== '' && v !== null && v !== undefined) payload[k] = v
      }
      if (Object.keys(payload).length === 0) {
        ElMessage.warning('请至少填写一个字段')
        return
      }
      await dataApi.insert(props.connId, selectedTable.value, payload)
      ElMessage.success('记录已添加')
    } else {
      const changed: Record<string, unknown> = {}
      for (const [k, v] of Object.entries(recordForm)) {
        if (String(originalRow.value[k] ?? '') !== String(v ?? '')) {
          changed[k] = v === '' ? null : v
        }
      }
      if (Object.keys(changed).length === 0) {
        ElMessage.info('没有修改')
        formVisible.value = false
        return
      }
      await dataApi.updateByPk(
        props.connId,
        selectedTable.value,
        changed,
        buildPk(originalRow.value),
      )
      ElMessage.success('记录已更新')
    }
    formVisible.value = false
    await loadData()
  } catch (e) {
    ElMessage.error((e as Error).message)
  } finally {
    saving.value = false
  }
}

async function deleteRow(row: Record<string, unknown>) {
  try {
    await ElMessageBox.confirm('确定要删除这条记录吗？', '确认删除', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await dataApi.deleteByPk(props.connId, selectedTable.value, buildPk(row))
    ElMessage.success('记录已删除')
    await loadData()
  } catch (e) {
    ElMessage.error('删除失败: ' + (e as Error).message)
  }
}

async function batchDelete() {
  if (selectedRows.value.length === 0) return
  try {
    await ElMessageBox.confirm(
      `确定要删除选中的 ${selectedRows.value.length} 条记录吗？`,
      '批量删除',
      { type: 'warning', confirmButtonText: '确认删除', cancelButtonText: '取消' },
    )
  } catch {
    return
  }
  loading.value = true
  let ok = 0
  let fail = 0
  for (const row of selectedRows.value) {
    try {
      await dataApi.deleteByPk(props.connId, selectedTable.value, buildPk(row))
      ok++
    } catch {
      fail++
    }
  }
  loading.value = false
  if (fail === 0) {
    ElMessage.success(`删除完成：成功 ${ok} 条`)
  } else {
    ElMessage.warning(`删除完成：成功 ${ok} 条，失败 ${fail} 条`)
  }
  await loadData()
}

// ===== 导入 / 导出 =====
function openImport() {
  importFormat.value = 'csv'
  importMode.value = 'ignore'
  importContent.value = ''
  fileName.value = ''
  importVisible.value = true
}

async function onFilePick(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  try {
    importContent.value = await file.text()
    fileName.value = file.name
  } catch (err) {
    ElMessage.error('读取文件失败: ' + (err as Error).message)
  }
  input.value = ''
}

async function submitImport() {
  if (!importContent.value.trim()) {
    ElMessage.warning('请选择文件或粘贴数据')
    return
  }
  importing.value = true
  try {
    const r = await dataApi.import(
      props.connId,
      selectedTable.value,
      importFormat.value,
      importContent.value,
      importMode.value,
    )
    ElMessage.success(`导入完成：共 ${r.totalRows} 行，影响 ${r.affectedRows} 行`)
    importVisible.value = false
    await loadData()
  } catch (e) {
    ElMessage.error('导入失败: ' + (e as Error).message)
  } finally {
    importing.value = false
  }
}

async function exportTable(format: 'csv' | 'json' | 'sql') {
  if (!selectedTable.value) return
  exporting.value = true
  try {
    const dialectQuote = /^\w+$/.test(selectedTable.value)
      ? `"${selectedTable.value}"`
      : `"${selectedTable.value.replace(/"/g, '""')}"`
    const path = await sqlApi.exportToFile(
      props.connId,
      `SELECT * FROM ${dialectQuote}`,
      format,
      selectedTable.value,
    )
    if (path) {
      ElMessage.success('已导出到: ' + path)
    }
  } catch (e) {
    ElMessage.error('导出失败: ' + (e as Error).message)
  } finally {
    exporting.value = false
  }
}

// ===== 单元格渲染 =====
function isNull(v: unknown) {
  return v === null || v === undefined
}

function displayValue(v: unknown): string {
  if (isNull(v)) return 'NULL'
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}

function isLongText(v: unknown): boolean {
  return typeof v === 'string' && v.length > 80
}

watch(
  () => props.connId,
  () => {
    selectedTable.value = ''
    tableInfo.value = null
    loadTables()
  },
  { immediate: true },
)

watch(
  () => props.initialTable,
  (v) => {
    if (v && v !== selectedTable.value) {
      selectedTable.value = v
      page.value = 1
      appliedWhere.value = ''
      whereInput.value = ''
      loadData()
    }
  },
)
</script>

<template>
  <div class="data-page">
    <!-- 工具栏 -->
    <div class="page-toolbar">
      <el-select
        v-model="selectedTable"
        placeholder="选择表"
        filterable
        style="width: 240px"
        @change="onTableChange"
      >
        <el-option v-for="t in tables" :key="t.name" :label="t.name" :value="t.name" />
      </el-select>

      <el-input
        v-model="whereInput"
        placeholder="WHERE 条件，例如 id > 100 AND status = 1"
        clearable
        :prefix-icon="Filter"
        class="where-input"
        @keyup.enter="applyWhere"
      />
      <el-button :icon="Search" @click="applyWhere">查询</el-button>
      <el-button v-if="appliedWhere" text @click="resetWhere">清除条件</el-button>

      <div class="flex-1" />

      <el-button :icon="Refresh" circle :loading="loading" @click="loadData" />
      <el-dropdown trigger="click" :disabled="exporting" @command="exportTable">
        <el-button :icon="Download" :loading="exporting">导出</el-button>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item command="csv">导出为 CSV</el-dropdown-item>
            <el-dropdown-item command="json">导出为 JSON</el-dropdown-item>
            <el-dropdown-item command="sql">导出为 SQL</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
      <el-button :icon="Upload" @click="openImport">导入</el-button>
      <el-button type="primary" :icon="Plus" :disabled="!selectedTable" @click="openInsert">
        新增记录
      </el-button>
    </div>

    <!-- 批量操作条 -->
    <div v-if="selectedRows.length > 0" class="batch-bar">
      <span>已选择 {{ selectedRows.length }} 条记录</span>
      <el-button size="small" type="danger" plain @click="batchDelete">批量删除</el-button>
    </div>

    <!-- 数据表格 -->
    <el-table
      v-loading="loading"
      :data="rows"
      border
      stripe
      size="small"
      height="calc(100vh - 196px)"
      class="data-table"
      @selection-change="(rows: Record<string, unknown>[]) => (selectedRows = rows)"
      @sort-change="onSortChange"
    >
      <el-table-column type="selection" width="42" fixed="left" />

      <el-table-column
        v-for="col in columns"
        :key="col"
        :prop="col"
        :label="col"
        :sortable="pkColumns.includes(col) ? 'custom' : false"
        min-width="140"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span v-if="isNull(row[col])" class="null-value">NULL</span>
          <span v-else-if="isLongText(row[col])" class="text-mono">{{ displayValue(row[col]) }}</span>
          <span v-else>{{ displayValue(row[col]) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="操作" width="120" align="center" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" size="small" :icon="Edit" @click="openEdit(row)">
            编辑
          </el-button>
          <el-button link type="danger" size="small" @click="deleteRow(row)">删除</el-button>
        </template>
      </el-table-column>

      <template #empty>
        <el-empty :description="selectedTable ? '该表暂无数据' : '请选择要浏览的表'" />
      </template>
    </el-table>

    <!-- 分页 -->
    <div class="pagination-bar">
      <span class="text-xs text-secondary">
        共 {{ total.toLocaleString('zh-CN') }} 行 ·
        {{ columns.length }} 列
        <template v-if="appliedWhere"> · 已应用条件</template>
      </span>
      <el-pagination
        v-model:current-page="page"
        v-model:page-size="pageSize"
        :total="total"
        :page-sizes="[20, 50, 100, 200, 500]"
        layout="sizes, prev, pager, next, jumper"
        size="small"
        background
        @current-change="onPageChange"
        @size-change="onSizeChange"
      />
    </div>

    <!-- 行编辑对话框 -->
    <el-dialog
      v-model="formVisible"
      :title="formMode === 'insert' ? '新增记录' : '编辑记录'"
      width="600px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <el-form label-width="140px" label-position="right" class="record-form">
        <el-form-item
          v-for="(_, col) in recordForm"
          :key="col"
          :label="String(col)"
        >
          <el-input
            v-model="recordForm[col] as any"
            :type="String(recordForm[col] ?? '').length > 60 ? 'textarea' : 'text'"
            :rows="3"
            :placeholder="pkColumns.includes(String(col)) ? '主键' : ''"
            :disabled="formMode === 'update' && pkColumns.includes(String(col))"
          />
        </el-form-item>
        <el-empty v-if="Object.keys(recordForm).length === 0" description="请先加载表结构" />
      </el-form>
      <template #footer>
        <el-button @click="formVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="saveRecord">保存</el-button>
      </template>
    </el-dialog>

    <!-- 导入对话框 -->
    <el-dialog v-model="importVisible" title="导入数据" width="640px" :close-on-click-modal="false">
      <el-form label-width="92px">
        <el-form-item label="目标表">
          <el-tag>{{ selectedTable }}</el-tag>
        </el-form-item>
        <el-form-item label="数据格式">
          <el-radio-group v-model="importFormat">
            <el-radio-button value="csv">CSV</el-radio-button>
            <el-radio-button value="json">JSON</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="冲突处理">
          <el-radio-group v-model="importMode">
            <el-radio-button value="ignore">跳过</el-radio-button>
            <el-radio-button value="replace">覆盖</el-radio-button>
            <el-radio-button value="insert">报错</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="数据文件">
          <div class="flex gap-8" style="width: 100%">
            <label class="file-picker">
              <input type="file" :accept="importFormat === 'csv' ? '.csv,text/csv' : '.json,application/json'" @change="onFilePick" />
              <el-button :icon="Upload">选择文件</el-button>
            </label>
            <span v-if="fileName" class="text-xs text-secondary">{{ fileName }}</span>
          </div>
        </el-form-item>
        <el-form-item label="或粘贴内容">
          <el-input v-model="importContent" type="textarea" :rows="8" :placeholder="importPlaceholder" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="importVisible = false">取消</el-button>
        <el-button type="primary" :loading="importing" @click="submitImport">开始导入</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped lang="scss">
.data-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 12px 16px;
  overflow: hidden;
}

.where-input {
  width: 300px;
}

.batch-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 12px;
  margin-bottom: 8px;
  font-size: 12px;
  background: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary-light-7);
  border-radius: var(--app-radius);
  color: var(--el-color-primary);
}

.pagination-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 8px;
  flex-shrink: 0;
}

.file-picker {
  input {
    display: none;
  }
}

.record-form {
  max-height: 55vh;
  overflow-y: auto;
  padding-right: 8px;
}
</style>
