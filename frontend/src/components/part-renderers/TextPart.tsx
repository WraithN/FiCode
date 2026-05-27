import React, { useRef, useEffect } from 'react';
import { Part } from '../../types/part';

export const TextPart: React.FC<{ part: Extract<Part, { type: 'text' }> }> = ({ part }) => {
  const hasLogged = useRef(false);
  useEffect(() => {
    if (!hasLogged.current && part.text.length > 0) {
      hasLogged.current = true;
      console.log(`[TTFT-DIAG] TextPart first render | text_len=${part.text.length} | preview=${part.text.slice(0, 30)}`);
    }
  }, [part.text]);
  return (
    <div className="text-sm text-gray-200 whitespace-pre-wrap break-words leading-relaxed">
      {part.text}
    </div>
  );
};
