import { useContext } from 'react';
import { ThemeContext, ThemeContextValue } from './context';

/**
 * Access the current theme state and setter.
 *
 * Must be called within a <ThemeProvider>.
 *
 * @returns {ThemeContextValue} { theme, setTheme, resolvedTheme }
 */
export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw new Error(
      'useTheme() must be used within a <ThemeProvider>. ' +
        'Wrap your application root with <ThemeProvider>.',
    );
  }
  return ctx;
}
