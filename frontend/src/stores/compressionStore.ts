import { create } from 'zustand';

interface CompressionState {
  isCompressing: boolean;
  progress: number;
  contextRatio: number;
  setCompressionStatus: (status: { isCompressing: boolean; progress: number; contextRatio: number }) => void;
}

export const useCompressionStore = create<CompressionState>((set) => ({
  isCompressing: false,
  progress: 0,
  contextRatio: 0,
  setCompressionStatus: (status) => set(status),
}));
