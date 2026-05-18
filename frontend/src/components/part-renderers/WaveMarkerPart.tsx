import React from 'react';
import { Part } from '../../types/part';

export const WaveMarkerPart: React.FC<{ part: Extract<Part, { type: 'wave_marker' }> }> = ({ part }) => (
  <div className="text-xs text-text-muted opacity-50" data-wave-id={part.wave_id} data-turn={part.turn} />
);
