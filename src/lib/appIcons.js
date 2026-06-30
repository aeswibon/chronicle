import { invoke } from '@tauri-apps/api/core';
import { eventIconKey } from '$lib/format.js';

/** @type {Record<string, string>} */
const CATEGORY_ICONS = {
  git: iconSvg(
    '<path fill="currentColor" d="M6 2a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6H6zm7 1.5L18.5 9H13V3.5zM8 12.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm8-1a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM9.2 14.8l5.6-3.2"/>',
    '#f97316'
  ),
  shell: iconSvg(
    '<path fill="currentColor" d="M4 5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5zm4.7 4.3-2.1 2.1a.75.75 0 1 0 1.06 1.06l.97-.97V15a.75.75 0 0 0 1.5 0v-3.61l.97.97a.75.75 0 1 0 1.06-1.06l-2.1-2.1a.75.75 0 0 0-1.06 0z"/>',
    '#14b8a6'
  ),
  filesystem: iconSvg(
    '<path fill="currentColor" d="M6 2a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6H6zm7 1.5L18.5 9H13V3.5z"/>',
    '#6366f1'
  ),
};

/** @type {Map<string, string | null>} */
const cache = new Map();

/** @param {string} paths @param {string} color */
function iconSvg(paths, color) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" color="${color}">${paths}</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

/** @param {ReturnType<typeof eventIconKey>} key */
function cacheKey(key) {
  if (!key) return '';
  if (key.type === 'category') return `category:${key.category}`;
  return `app:${key.bundleId ?? ''}:${key.appName ?? ''}`;
}

/**
 * @param {ReturnType<typeof eventIconKey>} key
 * @returns {Promise<string | null>}
 */
export async function getAppIconDataUrl(key) {
  if (!key) return null;

  const id = cacheKey(key);
  if (cache.has(id)) return cache.get(id) ?? null;

  if (key.type === 'category') {
    const url = CATEGORY_ICONS[key.category] ?? null;
    cache.set(id, url);
    return url;
  }

  try {
    const url = await invoke('resolve_app_icon', {
      bundleId: key.bundleId,
      appName: key.appName,
    });
    cache.set(id, url ?? null);
    return url ?? null;
  } catch {
    cache.set(id, null);
    return null;
  }
}

/** @param {string} path @returns {Promise<string | null>} */
export async function getPathIconDataUrl(path) {
  if (!path) return null;

  const id = `path:${path}`;
  if (cache.has(id)) return cache.get(id) ?? null;

  try {
    const map = await invoke('resolve_path_icons', { paths: [path] });
    const url = map[path] ?? null;
    cache.set(id, url);
    return url;
  } catch {
    cache.set(id, null);
    return null;
  }
}

/**
 * @param {string[]} paths
 * @returns {Promise<Record<string, string>>}
 */
export async function preloadPathIcons(paths) {
  const unique = [...new Set(paths.filter(Boolean))];
  const missing = unique.filter((p) => !cache.has(`path:${p}`));
  if (missing.length === 0) {
    return Object.fromEntries(
      unique.map((p) => [p, cache.get(`path:${p}`)]).filter(([, u]) => u)
    );
  }

  const merged = /** @type {Record<string, string>} */ ({});
  const chunkSize = 24;
  for (let i = 0; i < missing.length; i += chunkSize) {
    const chunk = missing.slice(i, i + chunkSize);
    try {
      const map = await invoke('resolve_path_icons', { paths: chunk });
      for (const p of chunk) {
        cache.set(`path:${p}`, map[p] ?? null);
        if (map[p]) merged[p] = map[p];
      }
    } catch {
      for (const p of chunk) {
        cache.set(`path:${p}`, null);
      }
    }
  }
  return merged;
}

/** @param {Record<string, unknown>} event */
export function getEventIconDataUrl(event) {
  return getAppIconDataUrl(eventIconKey(event));
}
