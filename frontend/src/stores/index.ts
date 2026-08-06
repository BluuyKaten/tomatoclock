/**
 * 全局状态 store（Svelte writable）
 */
import { writable, derived, type Readable } from 'svelte/store';

export interface SessionUser {
  user_id: number;
  username: string;
}

/// 会话令牌在 localStorage 中的 key
const TOKEN_KEY = 'tomatoclock.session_token';
/// 记住的用户名在 localStorage 中的 key
const REMEMBERED_USERNAME_KEY = 'tomatoclock.remembered_username';

export const currentUser = writable<SessionUser | null>(null);

/** 持久化会话令牌到 localStorage */
export function persistToken(token: string | null) {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch (e) {
    console.warn('[store] 保存令牌失败：', e);
  }
}

/** 读取本地存储的会话令牌 */
export function loadPersistedToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch (e) {
    console.warn('[store] 读取令牌失败：', e);
    return null;
  }
}

/** 保存 / 读取记住的用户名 */
export function persistRememberedUsername(username: string | null) {
  try {
    if (username) localStorage.setItem(REMEMBERED_USERNAME_KEY, username);
    else localStorage.removeItem(REMEMBERED_USERNAME_KEY);
  } catch (e) {
    console.warn('[store] 保存用户名失败：', e);
  }
}
export function loadRememberedUsername(): string | null {
  try {
    return localStorage.getItem(REMEMBERED_USERNAME_KEY);
  } catch (e) {
    console.warn('[store] 读取用户名失败：', e);
    return null;
  }
}

export interface TimerState {
  id: number | null;
  remaining_seconds: number;
  planned_duration: number;
  status: number; // 0 进行中, 1 完成, 2 放弃, 3 打断
  distraction_count: number; // 当前番茄的实时分心次数
}

export const timer = writable<TimerState>({
  id: null,
  remaining_seconds: 0,
  planned_duration: 0,
  status: -1,
  distraction_count: 0,
});

// 番茄钟是否在进行中（status === 0）
export const isTimerRunning: Readable<boolean> = derived(
  timer,
  ($t) => $t.id !== null && $t.status === 0
);

// 派生：mm:ss 显示
export const timerDisplay: Readable<string> = derived(timer, ($t) => {
  const secs = Math.max(0, $t.remaining_seconds);
  const m = Math.floor(secs / 60).toString().padStart(2, '0');
  const s = (secs % 60).toString().padStart(2, '0');
  return `${m}:${s}`;
});

// 导航
export type Route = 'timer' | 'notes' | 'stats' | 'settings';
export const currentRoute = writable<Route>('timer');

// 全局消息
export interface Toast {
  kind: 'info' | 'error' | 'success';
  message: string;
}
export const toasts = writable<Toast[]>([]);
export function pushToast(t: Toast) {
  toasts.update((arr) => [...arr, t]);
  setTimeout(() => {
    toasts.update((arr) => arr.slice(1));
  }, 3000);
}
