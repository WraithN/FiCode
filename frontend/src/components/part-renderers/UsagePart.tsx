import React from 'react';
import { Part } from '../../types/part';

export const UsagePart: React.FC<{ part: Extract<Part, { type: 'usage' }> }> = ({ part }) => (
  <div className="text-xs text-text-muted mt-2">
    Tokens: {part.prompt_tokens} prompt + {part.completion_tokens} completion
  </div>
);
