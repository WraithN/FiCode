import React from 'react';
import { useChatStore } from '../../stores/chatStore';
import { useUIStore } from '../../stores/uiStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useCompressionStore } from '../../stores/compressionStore';

export const StatusBar: React.FC = () => {
  const { currentAgent, setAgent, isGenerating } = useChatStore();
  const { currentModel } = useUIStore();
  const { connectionStatus } = useConnectionStore();
  const { isCompressing, progress, contextRatio } = useCompressionStore();

  const ratioColor = contextRatio > 85 ? 'text-error' : contextRatio > 60 ? 'text-warning' : 'text-success';
  const filled = Math.ceil((contextRatio / 100) * 10);
  const ctxBar = '█'.repeat(filled) + '░'.repeat(10 - filled);

  return (
    <div className="h-8 flex items-center px-4 bg-bg-secondary border-t border-border text-xs select-none">
      <span className="font-bold text-brand">fi-code</span>
      <span className="mx-2 text-border">│</span>

      <button
        onClick={() => setAgent(currentAgent === 'build' ? 'plan' : 'build')}
        className="flex items-center gap-1 hover:text-brand transition-colors"
        title="Click to switch agent"
      >
        <span>AGT: {currentAgent === 'build' ? 'Build' : 'Plan'}</span>
      </button>

      <span className="mx-2 text-border">│</span>
      <span className={`${ratioColor} font-mono`}>
        CTX: [{ctxBar}] {contextRatio}%
      </span>

      {isCompressing && (
        <>
          <span className="mx-2 text-border">│</span>
          <span className="text-brand animate-pulse">🗜️ Compressing {progress}%...</span>
        </>
      )}

      <span className="mx-2 text-border">│</span>
      <span className="text-text-secondary">{currentModel}</span>

      <span className="mx-2 text-border">│</span>
      <span className={`${connectionStatus === 'connected' ? 'text-success' : 'text-error'}`}>
        {connectionStatus}
      </span>

      {isGenerating && (
        <>
          <span className="mx-2 text-border">│</span>
          <span className="text-brand animate-pulse">generating...</span>
        </>
      )}
    </div>
  );
};
