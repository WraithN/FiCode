import { useUIStore } from '../stores/uiStore';

export function useTheme() {
  const themeName = useUIStore((state) => state.themeName);
  const isLight = themeName === 'light' || themeName === 'one_light';
  
  return {
    theme: themeName,
    isLight,
    isDark: !isLight,
  };
}
