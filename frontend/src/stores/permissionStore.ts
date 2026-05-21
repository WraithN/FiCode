import { create } from 'zustand';

export interface PermissionAskItem {
  toolCallId: string;
  toolName: string;
  risk: string;
  reason: string;
}

interface PermissionState {
  pending: PermissionAskItem | null;
  setPending: (item: PermissionAskItem | null) => void;
}

export const usePermissionStore = create<PermissionState>((set) => ({
  pending: null,
  setPending: (item) => set({ pending: item }),
}));
