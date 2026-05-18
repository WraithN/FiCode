import React from 'react';
import { useUIStore } from '../../stores/uiStore';

export const LogPanel: React.FC = () => {
  const { logOpen, toggleLog } = useUIStore();

  if (!logOpen) return null;

  return (
    <div className="absolute bottom-8 right-4 w-96 h-64 bg-bg-secondary border border-border rounded shadow-lg flex flex-col z-50">
      <div className="h-8 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">Logs</span>
        <button onClick={toggleLog} className="text-text-muted hover:text-text-primary">✕</button>
      </div>
      <div className="flex-1 p-2 overflow-y-auto text-xs font-mono text-text-secondary">
        <p>Log output placeholder...</p>
      </div>
    </div>
  );
};
