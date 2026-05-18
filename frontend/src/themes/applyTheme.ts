import { ThemePreset } from '../types/theme';

export function applyTheme(preset: ThemePreset): void {
  const root = document.documentElement;
  const c = preset.colors;
  root.style.setProperty('--color-bg', c.bg);
  root.style.setProperty('--color-bg-secondary', c.bgSecondary);
  root.style.setProperty('--color-bg-overlay', c.bgOverlay);
  root.style.setProperty('--color-bg-user-area', c.bgUserArea);
  root.style.setProperty('--color-bg-ai-area', c.bgAiArea);
  root.style.setProperty('--color-text-primary', c.textPrimary);
  root.style.setProperty('--color-text-secondary', c.textSecondary);
  root.style.setProperty('--color-text-muted', c.textMuted);
  root.style.setProperty('--color-text-placeholder', c.textPlaceholder);
  root.style.setProperty('--color-border', c.border);
  root.style.setProperty('--color-brand', c.brand);
  root.style.setProperty('--color-accent-hover', c.accentHover);
  root.style.setProperty('--color-user', c.user);
  root.style.setProperty('--color-success', c.success);
  root.style.setProperty('--color-error', c.error);
  root.style.setProperty('--color-warning', c.warning);
  root.style.setProperty('--color-selection-bg', c.selectionBg);
  root.style.setProperty('--color-selection-fg', c.selectionFg);
}
