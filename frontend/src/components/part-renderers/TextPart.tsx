import React from 'react';
import { Part } from '../../types/part';

export const TextPart: React.FC<{ part: Extract<Part, { type: 'text' }> }> = ({ part }) => (
  <div className="text-sm text-text-primary whitespace-pre-wrap break-words">{part.text}</div>
);
