import React from 'react';
import { Part } from '../../types/part';

export const CodeBlockPart: React.FC<{ part: Extract<Part, { type: 'code_block' }> }> = ({ part }) => {
  const lines = part.code.split('\n');

  const renderLine = (line: string, index: number) => {
    let className = 'block';

    if (line.startsWith('+')) {
      className += ' bg-green-900/30 text-green-400';
    } else if (line.startsWith('-')) {
      className += ' bg-red-900/30 text-red-400';
    }

    return (
      <span key={index} className={className}>
        {line}
        {'\n'}
      </span>
    );
  };

  return (
    <div className="my-2 rounded overflow-hidden border border-border">
      <div className="text-xs text-text-muted bg-bg-secondary px-3 py-1 border-b border-border flex justify-between items-center">
        <span>{part.language || 'code'}</span>
      </div>
      <pre 
        className="text-sm text-text-primary bg-bg p-3 overflow-x-auto"
        style={{ tabSize: 4, whiteSpace: 'pre' }}
      >
        <code>{lines.map((line, idx) => renderLine(line, idx))}</code>
      </pre>
    </div>
  );
};
