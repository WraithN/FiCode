import { create } from 'zustand';
import { SessionInfo } from '../types/api';

interface SessionState {
  currentSessionId: string | null;
  sessions: SessionInfo[];
  setCurrentSessionId: (id: string | null) => void;
  setSessions: (sessions: SessionInfo[]) => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  currentSessionId: null,
  sessions: [],
  setCurrentSessionId: (id) => set({ currentSessionId: id }),
  setSessions: (sessions) => set({ sessions }),
}));
