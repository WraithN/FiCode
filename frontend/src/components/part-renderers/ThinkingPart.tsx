import React from 'react';
import { Part } from '../../types/part';

export const ThinkingPart: React.FC<{ part: Extract<Part, { type: 'thinking' }> }> = ({ part }) => (
  <div className="text-sm text-text-muted italic border-l-2 border-brand pl-3 my-2">
    {part.content}
  </div>
);
