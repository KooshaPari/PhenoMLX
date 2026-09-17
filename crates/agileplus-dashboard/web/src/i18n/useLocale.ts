import { useContext } from 'react';
import { LocaleContext } from './context';
import type { LocaleContextValue } from './context';

/**
 * Access the current locale, translation function, and locale switcher.
 *
 * Must be called within a <LocaleProvider>.
 *
 * @example
 *   const { t, locale, setLocale } = useLocale();
 *   return <h1>{t("overview.title")}</h1>;
 */
export function useLocale(): LocaleContextValue {
  const ctx = useContext(LocaleContext);
  if (!ctx) {
    throw new Error('useLocale must be used within a <LocaleProvider>');
  }
  return ctx;
}

export default useLocale;
