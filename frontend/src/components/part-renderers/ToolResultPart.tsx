import React from 'react';
import { Part } from '../../types/part';

export const ToolResultPart: React.FC<{ part: Extract<Part, { type: 'tool_result' }> }> = ({ part }) => (
  <div className="my-1 px-2 py-1 rounded bg-bg-secondary/50 border-l-2 border-success">
    <span className="text-xs text-success font-mono">
      ✓ Result ({part.duration_ms}ms)
    </span>
  </div>
);
