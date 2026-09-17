/**
 * i18n — Internationalisation for the AgilePlus dashboard.
 *
 * @example
 *   import { LocaleProvider, useLocale } from '@/i18n';
 *
 *   function Root() {
 *     return (
 *       <LocaleProvider defaultLocale="en">
 *         <App />
 *       </LocaleProvider>
 *     );
 *   }
 *
 *   function App() {
 *     const { t, locale, setLocale } = useLocale();
 *     return <h1>{t("overview.title")}</h1>;
 *   }
 */

export { LocaleProvider, LocaleContext } from './context';
export type { LocaleContextValue } from './context';
export { useLocale } from './useLocale';
