// [FIX] 新增文件：Svelte + TypeScript 预处理配置
// 原因：Svelte 组件默认不支持 TypeScript，需通过 svelte-preprocess 桥接
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

const config = {
  // 启用 TypeScript 预处理，使 .svelte 文件支持 <script lang="ts">
  preprocess: vitePreprocess(),

  // 编译选项
  compilerOptions: {
    // 开发模式启用运行时检查
    dev: !process.env.PROD,
  },
};

export default config;
