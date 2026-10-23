import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// Tauri 期望固定端口，并在 start 时清理旧输出，避免二次启动失败
const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  // Vite 选项供 Tauri CLI 使用
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      // 避免监听 Rust 侧文件
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // Windows 上默认使用 Chromium，而 macOS/Linux 使用 WebKit
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    // Vite 8 基于 rolldown，默认使用内置压缩器
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    chunkSizeWarningLimit: 1500,
    // 拆分体积较大的第三方依赖，避免单一 chunk 过大
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            { name: 'element-plus', test: /node_modules[\\/]element-plus/ },
            { name: 'codemirror', test: /node_modules[\\/]@?codemirror|node_modules[\\/]@lezer/ },
            { name: 'vendor', test: /node_modules[\\/](vue|@vue|@tauri-apps)/ },
          ],
        },
      },
    },
  },
})
