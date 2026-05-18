import { create } from 'zustand';
import { ProviderItem } from '../types/api';

interface UIState {
  leftDrawerOpen: boolean;
  rightDrawerOpen: boolean;
  logOpen: boolean;
  themeName: string;
  providers: ProviderItem[];
  currentModel: string;
  toggleLeftDrawer: () => void;
  toggleRightDrawer: () => void;
  toggleLog: () => void;
  setThemeName: (name: string) => void;
  setProviders: (providers: ProviderItem[]) => void;
  setCurrentModel: (model: string) => void;
}

export const useUIStore = create<UIState>((set) => ({
  leftDrawerOpen: true,
  rightDrawerOpen: false,
  logOpen: false,
  themeName: 'deep_ocean',
  providers: [],
  currentModel: 'unknown',
  toggleLeftDrawer: () => set((s) => ({ leftDrawerOpen: !s.leftDrawerOpen })),
  toggleRightDrawer: () => set((s) => ({ rightDrawerOpen: !s.rightDrawerOpen })),
  toggleLog: () => set((s) => ({ logOpen: !s.logOpen })),
  setThemeName: (name) => set({ themeName: name }),
  setProviders: (providers) => set({ providers }),
  setCurrentModel: (model) => set({ currentModel: model }),
}));
