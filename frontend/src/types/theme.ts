export interface ThemeColors {
  bg: string;
  bgSecondary: string;
  bgOverlay: string;
  bgUserArea: string;
  bgAiArea: string;
  textPrimary: string;
  textSecondary: string;
  textMuted: string;
  textPlaceholder: string;
  border: string;
  brand: string;
  accentHover: string;
  user: string;
  success: string;
  warning: string;
  error: string;
  selectionBg: string;
  selectionFg: string;
}

export interface ThemePreset {
  name: string;
  description: string;
  colors: ThemeColors;
}
