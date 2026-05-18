import React from 'react';
import { useUIStore } from '../../stores/uiStore';
import { useSessionStore } from '../../stores/sessionStore';

export const RightDrawer: React.FC = () => {
  const { rightDrawerOpen, toggleRightDrawer } = useUIStore();
  const { sessions, currentSessionId, setCurrentSessionId } = useSessionStore();

  if (!rightDrawerOpen) return null;

  return (
    <div className="w-64 bg-bg-secondary border-l border-border flex flex-col">
      <div className="h-10 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">History</span>
        <button onClick={toggleRightDrawer} className="text-text-muted hover:text-text-primary text-xs">›</button>
      </div>
      <div className="flex-1 overflow-y-auto">
        {sessions.map((session) => (
          <button
            key={session.id}
            onClick={() => setCurrentSessionId(session.id)}
            className={`w-full text-left px-3 py-2 text-sm border-b border-border ${
              session.id === currentSessionId ? 'bg-bg-overlay text-brand' : 'text-text-secondary hover:bg-bg-overlay'
            }`}
          >
            {session.name}
          </button>
        ))}
      </div>
    </div>
  );
};
