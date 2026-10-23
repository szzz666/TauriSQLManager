<script setup lang="ts">
/**
 * SQL 执行：编辑器 + 结果集 + 执行历史
 * 对应原项目 components/sql-editor.js，CodeMirror 6 替代 CodeMirror 5
 */
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Delete,
  DocumentCopy,
  Download,
  Files,
  Timer,
  VideoPlay,
} from '@element-plus/icons-vue'
import SQLCodeEditor from '@/components/SqlCodeEditor.vue'
import { sql as sqlApi, tables as tableApi } from '@/api'
import type { QueryResult } from '@/api/types'
import { state } from '@/stores/app'

const props = defineProps<{ connId: string }>()

const SQL_KEY_PREFIX = 'tsm.sql.draft.'

const editorRef = ref<InstanceType<typeof SQLCodeEditor>>()
const sqlText = ref('')
const executing = ref(false)
const results = ref<QueryResult[]>([])
const activeResult = ref('0')
const history = ref<{ sql: string; time: string; success: boolean; elapsed: number }[]>([])
const exporting = ref(false)

/** 表名 -> 列名，用于自动补全 */
const schema = ref<Record<string, string[]>>({})

/** 按当前数据库类型引用标识符 */
const quoteIdent = computed(() => {
  const t = state.activeConnection?.type
  if (t === 'mariadb') {
    return (n: string) => '\u0060' + n + '\u0060'
  }
  if (t === 'mysql') {
    return (n: string) => '\u0060' + n + '\u0060'
  }
  if (t === 'mssql') {
    return (n: string) => '[' + n + ']'
  }
  return (n: string) => '"' + n.replace(/"/g, '""') + '"'
})

const defaultSql = computed(
  () =>
    '-- 在此输入 SQL，Ctrl/Cmd + Enter 执行；选中部分代码时仅执行选中内容\n' +
    '-- 「批量执行」会按分号拆分为多条语句\n\n' +
    'SELECT * FROM ' +
    quoteIdent.value('your_table') +
    ' LIMIT 20;\n',
)

const activeResultData = computed(() => results.value[Number(activeResult.value)] ?? null)

async function loadSchema() {
  try {
    const list = await tableApi.list(props.connId)
    const entries = await Promise.all(
      list
        .filter((t) => t.tableType !== 'VIEW')
        .slice(0, 200)
        .map(async (t) => {
          try {
            const info = await tableApi.get(props.connId, t.name)
            return [t.name, info.columns.map((c) => c.name)] as const
          } catch {
            return [t.name, []] as const
          }
        }),
    )
    schema.value = Object.fromEntries(entries)
  } catch {
    schema.value = {}
  }
}

/** 取要执行的 SQL：优先选区，否则全文 */
function sqlToRun(): string {
  const sel = editorRef.value?.getSelectionOrAll()
  return (sel?.text ?? sqlText.value).trim()
}

async function execute() {
  const text = sqlToRun()
  if (!text) {
    ElMessage.warning('请输入 SQL 语句')
    return
  }
  executing.value = true
  try {
    const r = await sqlApi.execute(props.connId, text)
    results.value = [r]
    activeResult.value = '0'
    addHistory(text, true, r.elapsed)
    ElMessage.success(
      r.query
        ? '查询完成，返回 ' + r.returnedRows + ' 行，耗时 ' + r.elapsed + 'ms'
        : '执行完成，影响 ' + r.affectedRows + ' 行，耗时 ' + r.elapsed + 'ms',
    )
  } catch (e) {
    addHistory(text, false, 0)
    ElMessage.error('执行失败: ' + (e as Error).message)
  } finally {
    executing.value = false
  }
}

async function executeBatch() {
  const text = sqlToRun()
  if (!text) {
    ElMessage.warning('请输入 SQL 语句')
    return
  }
  executing.value = true
  try {
    const list = await sqlApi.batch(props.connId, text)
    results.value = list
    activeResult.value = '0'
    addHistory(text, true, list.reduce((sum, r) => sum + r.elapsed, 0))
    ElMessage.success('批量执行完成，共 ' + list.length + ' 条语句')
  } catch (e) {
    addHistory(text, false, 0)
    ElMessage.error('执行失败: ' + (e as Error).message)
  } finally {
    executing.value = false
  }
}

function addHistory(sql: string, success: boolean, elapsed: number) {
  history.value.unshift({
    sql: sql.length > 200 ? sql.slice(0, 200) + '…' : sql,
    time: new Date().toLocaleTimeString('zh-CN'),
    success,
    elapsed,
  })
  if (history.value.length > 30) history.value.length = 30
}

function loadFromHistory(sql: string) {
  sqlText.value = sql.endsWith('…') ? sql.slice(0, -1) : sql
  editorRef.value?.focus()
}

function clearEditor() {
  sqlText.value = ''
  results.value = []
}

async function copyResult() {
  const r = activeResultData.value
  if (!r?.query) return
  const lines = [r.columns.join('\t')]
  r.rows.forEach((row) => lines.push(row.map((v) => (v == null ? '' : String(v))).join('\t')))
  try {
    await navigator.clipboard.writeText(lines.join('\n'))
    ElMessage.success('已复制到剪贴板')
  } catch (e) {
    ElMessage.error('复制失败: ' + (e as Error).message)
  }
}

async function exportResult(format: 'csv' | 'json' | 'sql') {
  const r = activeResultData.value
  if (!r?.query) return
  exporting.value = true
  try {
    const path = await sqlApi.exportToFile(props.connId, r.sql, format)
    if (path) ElMessage.success('已导出到: ' + path)
  } catch (e) {
    ElMessage.error('导出失败: ' + (e as Error).message)
  } finally {
    exporting.value = false
  }
}

function arrayToObject(row: unknown[], cols: string[]) {
  const obj: Record<string, unknown> = {}
  cols.forEach((c, i) => {
    obj[c] = row[i]
  })
  return obj
}

function formatValue(v: unknown) {
  if (v === null || v === undefined) return 'NULL'
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}

function isNull(v: unknown) {
  return v === null || v === undefined
}

function resultTabLabel(r: QueryResult, idx: number) {
  return r.query
    ? '结果 ' + (idx + 1) + ' · ' + r.returnedRows + ' 行'
    : '结果 ' + (idx + 1) + ' · 影响 ' + r.affectedRows + ' 行'
}

function persistDraft() {
  try {
    localStorage.setItem(SQL_KEY_PREFIX + props.connId, sqlText.value)
  } catch {
    /* localStorage 不可用时忽略 */
  }
}

function restoreDraft() {
  try {
    const saved = localStorage.getItem(SQL_KEY_PREFIX + props.connId)
    sqlText.value = saved && saved.trim() ? saved : defaultSql.value
  } catch {
    sqlText.value = defaultSql.value
  }
}

watch(sqlText, persistDraft)

watch(
  () => props.connId,
  () => {
    results.value = []
    history.value = []
    restoreDraft()
    loadSchema()
  },
  { immediate: true },
)

onMounted(() => {
  if (!sqlText.value) restoreDraft()
  loadSchema()
})
</script>
<template>
  <div class="sql-page">
    <!-- 工具栏 -->
    <div class="page-toolbar">
      <h2 class="page-title">SQL 执行</h2>
      <div class="flex-1" />
      <el-tooltip content="执行（Ctrl/Cmd + Enter）" placement="bottom">
        <el-button type="primary" :icon="VideoPlay" :loading="executing" @click="execute">
          执行
        </el-button>
      </el-tooltip>
      <el-tooltip content="按分号拆分为多条语句并依次执行" placement="bottom">
        <el-button :icon="Files" :loading="executing" @click="executeBatch">批量执行</el-button>
      </el-tooltip>
      <el-button :icon="Delete" circle @click="clearEditor" />
    </div>

    <!-- 编辑器 -->
    <div class="editor-area">
      <SQLCodeEditor
        ref="editorRef"
        v-model="sqlText"
        :db-type="state.activeConnection?.type"
        :dark="state.theme === 'dark'"
        :schema="schema"
        @execute="execute"
      />
    </div>

    <!-- 结果区 -->
    <div v-if="results.length > 0" class="result-area">
      <el-tabs v-model="activeResult" class="result-tabs">
        <el-tab-pane
          v-for="(r, idx) in results"
          :key="idx"
          :label="resultTabLabel(r, idx)"
          :name="String(idx)"
        >
          <template v-if="r.query">
            <div class="result-meta">
              <span class="flex-center gap-4">
                <el-icon><Timer /></el-icon>{{ r.elapsed }}ms
              </span>
              <span>{{ r.returnedRows }} 行</span>
              <span v-if="r.truncated" class="warn-text">结果已截断</span>
              <div class="flex-1" />
              <el-button link size="small" :icon="DocumentCopy" @click="copyResult">
                复制
              </el-button>
              <el-dropdown trigger="click" @command="exportResult">
                <el-button link size="small" :icon="Download" :loading="exporting">导出</el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item command="csv">CSV</el-dropdown-item>
                    <el-dropdown-item command="json">JSON</el-dropdown-item>
                    <el-dropdown-item command="sql">SQL INSERT</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>

            <el-table
              :data="r.rows.map((row) => arrayToObject(row, r.columns))"
              border
              stripe
              size="small"
              max-height="320"
              class="data-table"
            >
              <el-table-column
                v-for="(col, i) in r.columns"
                :key="i"
                :prop="col"
                min-width="130"
                show-overflow-tooltip
              >
                <template #header>
                  <div class="col-header">
                    <span>{{ col }}</span>
                    <span class="col-type">{{ r.columnTypes[i] }}</span>
                  </div>
                </template>
                <template #default="{ row }">
                  <span v-if="isNull(row[col])" class="null-value">NULL</span>
                  <span v-else>{{ formatValue(row[col]) }}</span>
                </template>
              </el-table-column>
            </el-table>
          </template>

          <el-result
            v-else
            icon="success"
            :title="'影响 ' + r.affectedRows + ' 行'"
            :sub-title="'耗时 ' + r.elapsed + 'ms'"
          />
        </el-tab-pane>
      </el-tabs>
    </div>

    <!-- 执行历史 -->
    <el-collapse v-if="history.length > 0" class="history-area">
      <el-collapse-item name="history">
        <template #title>
          <span class="text-xs">执行历史（{{ history.length }}）</span>
        </template>
        <div class="history-list">
          <div v-for="(h, i) in history" :key="i" class="history-row">
            <el-tag size="small" :type="h.success ? 'success' : 'danger'" effect="plain">
              {{ h.success ? '成功' : '失败' }}
            </el-tag>
            <span class="text-xs text-secondary">{{ h.time }}</span>
            <span class="history-sql ellipsis text-mono text-xs">{{ h.sql }}</span>
            <el-button link size="small" @click="loadFromHistory(h.sql)">载入</el-button>
          </div>
        </div>
      </el-collapse-item>
    </el-collapse>
  </div>
</template>

<style scoped lang="scss">
.sql-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 12px 16px;
  overflow: hidden;
}

.editor-area {
  flex: 1;
  min-height: 160px;
  overflow: hidden;
}

.result-area {
  flex-shrink: 0;
  max-height: 46vh;
  overflow: auto;
  margin-top: 10px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.result-tabs {
  :deep(.el-tabs__header) {
    margin-bottom: 6px;
  }
}

.result-meta {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 4px 2px 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.warn-text {
  color: var(--el-color-warning);
}

.col-header {
  display: flex;
  flex-direction: column;
  line-height: 1.3;

  .col-type {
    font-size: 10px;
    font-weight: 400;
    color: var(--el-text-color-placeholder);
  }
}

.history-area {
  flex-shrink: 0;
  border-top: 1px solid var(--el-border-color-lighter);
  margin-top: 8px;

  :deep(.el-collapse-item__header) {
    height: 32px;
    line-height: 32px;
  }

  :deep(.el-collapse-item__content) {
    padding-bottom: 8px;
  }
}

.history-list {
  max-height: 140px;
  overflow-y: auto;
}

.history-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 0;
  border-bottom: 1px dashed var(--el-border-color-lighter);
}

.history-sql {
  flex: 1;
  min-width: 0;
}
</style>
