import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  // Tauri 开发时固定端口，devUrl 里写的就是它
  server: {
    port: 5273,
    strictPort: true,
    host: '127.0.0.1'
  },
  build: {
    // Tauri 用系统 WebView2，目标可以放宽到现代浏览器
    target: 'chrome110',
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false
  }
})
