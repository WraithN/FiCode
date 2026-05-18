import { create } from 'zustand';

interface ConnectionState {
  mode: 'standalone' | 'remote';
  connectionStatus: 'connecting' | 'connected' | 'error';
  serverUrl: string;
  connectionError: string | null;
  setMode: (mode: 'standalone' | 'remote') => void;
  setConnectionStatus: (status: 'connecting' | 'connected' | 'error', error?: string) => void;
  setServerUrl: (url: string) => void;
}

export const useConnectionStore = create<ConnectionState>((set) => ({
  mode: 'standalone',
  connectionStatus: 'connecting',
  serverUrl: 'http://localhost:4040',
  connectionError: null,
  setMode: (mode) => set({ mode }),
  setConnectionStatus: (status, error) => set({ connectionStatus: status, connectionError: error || null }),
  setServerUrl: (url) => set({ serverUrl: url }),
}));
