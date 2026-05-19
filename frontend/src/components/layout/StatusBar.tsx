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
  // 使用 8 格进度条，避免全角字符溢出
  const CTX_BAR_WIDTH = 8;
  const filled = Math.min(Math.ceil((contextRatio / 100) * CTX_BAR_WIDTH), CTX_BAR_WIDTH);
  const ctxBar = '█'.repeat(filled) + '░'.repeat(CTX_BAR_WIDTH - filled);

  return (
    <div className="h-8 flex items-center px-4 bg-bg-secondary border-t border-border text-xs select-none overflow-hidden whitespace-nowrap">
      <span className="font-bold text-brand flex-shrink-0">fi-code</span>
      <span className="mx-2 text-border flex-shrink-0">│</span>

      <button
        onClick={() => setAgent(currentAgent === 'build' ? 'plan' : 'build')}
        className="flex items-center gap-1 hover:text-brand transition-colors flex-shrink-0"
        title="Click to switch agent"
      >
        <span>AGT: {currentAgent === 'build' ? 'Build' : 'Plan'}</span>
      </button>

      <span className="mx-2 text-border flex-shrink-0">│</span>
      <span className={`${ratioColor} font-mono flex-shrink-0`}>
        CTX: [{ctxBar}] {contextRatio}%
      </span>

      {isCompressing && (
        <>
          <span className="mx-2 text-border flex-shrink-0">│</span>
          <span className="text-brand animate-pulse flex-shrink-0">🗜️ {progress}%</span>
        </>
      )}

      <span className="mx-2 text-border flex-shrink-0">│</span>
      <span className="text-text-secondary truncate flex-shrink min-w-0">{currentModel}</span>

      <span className="mx-2 text-border flex-shrink-0">│</span>
      <span className={`${connectionStatus === 'connected' ? 'text-success' : 'text-error'} flex-shrink-0`}>
        {connectionStatus}
      </span>

      {isGenerating && (
        <>
          <span className="mx-2 text-border flex-shrink-0">│</span>
          <span className="text-brand animate-pulse flex-shrink-0">generating...</span>
        </>
      )}
    </div>
  );
};
