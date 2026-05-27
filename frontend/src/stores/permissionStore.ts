import { create } from 'zustand';

export interface PermissionAskItem {
  toolCallId: string;
  toolName: string;
  risk: string;
  reason: string;
}

export interface QuestionAskItem {
  toolCallId: string;
  question: string;
  options: { id: string; label: string; description?: string }[];
  recommended?: string;
  allowCustom: boolean;
}

interface PermissionState {
  pendingPermission: PermissionAskItem | null;
  setPendingPermission: (item: PermissionAskItem | null) => void;
  pendingQuestion: QuestionAskItem | null;
  setPendingQuestion: (item: QuestionAskItem | null) => void;
}

export const usePermissionStore = create<PermissionState>((set) => ({
  pendingPermission: null,
  setPendingPermission: (item) => set({ pendingPermission: item }),
  pendingQuestion: null,
  setPendingQuestion: (item) => set({ pendingQuestion: item }),
}));
