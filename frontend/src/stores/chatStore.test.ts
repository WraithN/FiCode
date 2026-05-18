import { describe, it, expect } from 'vitest';
import { useChatStore } from './chatStore';

describe('chatStore', () => {
  it('should start a new turn', () => {
    const store = useChatStore.getState();
    store.clearTurns();
    const turnId = store.startTurn('hello');
    expect(turnId).toBeDefined();
    expect(store.turns).toHaveLength(1);
    expect(store.turns[0].userMessage).toBe('hello');
    expect(store.turns[0].isComplete).toBe(false);
    expect(store.isGenerating).toBe(true);
  });

  it('should append part to current turn', () => {
    const store = useChatStore.getState();
    store.clearTurns();
    const turnId = store.startTurn('hello');
    store.appendPart(turnId, { type: 'text', text: 'world' });
    expect(store.turns[0].parts).toHaveLength(1);
    expect(store.turns[0].parts[0]).toEqual({ type: 'text', text: 'world' });
  });

  it('should complete turn', () => {
    const store = useChatStore.getState();
    store.clearTurns();
    const turnId = store.startTurn('hello');
    store.completeTurn(turnId);
    expect(store.turns[0].isComplete).toBe(true);
    expect(store.isGenerating).toBe(false);
  });

  it('should switch agent', () => {
    const store = useChatStore.getState();
    store.setAgent('plan');
    expect(store.currentAgent).toBe('plan');
  });
});
