import React from 'react';
import { Part } from '../../types/part';

interface SystemNoticePartProps {
  part: Extract<Part, { type: 'system_notice' }>;
}

export const SystemNoticePart: React.FC<SystemNoticePartProps> = ({ part }) => {
  return (
    <div className="my-2 px-3 py-2 bg-bg-tertiary/50 border-l-2 border-brand rounded text-xs text-text-secondary">
      {part.content}
    </div>
  );
};
