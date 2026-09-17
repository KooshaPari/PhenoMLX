import React, { useState, useEffect, useCallback } from 'react';
import { Card } from '../layout/Card';
import { Toggle } from '../foundation/Toggle';
import { Select } from '../foundation/Select';
import { Button } from '../foundation/Button';
import { useTheme } from '../../theme';
import { useLocale } from '../../i18n';
import { cn } from '../../lib/utils';
import type { SelectOption } from '../../types';

// ============================================================================
// Constants
// ============================================================================

const THEME_OPTIONS: SelectOption[] = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'System' },
];

const LOCALE_OPTIONS: SelectOption[] = [
  { value: 'en', label: 'English' },
  { value: 'de', label: 'Deutsch' },
  { value: 'fr', label: 'Français', disabled: true },
  { value: 'es', label: 'Español', disabled: true },
  { value: 'ja', label: '日本語', disabled: true },
];

const APP_VERSION = '0.1.0';

const NOTIF_STORAGE_KEY = 'agileplus_notification_prefs';

// ============================================================================
// Helpers
// ============================================================================

interface NotificationPrefs {
  desktopEnabled: boolean;
  emailDigest: boolean;
}

function loadNotifPrefs(): NotificationPrefs {
  try {
    const stored = localStorage.getItem(NOTIF_STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored) as NotificationPrefs;
      return {
        desktopEnabled:
          typeof parsed.desktopEnabled === 'boolean' ? parsed.desktopEnabled : false,
        emailDigest: typeof parsed.emailDigest === 'boolean' ? parsed.emailDigest : false,
      };
    }
  } catch {
    // localStorage unavailable or corrupt data
  }
  return { desktopEnabled: false, emailDigest: false };
}

function persistNotifPrefs(prefs: NotificationPrefs): void {
  try {
    localStorage.setItem(NOTIF_STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // localStorage unavailable (private browsing, quota)
  }
}

function persistLocalePref(next: string): void {
  try {
    localStorage.setItem('agileplus_locale', next);
  } catch {
    // silently ignore
  }
}

// ============================================================================
// Section header sub-component
// ============================================================================

function SectionHeading({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="text-base font-bold text-gray-900 dark:text-gray-100 mb-4 pb-2 border-b border-gray-200 dark:border-gray-700">
      {children}
    </h3>
  );
}

// ============================================================================
// Settings View
// ============================================================================

export function SettingsView() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const { t, locale, setLocale } = useLocale();

  const [notifPrefs, setNotifPrefs] = useState<NotificationPrefs>(loadNotifPrefs);
  const [, setSavedToast] = useState(false);

  // Persist notification preferences whenever they change
  useEffect(() => {
    persistNotifPrefs(notifPrefs);
  }, [notifPrefs]);

  const handleSetLocale = useCallback(
    (value: string | number) => {
      const next = String(value);
      setLocale(next);
      persistLocalePref(next);
    },
    [setLocale],
  );

  const handleSetTheme = useCallback(
    (value: string | number) => {
      setTheme(value as 'light' | 'dark' | 'system');
    },
    [setTheme],
  );

  // ── Live preview colors ───────────────────────────────────────────────

  const previewBg = resolvedTheme === 'dark' ? 'bg-gray-800' : 'bg-white';
  const previewFg = resolvedTheme === 'dark' ? 'text-gray-100' : 'text-gray-900';
  const previewAccent = 'bg-cyan-500';
  const previewMuted = resolvedTheme === 'dark' ? 'bg-gray-700' : 'bg-gray-200';

  return (
    <div className="max-w-2xl mx-auto space-y-6">
      {/* ── Header ──────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold text-gray-900 dark:text-gray-100">
          {t('settings.title')}
        </h2>
      </div>

      {/* ── Appearance ──────────────────────────────────────────────── */}
      <Card variant="elevated">
        <SectionHeading>{t('settings.appearance') ?? 'Appearance'}</SectionHeading>

        <div className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <Select
              label={t('settings.theme')}
              options={THEME_OPTIONS}
              value={theme}
              onChange={handleSetTheme}
              ariaLabel="Select theme"
            />

            <div className="flex flex-col gap-1">
              <span className="block text-xs font-semibold text-gray-700 dark:text-gray-300 mb-1">
                Preview
              </span>
              <div
                className={cn(
                  'rounded-lg border border-gray-200 dark:border-gray-600 p-3 transition-colors',
                  previewBg,
                )}
              >
                <div className="flex items-center gap-2 mb-2">
                  <div className={cn('w-3 h-3 rounded-full', previewAccent)} />
                  <div className={cn('h-2 w-20 rounded', previewMuted)} />
                  <div className={cn('h-2 w-12 rounded ml-auto', previewMuted)} />
                </div>
                <div className={cn('h-2 w-full rounded mb-1.5', previewMuted)} />
                <div className={cn('h-2 w-3/4 rounded', previewMuted)} />
              </div>
            </div>
          </div>

          <p className="text-xs text-gray-500 dark:text-gray-400">
            {resolvedTheme === 'dark'
              ? 'Dark mode is active'
              : resolvedTheme === 'light'
                ? 'Light mode is active'
                : `System preference (currently ${resolvedTheme})`}
          </p>
        </div>
      </Card>

      {/* ── Language ────────────────────────────────────────────────── */}
      <Card variant="elevated">
        <SectionHeading>{t('settings.language')}</SectionHeading>

        <div className="max-w-xs">
          <Select
            label={t('settings.language')}
            options={LOCALE_OPTIONS}
            value={locale}
            onChange={handleSetLocale}
            ariaLabel="Select language"
          />
        </div>

        <p className="mt-3 text-xs text-gray-500 dark:text-gray-400">
          {locale === 'en'
            ? 'English selected'
            : locale === 'de'
              ? 'Deutsch ausgewählt'
              : `${locale} selected`}
        </p>
      </Card>

      {/* ── Notifications ────────────────────────────────────────────── */}
      <Card variant="elevated">
        <SectionHeading>{t('settings.notifications')}</SectionHeading>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                {t('settings.desktopNotifications') ?? 'Desktop notifications'}
              </span>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                Receive browser notifications for task updates
              </p>
            </div>
            <Toggle
              checked={notifPrefs.desktopEnabled}
              onChange={(checked) =>
                setNotifPrefs((prev) => ({ ...prev, desktopEnabled: checked }))
              }
              ariaLabel="Toggle desktop notifications"
            />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                {t('settings.emailDigest') ?? 'Email digest'}
              </span>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                Receive a daily summary of project activity
              </p>
            </div>
            <Toggle
              checked={notifPrefs.emailDigest}
              onChange={(checked) =>
                setNotifPrefs((prev) => ({ ...prev, emailDigest: checked }))
              }
              ariaLabel="Toggle email digest"
            />
          </div>
        </div>
      </Card>

      {/* ── About ───────────────────────────────────────────────────── */}
      <Card variant="elevated">
        <SectionHeading>{t('settings.about') ?? 'About'}</SectionHeading>

        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <span className="text-sm text-gray-500 dark:text-gray-400 min-w-[5rem]">
              {t('settings.version') ?? 'Version'}
            </span>
            <span className="text-sm font-mono font-medium text-gray-900 dark:text-gray-100">
              {APP_VERSION}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-sm text-gray-500 dark:text-gray-400 min-w-[5rem]">
              Framework
            </span>
            <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
              React 19 + Rust (Axum)
            </span>
          </div>

          <div className="pt-2 flex flex-wrap gap-3">
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                window.open('https://docs.agileplus.dev', '_blank', 'noopener')
              }
            >
              <svg
                className="w-4 h-4 mr-1"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
                />
              </svg>
              {t('settings.documentation') ?? 'Documentation'}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                window.open(
                  'https://github.com/KooshaPari/AgilePlus',
                  '_blank',
                  'noopener',
                )
              }
            >
              <svg className="w-4 h-4 mr-1" fill="currentColor" viewBox="0 0 24 24">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
              </svg>
              {t('settings.repository') ?? 'Repository'}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}

export default SettingsView;
