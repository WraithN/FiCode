import React from 'react';
import { Part } from '../../types/part';

export const TextPart: React.FC<{ part: Extract<Part, { type: 'text' }> }> = ({ part }) => (
  <div className="text-sm text-gray-200 whitespace-pre-wrap break-words leading-relaxed">
    {part.text}
  </div>
);
