<script setup lang="ts">
/**
 * SQL 代码编辑器（CodeMirror 6）
 * 替代原项目通过 CDN 引入的 CodeMirror 5
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view'
import { EditorState, Compartment } from '@codemirror/state'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { sql, SQLite, MySQL, PostgreSQL, MSSQL, StandardSQL } from '@codemirror/lang-sql'
import { oneDark } from '@codemirror/theme-one-dark'
import { autocompletion, completionKeymap } from '@codemirror/autocomplete'
import { bracketMatching, indentOnInput } from '@codemirror/language'
import type { DbType } from '@/api/types'

const props = defineProps<{
  modelValue: string
  dbType?: DbType
  /** 编辑器主题 */
  dark?: boolean
  /** 表名与列名，用于自动补全 */
  schema?: Record<string, string[]>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'execute'): void
}>()

const host = ref<HTMLDivElement>()
let view: EditorView | null = null
const themeCompartment = new Compartment()
const dialectCompartment = new Compartment()

/** 根据数据库类型选择 SQL 方言 */
function sqlDialect(type?: DbType) {
  switch (type) {
    case 'mysql':
    case 'mariadb':
      return MySQL
    case 'postgresql':
      return PostgreSQL
    case 'mssql':
      return MSSQL
    case 'sqlite':
      return SQLite
    default:
      return StandardSQL
  }
}

/** 执行快捷键：Ctrl/Cmd + Enter */
const executeKeymap = keymap.of([
  {
    key: 'Mod-Enter',
    preventDefault: true,
    run: () => {
      emit('execute')
      return true
    },
  },
])

function currentTheme() {
  return props.dark ? [oneDark] : []
}

function buildExtension() {
  return [
    lineNumbers(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    history(),
    bracketMatching(),
    indentOnInput(),
    autocompletion(),
    keymap.of([...defaultKeymap, ...historyKeymap, ...completionKeymap, indentWithTab]),
    executeKeymap,
    EditorView.lineWrapping,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        emit('update:modelValue', update.state.doc.toString())
      }
    }),
    themeCompartment.of(currentTheme()),
    dialectCompartment.of(
      sql({
        dialect: sqlDialect(props.dbType),
        schema: props.schema,
        upperCaseKeywords: true,
      }),
    ),
  ]
}

function createEditor() {
  if (!host.value) return
  view = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: buildExtension(),
    }),
    parent: host.value,
  })
}

onMounted(createEditor)

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

// 外部修改内容时同步（避免自身更新导致的死循环）
watch(
  () => props.modelValue,
  (value) => {
    if (!view) return
    const current = view.state.doc.toString()
    if (value !== current) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      })
    }
  },
)

watch(
  () => props.dark,
  () => {
    view?.dispatch({
      effects: themeCompartment.reconfigure(currentTheme()),
    })
  },
)

watch(
  () => [props.dbType, props.schema],
  () => {
    view?.dispatch({
      effects: dialectCompartment.reconfigure(
        sql({
          dialect: sqlDialect(props.dbType),
          schema: props.schema,
          upperCaseKeywords: true,
        }),
      ),
    })
  },
  { deep: true },
)

/** 选中文本；无选区时返回全文 */
function getSelectionOrAll(): { text: string; from: number; to: number } {
  if (!view) return { text: props.modelValue, from: 0, to: props.modelValue.length }
  const { from, to } = view.state.selection.main
  if (from === to) {
    return { text: view.state.doc.toString(), from: 0, to: view.state.doc.length }
  }
  return { text: view.state.sliceDoc(from, to), from, to }
}

function focus() {
  view?.focus()
}

defineExpose({ focus, getSelectionOrAll })
</script>

<template>
  <div ref="host" class="sql-code-editor" />
</template>

<style scoped lang="scss">
.sql-code-editor {
  height: 100%;
  overflow: hidden;
  border-radius: 6px;
  border: 1px solid var(--el-border-color);
  background: var(--el-bg-color);

  :deep(.cm-editor) {
    height: 100%;
  }

  :deep(.cm-scroller) {
    overflow: auto;
  }
}
</style>
