import { create } from 'zustand';

interface UIState {
  leftDrawerOpen: boolean;
  rightDrawerOpen: boolean;
  logOpen: boolean;
  currentModel: string;
  toggleLeftDrawer: () => void;
  toggleRightDrawer: () => void;
  toggleLog: () => void;
}

export const useUIStore = create<UIState>((set) => ({
  leftDrawerOpen: true,
  rightDrawerOpen: false,
  logOpen: false,
  currentModel: 'unknown',
  toggleLeftDrawer: () => set((state) => ({ leftDrawerOpen: !state.leftDrawerOpen })),
  toggleRightDrawer: () => set((state) => ({ rightDrawerOpen: !state.rightDrawerOpen })),
  toggleLog: () => set((state) => ({ logOpen: !state.logOpen })),
}));
