export function formatTime(ts) {
  return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
}

export function formatDate(ts) {
  return new Date(ts).toLocaleDateString([], {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  });
}

export function formatDateTime(ts) {
  return new Date(ts).toLocaleString([], {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}

/** @param {number | null | undefined} ms */
export function formatDuration(ms) {
  if (!ms || ms <= 0) return '—';
  const mins = Math.floor(ms / 60000);
  if (mins < 1) return '<1m';
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  const rem = mins % 60;
  return rem > 0 ? `${hours}h ${rem}m` : `${hours}h`;
}

/** @param {number} ts */
export function dateGroupLabel(ts) {
  const date = new Date(ts);
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfDate = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const diffDays = Math.round((startOfToday.getTime() - startOfDate.getTime()) / 86400000);

  if (diffDays === 0) return 'Today';
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return formatDate(ts);
  return date.toLocaleDateString([], { month: 'long', day: 'numeric', year: 'numeric' });
}

/** @param {Array<Record<string, unknown> & { timestamp: number }>} items */
export function groupByDate(items) {
  /** @type {Record<string, typeof items>} */
  const groups = {};
  for (const item of items) {
    const key = dateGroupLabel(item.timestamp);
    if (!groups[key]) groups[key] = [];
    groups[key].push(item);
  }
  return Object.entries(groups);
}

/** @param {string | undefined} text @param {number} max */
function truncate(text, max = 72) {
  if (!text) return '';
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** Middle-ellipsis for long filesystem paths. */
export function formatPathMiddle(path, max = 52) {
  if (!path) return '';
  if (path.length <= max) return path;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${path.slice(0, head)}…${path.slice(-tail)}`;
}

/** Open span = still in progress on the daemon (no end time). */
export function isSpanActive(span) {
  return span.ended_at == null || span.ended_at === undefined;
}

/** Show only in-progress sessions (open spans, not idle). */
export function isListableSpan(span) {
  if (span.span_type === 'idle') return false;
  return isSpanActive(span);
}

/** Human-readable span type for session chips. */
/** @param {string | { span_type?: string, metadata?: Record<string, unknown> }} spanOrType */
export function spanTypeLabel(spanOrType, metadata = null) {
  const spanType =
    typeof spanOrType === 'object' && spanOrType !== null
      ? spanOrType.span_type
      : spanOrType;
  const meta =
    typeof spanOrType === 'object' && spanOrType !== null
      ? spanOrType.metadata ?? null
      : metadata;
  const type = String(spanType ?? '').toLowerCase();
  if (type === 'documentation' && meta && typeof meta === 'object') {
    const activityLabels = Array.isArray(meta.activity_labels)
      ? meta.activity_labels.map((label) => String(label).toLowerCase())
      : [];
    const app = String(meta.app_name ?? meta.bundle_id ?? '').toLowerCase();
    if (app.includes('finder')) return 'Files';
    if (
      ['safari', 'chrome', 'firefox', 'arc', 'brave', 'edge', 'opera', 'vivaldi'].some((b) =>
        app.includes(b),
      )
    ) {
      return 'Browser';
    }
    if (activityLabels.includes('files')) return 'Files';
    if (activityLabels.includes('browsing')) return 'Browser';
    if (activityLabels.includes('coding')) return 'IDE';
    if (activityLabels.includes('terminal')) return 'Terminal';
    if (activityLabels.includes('agent session')) return 'Agent';
  }
  return String(spanType ?? '')
    .replaceAll('_', ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Detect summaries that leaked model chain-of-thought. */
export function summaryLooksLikeReasoning(text) {
  if (!text?.trim()) return false;
  const lower = text.toLowerCase();
  const markers = [
    'let me think',
    'to answer this',
    'let me check',
    "here's my attempt",
    'however, i need to rephrase',
    'carefully review',
    'extract the essential',
    'provided developer summary',
  ];
  if (markers.some((m) => lower.includes(m))) return true;
  const hits = markers.filter((m) => lower.includes(m)).length;
  return hits >= 2 || (hits >= 1 && text.length > 160);
}

/** Daily rollup rows worth showing (skip empty summaries and model reasoning leaks). */
export function isListableSummary(session) {
  const text = (session.summary ?? '').trim();
  if (!text) return false;
  if (summaryLooksLikeReasoning(text)) return false;
  return true;
}

/** Nav highlight — Timeline is default; Sessions includes detail routes. */
export function isNavActive(pathname, href) {
  const path = pathname || '/';
  if (href === '/') {
    return path === '/' || path === '' || path.endsWith('/index.html');
  }
  if (href === '/sessions') {
    return path === '/sessions' || path.startsWith('/sessions/');
  }
  if (href === '/projects') {
    return path === '/projects' || path.startsWith('/projects/');
  }
  return path === href;
}

/** @param {string | undefined} path */
function basename(path) {
  if (!path) return '';
  const parts = path.split('/');
  return parts[parts.length - 1] || path;
}

/** @param {string} type */
function humanizeType(type) {
  return type.replaceAll('.', ' · ').replaceAll('_', ' ');
}

/** @param {Record<string, unknown>} event */

/** @param {Record<string, unknown>} event */
function osAppName(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  return meta.app_name || /** @type {string} */ (event.source) || 'App';
}

/** @param {Record<string, unknown>} event */
/** Prefer normalized tab title for any focused window (IDE, browser, Finder, terminal). */
export function osDisplayLabel(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const app = osAppName(event);
  const tab = meta.tab_title?.trim() || meta.window_title?.trim();
  const title = tab ? normalizeDisplayTitle(tab) : null;
  if (title) return truncate(`${app} — ${title}`, 72);
  return app;
}

/** Strip volatile UI suffixes when rendering (mirrors Rust normalize_tab_title). */
function normalizeDisplayTitle(title) {
  let t = title.trim();
  for (const marker of [' - ⏳', ' — ⏳', ' – ⏳', ' ⏳']) {
    const idx = t.indexOf(marker);
    if (idx >= 0) {
      t = t.slice(0, idx);
      break;
    }
  }
  t = t.replace(/^[●•◦*○]\s*/, '').replace(/\s*[●•◦*○]$/, '');
  for (const suffix of [' (unsaved)', ' — unsaved', ' - unsaved', ' (modified)']) {
    if (t.toLowerCase().endsWith(suffix)) {
      t = t.slice(0, -suffix.length);
      break;
    }
  }
  return t.trim();
}

/** True when a focus event has a real window/tab title (not just the app name). */
export function hasMeaningfulTabTitle(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const app = (meta.app_name || /** @type {string} */ (event.source) || '').trim();
  const tab = normalizeDisplayTitle(meta.tab_title?.trim() || meta.window_title?.trim() || '');
  if (!tab) return false;
  if (tab.toLowerCase() === app.toLowerCase()) return false;
  if (tab === '_default') return false;
  return true;
}

/** Hide generic app-switch noise in the live feed. */
export function isInterestingActivity(event) {
  const category = event.category;
  const type = /** @type {string} */ (event.type ?? '');
  if (category === 'shell' || category === 'git' || category === 'filesystem' || category === 'ide' || category === 'build' || category === 'ai') {
    return true;
  }
  if (category !== 'os') return true;
  if (type === 'window.focus') return false;
  if (type === 'process.focus') {
    return hasMeaningfulTabTitle(event);
  }
  return activityLabel(event) != null;
}

/** Category chip for timeline rows — never "Focus". */
export function shouldShowCategoryBadge(event) {
  if (event.category !== 'os') return true;
  if (activityLabel(event)) return false;
  return /** @type {string} */ (event.type ?? '') === 'window.focus';
}


export function activityLabel(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  return meta.activity_label ?? null;
}

/** @param {Record<string, unknown>} span */
export function spanActivityLabels(span) {
  const meta = /** @type {Record<string, string[]>} */ (span.metadata ?? {});
  const labels = meta.activity_labels;
  if (!Array.isArray(labels)) return [];
  return labels.map((label) =>
    String(label)
      .replaceAll('_', ' ')
      .replace(/\b\w/g, (c) => c.toUpperCase()),
  );
}

/** @param {Record<string, unknown>} event */
export function eventLabel(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const category = event.category;
  const type = /** @type {string} */ (event.type ?? '');

  if (category === 'os' && (type === 'process.focus' || type === 'window.focus')) {
    return osDisplayLabel(event);
  }
  if (category === 'browser') {
    const domain = meta.domain?.trim() || 'page';
    const title = normalizeDisplayTitle(meta.title?.trim() || '');
    if (title) return truncate(`${domain} — ${title}`, 72);
    return domain;
  }
  if (meta.report_line) {
    const line = meta.report_line.replace(/^Focused\s+/i, '').replace(/^Switched to\s+/i, '');
    return truncate(line, 72);
  }
  if (meta.app_name) return meta.app_name;
  if (category === 'shell' && meta.command) return truncate(meta.command, 56);
  if (category === 'filesystem' && meta.path) return basename(meta.path);
  if (category === 'git' && meta.branch) return `git · ${meta.branch}`;
  if (category === 'git' && meta.reflog) return truncate(meta.reflog, 56);

  if (event.source && event.source !== 'chronicle-daemon') return /** @type {string} */ (event.source);

  return humanizeType(type) || 'activity';
}

/** @param {Record<string, unknown>} event @param {{ count?: number, earliest?: number, latest?: number }} [group] */
export function eventSubtitle(event, group) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const category = event.category;
  const type = /** @type {string} */ (event.type ?? '');
  const parts = [];

  if (group && group.count > 1 && group.earliest != null && group.latest != null) {
    parts.push(`${formatTime(group.earliest)}–${formatTime(group.latest)}`);
    const visitWord =
      event.category === 'browser' || (event.category === 'os' && hasMeaningfulTabTitle(event))
        ? 'visits'
        : 'switches';
    parts.push(`${group.count} ${visitWord}`);
  }

  if (category === 'os' && (type === 'process.focus' || type === 'window.focus')) {
    if (hasMeaningfulTabTitle(event) && meta.window_title) {
      parts.push(truncate(meta.window_title, 80));
    } else if (type === 'window.focus') {
      parts.push('window changed');
    } else if (!group || group.count <= 1) {
      parts.push(meta.app_name || 'app switch');
    }
  } else if (category === 'filesystem' && meta.path) {
    parts.push(truncate(meta.path, 80));
  } else if (category === 'shell' && meta.cwd) {
    parts.push(truncate(meta.cwd, 60));
    if (meta.exit_code && meta.exit_code !== '0') parts.push(`exit ${meta.exit_code}`);
  } else if (category === 'git' && meta.reflog) {
    parts.push(truncate(meta.reflog, 80));
  } else {
    parts.push(humanizeType(type));
  }

  const project = /** @type {string | undefined} */ (event.project);
  const label = /** @type {string} */ (eventLabel(event));
  if (project && project.toLowerCase() !== label.toLowerCase()) {
    parts.push(project);
  }

  return parts.filter(Boolean).join(' · ');
}

/** @param {Record<string, unknown>} event */
export function eventCategoryLabel(event) {
  if (event.category === 'os') {
    const act = activityLabel(event);
    if (act === 'agent session') return 'Agent';
    if (act === 'terminal') return 'Terminal';
    if (act === 'coding') return 'IDE';
    if (act === 'browsing') return 'Browser';
    if (act === 'files') return 'Files';
    if (/** @type {string} */ (event.type ?? '') === 'window.focus') return 'Window';
    return 'App';
  }
  const map = {
    shell: 'Terminal',
    git: 'Git',
    filesystem: 'File',
    ide: 'IDE',
    browser: 'Browser',
    build: 'Build',
    ai: 'AI',
  };
  return map[/** @type {keyof typeof map} */ (event.category)] ?? event.category ?? 'Event';
}

/** @param {Record<string, unknown>} event */
export function eventIconKey(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const category = event.category;
  const type = /** @type {string} */ (event.type ?? '');

  // Only resolve macOS app icons for real window-focus events.
  if (category === 'os' && type === 'process.focus') {
    if (meta.bundle_id || meta.app_name) {
      return {
        type: 'app',
        bundleId: meta.bundle_id || null,
        appName: meta.app_name || null,
      };
    }
  }

  if (category === 'git') return { type: 'category', category: 'git' };
  if (category === 'shell') return { type: 'category', category: 'shell' };
  if (category === 'filesystem') return { type: 'category', category: 'filesystem' };

  return null;
}

/** @param {Record<string, unknown>} event */
export function eventIconChar(event) {
  const label = /** @type {string} */ (eventLabel(event));
  return (label || '?').charAt(0).toUpperCase();
}

/** @param {Record<string, unknown>} event */
function collapseIdentity(event) {
  if (event.category === 'browser') {
    const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
    const domain = meta.domain?.trim() || '';
    const title = normalizeDisplayTitle(meta.title?.trim() || '');
    return `browser:page.focus:${domain}:${title}`;
  }
  if (event.category === 'os' && !hasMeaningfulTabTitle(event)) {
    const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
    const app = meta.app_name || /** @type {string} */ (event.source) || 'App';
    const act = activityLabel(event) || 'app';
    return `os:generic:${app}:${act}`;
  }
  return `${event.category}:${event.type}:${eventLabel(event)}`;
}

/** @param {Record<string, unknown>} event */
function shouldGlobalVisitCollapse(event) {
  return event.category === 'browser' || (event.category === 'os' && hasMeaningfulTabTitle(event));
}

const VISIT_COLLAPSE_MS = 2 * 60 * 60 * 1000;

/**
 * Collapse duplicate events in the live feed (global revisit merge for browser/tab focus).
 * @param {Array<Record<string, unknown> & { timestamp: number }>} events
 */
export function collapseTimelineEvents(events) {
  /** @type {Map<string, number>} */
  const globalIndex = new Map();
  /** @type {Array<{ event: typeof events[number], count: number, latest: number, earliest: number }>} */
  const collapsed = [];

  for (const event of events) {
    const identity = collapseIdentity(event);
    const globalMerge = shouldGlobalVisitCollapse(event);
    const windowMs = globalMerge
      ? VISIT_COLLAPSE_MS
      : hasMeaningfulTabTitle(event)
        ? 30_000
        : 30 * 60_000;

    if (globalMerge) {
      const idx = globalIndex.get(identity);
      if (idx !== undefined) {
        const prev = collapsed[idx];
        if (prev.latest - event.timestamp < windowMs) {
          prev.count += 1;
          prev.earliest = Math.min(prev.earliest, event.timestamp);
          prev.latest = Math.max(prev.latest, event.timestamp);
          continue;
        }
      }
      const entry = { event, count: 1, latest: event.timestamp, earliest: event.timestamp };
      globalIndex.set(identity, collapsed.length);
      collapsed.push(entry);
      continue;
    }

    const prev = collapsed[collapsed.length - 1];
    if (
      prev &&
      !shouldGlobalVisitCollapse(prev.event) &&
      collapseIdentity(prev.event) === identity &&
      prev.latest - event.timestamp < windowMs
    ) {
      prev.count += 1;
      prev.earliest = Math.min(prev.earliest, event.timestamp);
      prev.latest = Math.max(prev.latest, event.timestamp);
      continue;
    }
    collapsed.push({ event, count: 1, latest: event.timestamp, earliest: event.timestamp });
  }

  return collapsed.sort((a, b) => b.latest - a.latest);
}

export function highlightMatch(text, query) {
  if (!text || !query.trim()) return text;
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const regex = new RegExp(`(${escaped})`, 'gi');
  return text.replace(regex, '<mark class="bg-teal-500/20 text-teal-700 dark:text-teal-300 rounded px-0.5">$1</mark>');
}

/** App label from span metadata (live focus session). */
export function spanAppName(span) {
  const meta = /** @type {Record<string, string>} */ (span.metadata ?? {});
  const app = meta.app_name?.trim();
  const tab = meta.tab_title?.trim();
  if (app && tab && tab.toLowerCase() !== app.toLowerCase()) {
    return truncate(`${app} — ${tab}`, 72);
  }
  return app || null;
}
