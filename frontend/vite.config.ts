import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

// Tauri 开发服务器配置
// dev 时由 tauri.conf.json 的 beforeDevCommand 启动；build 输出到 dist/
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@lib': resolve(__dirname, 'src/lib')
    }
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // 允许 Tauri 开发服务器跨域
    hmr: {
      protocol: 'ws',
      host: 'localhost'
    }
  },
  build: {
    target: ['es2022', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      output: {
        // 避免大文件hash导致Tauri加载问题
        manualChunks: undefined
      }
    }
  }
});
