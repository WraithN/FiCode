import React, { useEffect, useState } from 'react';
import { useUIStore } from '../../stores/uiStore';
import { apiClient } from '../../services/apiClient';
import { FileEntry } from '../../types/api';

interface FileTreeNodeProps {
  entry: FileEntry;
  depth: number;
}

const FileTreeNode: React.FC<FileTreeNodeProps> = ({ entry, depth }) => {
  const [expanded, setExpanded] = useState(false);
  const indent = depth * 16;

  if (!entry.is_dir) {
    return (
      <div
        className="flex items-center py-2 px-3 rounded-lg hover:bg-tauri-card/50 cursor-pointer text-gray-300 text-sm transition-colors"
        style={{ paddingLeft: `${indent + 12}px` }}
      >
        <svg className="w-4 h-4 mr-2 text-gray-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
        </svg>
        <span className="truncate">{entry.name}</span>
      </div>
    );
  }

  return (
    <div>
      <div
        className="flex items-center py-2 px-3 rounded-lg hover:bg-tauri-card/50 cursor-pointer text-gray-100 text-sm font-medium transition-colors"
        style={{ paddingLeft: `${indent + 12}px` }}
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? (
          <svg className="w-4 h-4 mr-2 text-gray-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7"/>
          </svg>
        ) : (
          <svg className="w-4 h-4 mr-2 text-gray-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 5l7 7-7 7"/>
          </svg>
        )}
        <svg className="w-4 h-4 mr-2 text-tauri-primary flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
        </svg>
        <span className="truncate">{entry.name}</span>
      </div>
      {expanded && entry.children && (
        <div>
          {entry.children.map((child, idx) => (
            <FileTreeNode key={`${child.path}-${idx}`} entry={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
};

export const LeftDrawer: React.FC = () => {
  const { leftDrawerOpen, toggleLeftDrawer } = useUIStore();
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!leftDrawerOpen) return;
    apiClient
      .getFileTree()
      .then((res) => setEntries(res.entries))
      .catch((err) => setError(err.message));
  }, [leftDrawerOpen]);

  if (!leftDrawerOpen) {
    return (
      <button
        onClick={toggleLeftDrawer}
        className="w-10 h-full glass border-r border-tauri-border flex items-center justify-center hover:bg-tauri-card/50 transition-colors"
      >
        <svg className="w-5 h-5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 5l7 7-7 7M5 5l7 7-7 7"/>
        </svg>
      </button>
    );
  }

  return (
    <div className="w-64 glass border-r border-tauri-border flex flex-col shrink-0">
      <div className="h-14 flex items-center justify-between px-5 border-b border-tauri-border">
        <span className="text-base font-semibold gradient-text">Files</span>
        <button onClick={toggleLeftDrawer} className="p-1 hover:bg-tauri-card rounded-lg transition-colors">
          <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7"/>
          </svg>
        </button>
      </div>
      <div className="flex-1 p-3 overflow-y-auto scrollbar-tauri">
        {error ? (
          <div className="p-4 bg-red-500/10 border border-red-500/20 rounded-lg">
            <p className="text-sm text-red-400">{error}</p>
          </div>
        ) : entries.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <div className="text-center">
              <div className="w-10 h-10 mx-auto mb-3 rounded-full bg-tauri-card flex items-center justify-center">
                <svg className="w-5 h-5 text-gray-500 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
                </svg>
              </div>
              <p className="text-sm text-gray-500">Loading...</p>
            </div>
          </div>
        ) : (
          entries.map((entry, idx) => <FileTreeNode key={`${entry.path}-${idx}`} entry={entry} depth={0} />)
        )}
      </div>
    </div>
  );
};
