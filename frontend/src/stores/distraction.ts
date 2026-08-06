//! 分心弹窗状态 store
//! 当后端检测到分心窗口时，isDistracted=true，前端弹出模态提醒。

import { writable } from 'svelte/store';

export interface DistractionState {
  isDistracted: boolean;
  appName: string | null;
  windowTitle: string | null;
}

function createDistractionStore() {
  const { subscribe, set } = writable<DistractionState>({
    isDistracted: false,
    appName: null,
    windowTitle: null,
  });

  return {
    subscribe,
    set,
    reset: () => set({ isDistracted: false, appName: null, windowTitle: null }),
  };
}

export const distractionStore = createDistractionStore();
