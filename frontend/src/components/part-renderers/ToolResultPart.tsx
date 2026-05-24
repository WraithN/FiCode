import React, { useState } from 'react';
import { Part, ToolResultMetadata } from '../../types/part';
import hljs from 'highlight.js';
import 'highlight.js/styles/atom-one-dark.css';

export const ToolResultPart: React.FC<{ part: Extract<Part, { type: 'tool_result' }> }> = ({ part }) => {
  const [isExpanded, setIsExpanded] = useState(true);
  const metadata = part.metadata as ToolResultMetadata | undefined;
  
  // 判断内容是否是代码
  const isCode = metadata?.content_type === 'code' || 
                 metadata?.tool_name === 'read' || 
                 metadata?.tool_name === 'write' ||
                 metadata?.tool_name === 'edit' ||
                 metadata?.tool_name === 'glob' ||
                 metadata?.tool_name === 'grep';
  
  const codeContent = part.content || '';
  const highlightedCode = isCode && codeContent ? hljs.highlightAuto(codeContent).value : codeContent;
  
  return (
    <div className={`my-3 rounded-xl border overflow-hidden transition-all ${
      metadata?.is_error ? 'bg-red-950/30 border-red-800/50' : 'bg-tauri-darker border-tauri-border'
    }`}>
      {/* 元数据区域 - 可折叠 */}
      <div 
        className={`px-4 py-3 border-b cursor-pointer flex items-center justify-between ${
          metadata?.is_error ? 'bg-red-900/20 border-red-800/30' : 'bg-tauri-card/50 border-tauri-border/50'
        }`}
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex flex-wrap gap-2 items-center">
          <span className={`text-sm font-semibold flex items-center gap-1.5 ${
            metadata?.is_error ? 'text-red-400' : 'text-green-400'
          }`}>
            {metadata?.is_error ? (
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"/>
              </svg>
            )}
            {metadata?.is_error ? 'Error' : 'Result'}
          </span>
          
          {part.duration_ms && (
            <span className="text-xs text-gray-500 font-mono bg-tauri-dark/50 px-2 py-0.5 rounded">
              ⏱ {part.duration_ms}ms
            </span>
          )}
          
          {metadata && (
            <>
              {metadata.tool_name && (
                <span className="text-xs text-tauri-primary font-mono bg-tauri-primary/10 px-2 py-0.5 rounded border border-tauri-primary/20">
                  🔧 {metadata.tool_name}
                </span>
              )}
              
              {metadata.tool_call_id && (
                <span className="text-xs text-gray-500 font-mono">
                  {metadata.tool_call_id.slice(0, 8)}...
                </span>
              )}
              
              {metadata.compressed && (
                <span className="text-xs text-yellow-400 font-mono bg-yellow-900/20 px-2 py-0.5 rounded">
                  📦 Compressed
                </span>
              )}
              
              {metadata.truncated && (
                <span className="text-xs text-orange-400 font-mono bg-orange-900/20 px-2 py-0.5 rounded">
                  … Truncated
                </span>
              )}
              
              {metadata.content_type && (
                <span className="text-xs text-gray-500 font-mono">
                  {metadata.content_type}
                </span>
              )}
              
              {metadata.line_count && (
                <span className="text-xs text-gray-500 font-mono">
                  #{metadata.line_count} lines
                </span>
              )}
              
              {metadata.byte_count && (
                <span className="text-xs text-gray-500 font-mono">
                  💾 {metadata.byte_count.toLocaleString()} bytes
                </span>
              )}
            </>
          )}
        </div>
        
        {/* 折叠/展开按钮 */}
        <div className="text-gray-400 transition-transform duration-200">
          {isExpanded ? (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 15l7-7 7 7"/>
            </svg>
          ) : (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7"/>
            </svg>
          )}
        </div>
      </div>
      
      {/* 内容区域 */}
      {isExpanded && part.content && part.content.trim().length > 0 && (
        <div className="p-4">
          {isCode ? (
            <div className="bg-tauri-dark rounded-lg overflow-hidden border border-tauri-border">
              <pre className="p-4 text-sm font-mono overflow-x-auto">
                <code 
                  className="hljs"
                  dangerouslySetInnerHTML={{ __html: highlightedCode }}
                />
              </pre>
            </div>
          ) : (
            <div className="text-sm text-gray-200 font-mono whitespace-pre-wrap break-words bg-tauri-dark/50 p-4 rounded-lg border border-tauri-border/50">
              {part.content}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
