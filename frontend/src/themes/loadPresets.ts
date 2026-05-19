import presetJson from '../../../crates/shared/src/preset_themes.json';
import { ThemePreset } from '../types/theme';

function u32ToHex(u32: number | undefined, fallback: number = 0): string {
  const val = u32 ?? fallback;
  return `#${val.toString(16).padStart(6, '0')}`;
}

export const themePresets: ThemePreset[] = (presetJson as any[]).map(p => ({
  name: p.name,
  description: p.description,
  colors: {
    bg: u32ToHex(p.bg_base),
    bgSecondary: u32ToHex(p.bg_surface),
    bgOverlay: u32ToHex(p.bg_overlay),
    bgUserArea: u32ToHex(p.bg_user_area, p.bg_base),
    bgAiArea: u32ToHex(p.bg_ai_area, p.bg_surface),
    textPrimary: u32ToHex(p.text_primary),
    textSecondary: u32ToHex(p.text_secondary),
    textMuted: u32ToHex(p.text_muted),
    textPlaceholder: u32ToHex(p.text_placeholder),
    border: u32ToHex(p.border),
    brand: u32ToHex(p.brand),
    accentHover: u32ToHex(p.accent_hover),
    user: u32ToHex(p.user),
    success: u32ToHex(p.success),
    warning: u32ToHex(p.warning),
    error: u32ToHex(p.error),
    selectionBg: u32ToHex(p.selection_bg),
    selectionFg: u32ToHex(p.selection_fg),
  },
}));

export function getPresetByName(name: string): ThemePreset | undefined {
  return themePresets.find(p => p.name.toLowerCase() === name.toLowerCase());
}
