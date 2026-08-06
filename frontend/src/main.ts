/**
 * 应用入口：挂载 Svelte 根组件
 */
import App from './App.svelte';
import './app.css';

const app = new App({
  target: document.getElementById('app')!,
});

export default app;
