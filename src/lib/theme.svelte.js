import { browser } from '$app/environment';

const STORAGE_KEY = 'chronicle-theme';

/** @typedef {'light' | 'dark' | 'system'} ThemePreference */

/** @type {{ preference: ThemePreference, isDark: boolean }} */
export const theme = $state({
  preference: 'system',
  isDark: false,
});

function resolveDark(pref) {
  if (!browser) return false;
  if (pref === 'dark') return true;
  if (pref === 'light') return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function applyTheme(dark) {
  if (!browser) return;
  document.documentElement.classList.toggle('dark', dark);
  theme.isDark = dark;
}

export function initTheme() {
  if (!browser) return;
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark' || stored === 'system') {
    theme.preference = stored;
  }
  applyTheme(resolveDark(theme.preference));

  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (theme.preference === 'system') applyTheme(resolveDark('system'));
  });
}

/** @param {ThemePreference} next */
export function setTheme(next) {
  theme.preference = next;
  if (browser) localStorage.setItem(STORAGE_KEY, next);
  applyTheme(resolveDark(next));
}

export function toggleTheme() {
  setTheme(theme.isDark ? 'light' : 'dark');
}
