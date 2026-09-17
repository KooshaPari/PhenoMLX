import React, { createContext, useState, useCallback, useRef } from 'react';
import type { ReactNode } from 'react';

// ─── Types ────────────────────────────────────────────────────────────────────

type MessageCatalog = Record<string, string>;

export interface LocaleContextValue {
  /** Current locale code (e.g. "en", "de") */
  locale: string;
  /**
   * Look up a translated string by dotted key.
   * Supports simple {placeholder} interpolation with the second argument.
   *
   * @example
   *   t("nav.dashboard")                // → "Dashboard"
   *   t("stories.shown", { count: 12 }) // → "12 shown"
   */
  t: (key: string, params?: Record<string, string | number>) => string;
  /** Switch the active locale and reload messages */
  setLocale: (locale: string) => void;
}

// ─── Locale registry — add new locales here ───────────────────────────────────

const LOCALE_MESSAGES: Record<string, () => Promise<MessageCatalog>> = {
  en: () => import('./messages/en.json').then((m) => m.default ?? m),
  de: () => import('./messages/de.json').then((m) => m.default ?? m),
};

// ─── Context ───────────────────────────────────────────────────────────────────

export const LocaleContext = createContext<LocaleContextValue>({
  locale: 'en',
  t: (key: string) => key,
  setLocale: () => {},
});

// ─── Provider ──────────────────────────────────────────────────────────────────

interface LocaleProviderProps {
  children: ReactNode;
  /** Initial locale (defaults to "en") */
  defaultLocale?: string;
}

/**
 * Provides locale state and a `t()` translation function to the component tree.
 *
 * Messages are loaded lazily via dynamic import when the locale changes.
 * Falls back to the message key itself when a translation is missing.
 */
export function LocaleProvider({ children, defaultLocale = 'en' }: LocaleProviderProps) {
  const [locale, setLocaleState] = useState(defaultLocale);
  const [messages, setMessages] = useState<MessageCatalog>(() => {
    // Synchronously require the default locale so first render is never empty
    try {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      return require('./messages/en.json') as MessageCatalog;
    } catch {
      return {};
    }
  });
  const loadingRef = useRef(false);

  const switchLocale = useCallback(async (next: string) => {
    const loader = LOCALE_MESSAGES[next];
    if (!loader) {
      console.warn(`[i18n] Unknown locale "${next}", falling back to key passthrough`);
      setLocaleState(next);
      setMessages({});
      return;
    }

    if (loadingRef.current) return;
    loadingRef.current = true;

    try {
      const mod = await loader();
      setMessages(mod);
      setLocaleState(next);
    } catch (err) {
      console.error(`[i18n] Failed to load messages for "${next}":`, err);
    } finally {
      loadingRef.current = false;
    }
  }, []);

  const t = useCallback(
    (key: string, params?: Record<string, string | number>): string => {
      let value = messages[key];
      if (value === undefined) {
        // Fall back to the key itself so the UI doesn't break
        if (process.env.NODE_ENV === 'development') {
          console.warn(`[i18n] Missing translation for "${key}" in locale "${locale}"`);
        }
        value = key;
      }
      if (params) {
        for (const [k, v] of Object.entries(params)) {
          value = value.replace(`{${k}}`, String(v));
        }
      }
      return value;
    },
    [messages, locale],
  );

  return (
    <LocaleContext.Provider value={{ locale, t, setLocale: switchLocale }}>
      {children}
    </LocaleContext.Provider>
  );
}
