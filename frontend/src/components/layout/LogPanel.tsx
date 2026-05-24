import React, { useEffect, useRef, useState } from 'react';
import { useUIStore } from '../../stores/uiStore';
import { apiClient } from '../../services/apiClient';
import { LogEntry } from '../../types/api';

const LEVEL_COLORS: Record<string, string> = {
  INFO: 'text-green-400',
  DEBUG: 'text-gray-500',
  ERROR: 'text-red-400',
  WARN: 'text-yellow-400',
};

export const LogPanel: React.FC = () => {
  const { logOpen, toggleLog } = useUIStore();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  // 初始加载历史日志
  useEffect(() => {
    if (!logOpen) return;
    setError(null);
    apiClient
      .getLogs(100)
      .then((entries) => setLogs(entries))
      .catch((err) => setError(err.message));
  }, [logOpen]);

  // SSE 实时订阅日志
  useEffect(() => {
    if (!logOpen) return;

    let cancelled = false;
    const controller = new AbortController();
    abortRef.current = controller;

    async function streamLogs() {
      try {
        for await (const entry of apiClient.subscribeLogs()) {
          if (cancelled) break;
          setLogs((prev) => [...prev.slice(-199), entry]);
        }
      } catch (err) {
        if (!cancelled) {
          console.warn('[LogPanel] SSE error:', err);
        }
      }
    }

    streamLogs();
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [logOpen]);

  // 自动滚动到底部
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  if (!logOpen) return null;

  return (
    <div className="absolute bottom-24 right-6 w-[450px] h-72 glass border border-tauri-border rounded-2xl shadow-2xl flex flex-col z-50">
      <div className="h-12 flex items-center justify-between px-5 border-b border-tauri-border bg-tauri-card/30">
        <div className="flex items-center gap-2">
          <svg className="w-4 h-4 text-tauri-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
          </svg>
          <span className="text-sm font-semibold gradient-text">Logs</span>
        </div>
        <button 
          onClick={toggleLog} 
          className="p-1.5 hover:bg-tauri-card rounded-lg transition-colors text-gray-400 hover:text-white"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12"/>
          </svg>
        </button>
      </div>
      <div ref={scrollRef} className="flex-1 p-4 overflow-y-auto text-xs font-mono space-y-1.5 scrollbar-tauri">
        {error ? (
          <div className="p-3 bg-red-900/20 border border-red-800/30 rounded-lg text-red-400">
            {error}
          </div>
        ) : logs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-600">
            No logs yet...
          </div>
        ) : (
          logs.map((log, idx) => (
            <div key={idx} className="flex gap-3 items-start p-2 rounded-lg hover:bg-tauri-card/20 transition-colors">
              <span className="text-gray-600 shrink-0 select-none">
                {log.timestamp.split('T')[1]?.replace('Z', '') || log.timestamp}
              </span>
              <span className={`shrink-0 font-bold ${LEVEL_COLORS[log.level] || 'text-gray-400'}`}>
                {log.level}
              </span>
              <span className="text-gray-600 shrink-0">[{log.module}]</span>
              <span className="text-gray-300 break-all">{log.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
