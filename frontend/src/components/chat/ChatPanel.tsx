import React, { useRef, useEffect } from 'react';
import { useChatStore } from '../../stores/chatStore';
import { TurnGroup } from './TurnGroup';

export const ChatPanel: React.FC = () => {
  const turns = useChatStore((s) => s.turns);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [turns]);

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-6 scrollbar-tauri">
        {turns.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center max-w-lg">
              <div className="w-20 h-20 mx-auto mb-6 rounded-2xl gradient-bg flex items-center justify-center">
                <svg className="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
                </svg>
              </div>
              <h2 className="text-2xl font-bold gradient-text mb-3">Welcome to fi-code</h2>
              <p className="text-gray-400 mb-2">Your AI-powered coding assistant</p>
              <p className="text-sm text-gray-500">Start a conversation or use /commands</p>
            </div>
          </div>
        ) : (
          <div className="space-y-6">
            {turns.map((turn) => <TurnGroup key={turn.id} turn={turn} />)}
          </div>
        )}
      </div>

      {isGenerating && (
        <div className="h-1 w-full bg-tauri-border overflow-hidden relative">
          <div className="absolute h-full w-1/4 gradient-bg animate-progress-slide rounded-full" />
        </div>
      )}
    </div>
  );
};
