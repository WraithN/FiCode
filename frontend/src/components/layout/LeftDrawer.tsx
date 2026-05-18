import React from 'react';
import { useUIStore } from '../../stores/uiStore';

export const LeftDrawer: React.FC = () => {
  const { leftDrawerOpen, toggleLeftDrawer } = useUIStore();

  if (!leftDrawerOpen) {
    return (
      <button
        onClick={toggleLeftDrawer}
        className="w-8 h-full bg-bg-secondary border-r border-border flex items-center justify-center hover:bg-bg-overlay"
      >
        <span className="text-text-muted text-xs">›</span>
      </button>
    );
  }

  return (
    <div className="w-64 bg-bg-secondary border-r border-border flex flex-col">
      <div className="h-10 flex items-center justify-between px-3 border-b border-border">
        <span className="text-sm font-medium text-text-primary">Files</span>
        <button onClick={toggleLeftDrawer} className="text-text-muted hover:text-text-primary text-xs">‹</button>
      </div>
      <div className="flex-1 p-2 text-sm text-text-muted">
        <p>File tree placeholder</p>
      </div>
    </div>
  );
};
