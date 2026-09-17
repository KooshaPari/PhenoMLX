/**
 * Lightweight i18n Module
 * Simple translation helper without external dependencies.
 * Supports locale detection from SettingsConfig or navigator.language.
 */

// ============================================================================
// Translation Definitions
// ============================================================================

const translations: Record<string, Record<string, string>> = {
  en: {
    // Navigation
    'nav.dashboard': 'Dashboard',
    'nav.epics': 'Epics',
    'nav.stories': 'Stories',
    'nav.evidence': 'Evidence',

    // Dashboard
    'dashboard.overview': 'Overview',
    'dashboard.totalEpics': 'Total Epics',
    'dashboard.epicsDone': 'Epics Done',
    'dashboard.inProgress': 'In Progress',
    'dashboard.planned': 'Planned',
    'dashboard.stories': 'Stories',
    'dashboard.storiesDone': 'Stories Done',

    // Common UI
    'common.loading': 'Loading…',
    'common.noData': 'No data available',
    'common.error': 'An error occurred',
    'common.retry': 'Retry',
    'common.save': 'Save',
    'common.cancel': 'Cancel',
    'common.close': 'Close',

    // Empty states
    'empty.noEpics': 'No epics yet',
    'empty.noStories': 'No stories yet',
    'empty.noEvidence': 'No evidence items',
    'empty.createFirst': 'Create your first one',
  },
};

// ============================================================================
// Locale Detection
// ============================================================================

/**
 * Detect the user's preferred locale.
 * Checks SettingsConfig.language (from localStorage) first,
 * then falls back to navigator.language.
 */
function detectLocale(): string {
  try {
    const stored = localStorage.getItem('agileplus-settings');
    if (stored) {
      const settings = JSON.parse(stored) as { language?: string };
      if (settings.language && translations[settings.language]) {
        return settings.language;
      }
    }
  } catch {
    // localStorage unavailable or parse failed — fall through
  }

  const navLang = (navigator.language ?? 'en').split('-')[0];
  return translations[navLang] ? navLang : 'en';
}

// ============================================================================
// Translation Function
// ============================================================================

const currentLocale = detectLocale();

/**
 * Translate a key into the current locale's string.
 * Falls back to the key itself if no translation is found.
 */
function t(key: string): string {
  return translations[currentLocale]?.[key] ?? translations['en']?.[key] ?? key;
}

// ============================================================================
// React Hook
// ============================================================================

/**
 * useTranslation Hook
 * Returns a `t` function for translating keys in the current locale.
 *
 * @example
 * const { t } = useTranslation();
 * <h1>{t('dashboard.overview')}</h1>
 */
export function useTranslation() {
  return { t };
}

export default useTranslation;
