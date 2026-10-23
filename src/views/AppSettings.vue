<script setup lang="ts">
/**
 * 应用设置：查询上限、超时、默认分页、数据目录
 */
import { onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { FolderOpened, Refresh, Check } from '@element-plus/icons-vue'
import { app as appApi } from '@/api'
import type { AppInfo, AppSettings } from '@/api/types'

const settings = reactive<AppSettings>({
  maxQueryRows: 10000,
  queryTimeout: 60,
  pageSize: 20,
})

const info = ref<AppInfo | null>(null)
const dataDir = ref('')
const loading = ref(false)
const saving = ref(false)

async function load() {
  loading.value = true
  try {
    const [s, i, dir] = await Promise.all([appApi.getSettings(), appApi.info(), appApi.dataDir()])
    Object.assign(settings, s)
    info.value = i
    dataDir.value = dir
  } catch (e) {
    ElMessage.error('读取设置失败: ' + (e as Error).message)
  } finally {
    loading.value = false
  }
}

async function save() {
  saving.value = true
  try {
    await appApi.saveSettings({ ...settings })
    ElMessage.success('设置已保存')
  } catch (e) {
    ElMessage.error('保存失败: ' + (e as Error).message)
  } finally {
    saving.value = false
  }
}

async function copyDataDir() {
  try {
    await navigator.clipboard.writeText(dataDir.value)
    ElMessage.success('路径已复制')
  } catch (e) {
    ElMessage.error('复制失败: ' + (e as Error).message)
  }
}

onMounted(load)
</script>

<template>
  <div class="page-container">
    <div class="page-toolbar">
      <h2 class="page-title">设置</h2>
      <div class="flex-1" />
      <el-button :icon="Refresh" circle :loading="loading" @click="load" />
      <el-button type="primary" :icon="Check" :loading="saving" @click="save">保存</el-button>
    </div>

    <el-card v-loading="loading" shadow="never" class="settings-card">
      <template #header>
        <span class="card-title">查询与展示</span>
      </template>
      <el-form label-width="160px" class="settings-form">
        <el-form-item label="单次查询最大行数">
          <el-input-number v-model="settings.maxQueryRows" :min="100" :max="1000000" :step="1000" controls-position="right" />
          <div class="hint">超出部分会被截断，避免大结果集占用过多内存</div>
        </el-form-item>

        <el-form-item label="查询超时（秒）">
          <el-input-number v-model="settings.queryTimeout" :min="0" :max="3600" controls-position="right" />
          <div class="hint">0 表示不限制</div>
        </el-form-item>

        <el-form-item label="默认每页行数">
          <el-input-number v-model="settings.pageSize" :min="10" :max="1000" controls-position="right" />
          <div class="hint">数据浏览页的默认分页大小</div>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card shadow="never" class="settings-card">
      <template #header>
        <span class="card-title">存储位置</span>
      </template>
      <el-form label-width="160px">
        <el-form-item label="配置目录">
          <div class="flex gap-8" style="width: 100%">
            <el-input :model-value="dataDir" readonly class="text-mono" />
            <el-button :icon="FolderOpened" @click="copyDataDir">复制路径</el-button>
          </div>
          <div class="hint">连接配置（包含密码）与设置保存在此目录，请妥善保管</div>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card v-if="info" shadow="never" class="settings-card">
      <template #header>
        <span class="card-title">关于</span>
      </template>
      <el-descriptions :column="2" border size="small">
        <el-descriptions-item label="应用名称">{{ info.name }}</el-descriptions-item>
        <el-descriptions-item label="版本">{{ info.version }}</el-descriptions-item>
        <el-descriptions-item label="支持数据库" :span="2">
          <el-tag
            v-for="(t, i) in info.supportedTypes"
            :key="t"
            size="small"
            style="margin-right: 6px"
          >
            {{ info.supportedTypeLabels?.[i] ?? t }}
          </el-tag>
        </el-descriptions-item>
      </el-descriptions>
    </el-card>
  </div>
</template>

<style scoped lang="scss">
.settings-card {
  max-width: 760px;
  margin-bottom: 14px;
  background: var(--el-bg-color);
}

.card-title {
  font-size: 14px;
  font-weight: 600;
}

.settings-form {
  padding-top: 4px;
}

.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.6;
  width: 100%;
}
</style>
