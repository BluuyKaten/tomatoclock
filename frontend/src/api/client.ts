/**
 * 类型化 Tauri API 客户端
 *
 * 严格对齐上游 §7 接口契约。所有调用走 invoke()，返回 ApiResponse<T>。
 */
import { invoke } from '@tauri-apps/api/core';

/** 通用响应信封 */
export interface ApiResponse<T> {
  code: number;
  msg: string;
  data: T;
}

/** 调用包装：自动解包 data，错误码非 0 抛异常 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  console.log(`[invoke] ${cmd}`, args);
  try {
    const res = await invoke<ApiResponse<T>>(cmd, args ?? {});
    console.log(`[invoke] ${cmd} ->`, res);
    if (res.code !== 0) {
      throw new ApiError(res.code, res.msg);
    }
    return res.data;
  } catch (e) {
    console.error(`[invoke] ${cmd} 失败:`, e);
    throw e;
  }
}

export class ApiError extends Error {
  code: number;
  constructor(code: number, message: string) {
    super(message);
    this.code = code;
    this.name = 'ApiError';
  }
}

// ===================== 账号 =====================
export interface AuthUser {
  user_id: number;
  username: string;
  created_at?: number;
}
export interface LoginResult {
  user_id: number;
  username: string;
  token?: string;
}
export interface AutoLoginResult {
  user_id: number;
  username: string;
  token: string;
}

export const authApi = {
  register: (username: string, password: string) =>
    call<AuthUser>('register', { req: { username, password } }),
  login: (username: string, password: string, rememberMe: boolean) =>
    call<LoginResult>('login', { req: { username, password, remember_me: rememberMe } }),
  autoLogin: (token: string) =>
    call<AutoLoginResult>('auto_login', { req: { token } }),
  logout: () => call<{ success: boolean }>('logout', {}),
};

// ===================== 番茄 =====================
export interface StartPomodoroResponse {
  id: number;
  started_at: number;
  planned_duration: number;
  status: number;
}
export interface PausePomodoroResponse {
  id: number;
  paused_at: number;
  accumulated_seconds: number;
}
export interface ResumePomodoroResponse {
  id: number;
  resumed_at: number;
}
export interface CompletePomodoroResponse {
  id: number;
  ended_at: number;
  actual_duration: number;
  distraction_count: number;
}
export interface CurrentPomodoroResponse {
  id: number;
  started_at: number;
  planned_duration: number;
  remaining_seconds: number;
  status: number;
  distraction_count: number;
}

export const pomodoroApi = {
  start: (task_id?: number, subject_id?: number, duration?: number) =>
    call<StartPomodoroResponse>('start_pomodoro', {
      req: { task_id: task_id ?? null, subject_id: subject_id ?? null, duration: duration ?? null },
    }),
  pause: () => call<PausePomodoroResponse>('pause_pomodoro', {}),
  resume: () => call<ResumePomodoroResponse>('resume_pomodoro', {}),
  complete: () => call<CompletePomodoroResponse>('complete_pomodoro', {}),
  abandon: (reason?: string) =>
    call<{ id: number; status: number }>('abandon_pomodoro', { req: { reason: reason ?? null } }),
  current: () => call<CurrentPomodoroResponse | null>('get_current_pomodoro', {}),
};

// ===================== 规则 =====================
export interface RuleView {
  id: number;
  rule_type: number;
  app_name: string;
  window_title_pattern: string | null;
  is_enabled: boolean;
}

export interface WindowInfo {
  app_name: string;
  window_title: string;
}

export const distractionApi = {
  // 枚举所有可见顶层窗口（按应用名去重），用于设置页「选择应用」对话框
  listTopWindows: () => call<{ windows: WindowInfo[] }>('list_top_windows', {}),
  listRules: () => call<{ rules: RuleView[] }>('list_rules', {}),
  createRule: (rule: {
    rule_type: number;
    app_name: string;
    window_title_pattern?: string;
    is_enabled?: boolean;
  }) => call<{ id: number }>('create_rule', { req: rule }),
  updateRule: (
    id: number,
    patch: {
      rule_type?: number;
      app_name?: string;
      window_title_pattern?: string | null;
      is_enabled?: boolean;
    }
  ) => call<{ id: number }>('update_rule', { id, req: patch }),
  deleteRule: (id: number) => call<{ id: number }>('delete_rule', { id }),
};

// ===================== 笔记 =====================
export interface NoteView {
  id: number;
  user_id: number;
  pomodoro_id: number | null;
  subject_id: number | null;
  title: string | null;
  content: string;
  tags: string[] | null;
  images: { id: number; file_path: string; mime_type: string | null; size_bytes: number | null }[];
  created_at: number;
  updated_at: number;
}

export const notesApi = {
  create: (data: {
    pomodoro_id?: number;
    subject_id?: number;
    title?: string;
    content: string;
    tags?: string[];
    image_paths?: string[];
  }) => call<{ id: number; created_at: number }>('create_note', { req: data }),
  list: (params?: {
    subject_id?: number;
    tag?: string;
    from?: number;
    to?: number;
    page?: number;
    page_size?: number;
  }) => call<{ total: number; items: NoteView[] }>('list_notes', { req: params ?? {} }),
  update: (
    id: number,
    patch: { title?: string; content?: string; tags?: string[] }
  ) => call<{ id: number; updated_at: number }>('update_note', { id, req: patch }),
  delete: (id: number) => call<{ id: number }>('delete_note', { id }),
};

// ===================== 统计 =====================
export interface OverviewResponse {
  total_minutes: number;
  completed_pomos: number;
  abandoned_pomos: number;
  distraction_count: number;
  distraction_rate: number;
  subject_distribution: { subject_id: number | null; name: string; minutes: number }[];
}
export interface TrendPoint {
  date: string;
  minutes: number;
  pomodoros: number;
  distractions: number;
}
export interface DistractionHotspotResponse {
  by_app: { app_name: string | null; count: number }[];
  by_hour: { hour: number; count: number }[];
  by_type: { type: number; count: number }[];
}
export interface InsightItem {
  type: string;
  severity: string;
  message: string;
}
export interface LlmSummaryResponse {
  summary: string;
  suggestions: string[];
}

export const statsApi = {
  overview: (from: number, to: number) =>
    call<OverviewResponse>('overview', { req: { from, to } }),
  trend: (from: number, to: number, granularity: 'day' | 'week' | 'month') =>
    call<{ points: TrendPoint[] }>('trend', { req: { from, to, granularity } }),
  distractionHotspot: (from: number, to: number) =>
    call<DistractionHotspotResponse>('distraction_hotspot', { req: { from, to } }),
  rulesSummary: (from: number, to: number) =>
    call<{ insights: InsightItem[] }>('rules_summary', { req: { from, to } }),
  llmSummary: (from: number, to: number, language?: string) =>
    call<LlmSummaryResponse>('llm_summary', { req: { from, to, language: language ?? null } }),
};

// ===================== 配置 =====================
export const settingsApi = {
  get: () => call<{ settings: Record<string, unknown> }>('get_settings', {}),
  update: (settings: Record<string, unknown>) =>
    call<{ updated_keys: string[] }>('update_settings', { req: { settings } }),
};
