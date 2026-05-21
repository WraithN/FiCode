import React from 'react';
import { Part } from '../../types/part';

export const ToolResultPart: React.FC<{ part: Extract<Part, { type: 'tool_result' }> }> = ({ part }) => {
  const lines = part.content.split('\n');
  const firstLine = lines[0];
  const hasMore = lines.length > 1 && lines[1].trim().length > 0;

  return (
    <div className="my-1 rounded bg-bg-secondary/50 border-l-2 border-success overflow-hidden">
      <div className="px-2 py-1">
        <span className="text-xs text-success font-mono">
          ✓ Result ({part.duration_ms}ms)
        </span>
      </div>
      {hasMore && (
        <div className="px-2 py-1 border-t border-border/30">
          <div className="text-xs text-text-muted font-mono whitespace-pre-wrap break-words">
            {firstLine}
          </div>
        </div>
      )}
    </div>
  );
};
