import { create } from 'zustand';
import { Turn } from '../types/turn';
import { Part } from '../types/part';
import { AgentType } from '../types/agent';

interface ChatState {
  turns: Turn[];
  isGenerating: boolean;
  currentAgent: AgentType;
  startTurn: (userMessage: string) => string;
  appendPart: (turnId: string, part: Part) => void;
  completeTurn: (turnId: string) => void;
  setAgent: (agent: AgentType) => void;
  setIsGenerating: (generating: boolean) => void;
  clearTurns: () => void;
  getCurrentTurnId: () => string | null;
}

export const useChatStore = create<ChatState>((set, get) => ({
  turns: [],
  isGenerating: false,
  currentAgent: 'build',

  startTurn: (userMessage: string) => {
    const turn: Turn = {
      id: `turn-${Date.now()}`,
      userMessage,
      parts: [],
      isComplete: false,
      timestamp: Date.now(),
    };
    set((state) => ({ turns: [...state.turns, turn], isGenerating: true }));
    return turn.id;
  },

  appendPart: (turnId: string, part: Part) => {
    console.log(`[TTFT-DIAG] appendPart | type=${part.type} | turnId=${turnId}`);
    set((state) => ({
      turns: state.turns.map((turn) => {
        if (turn.id !== turnId) return turn;
        const lastPart = turn.parts[turn.parts.length - 1];
        // 合并相邻的 text part，避免流式输出时每个 chunk 都变成独立块
        if (lastPart && lastPart.type === 'text' && part.type === 'text') {
          const merged: Part = { type: 'text', text: lastPart.text + part.text };
          return { ...turn, parts: [...turn.parts.slice(0, -1), merged] };
        }
        return { ...turn, parts: [...turn.parts, part] };
      }),
    }));
  },

  completeTurn: (turnId: string) => {
    set((state) => ({
      turns: state.turns.map((turn) =>
        turn.id === turnId ? { ...turn, isComplete: true } : turn
      ),
      isGenerating: false,
    }));
  },

  setAgent: (agent) => set({ currentAgent: agent }),
  setIsGenerating: (generating) => set({ isGenerating: generating }),
  clearTurns: () => set({ turns: [], isGenerating: false }),

  getCurrentTurnId: () => {
    const { turns } = get();
    const last = turns[turns.length - 1];
    return last && !last.isComplete ? last.id : null;
  },
}));
