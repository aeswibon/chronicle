export function formatTime(ts) {
  return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
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
export function activityLabel(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  return meta.activity_label ?? null;
}

/** @param {Record<string, unknown>} span */
export function spanActivityLabels(span) {
  const meta = /** @type {Record<string, string[]>} */ (span.metadata ?? {});
  const labels = meta.activity_labels;
  return Array.isArray(labels) ? labels : [];
}

/** @param {Record<string, unknown>} event */
export function eventLabel(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const category = event.category;
  const type = /** @type {string} */ (event.type ?? '');

  if (meta.app_name) return meta.app_name;
  if (category === 'shell' && meta.command) return truncate(meta.command, 56);
  if (category === 'filesystem' && meta.path) return basename(meta.path);
  if (category === 'git' && meta.branch) return `git · ${meta.branch}`;
  if (category === 'git' && meta.reflog) return truncate(meta.reflog, 56);

  if (event.source && event.source !== 'chronicle-daemon') return /** @type {string} */ (event.source);

  return humanizeType(type) || 'activity';
}

/** @param {Record<string, unknown>} event */
export function eventSubtitle(event) {
  const meta = /** @type {Record<string, string>} */ (event.metadata ?? {});
  const category = event.category;
  const type = /** @type {string} */ (event.type ?? '');
  const parts = [];

  if (category === 'os' && type === 'process.focus') {
    if (meta.window_title) parts.push(truncate(meta.window_title, 80));
    else parts.push('focused');
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
  const map = {
    os: 'Focus',
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
function eventIdentity(event) {
  return `${event.category}:${event.type}:${eventLabel(event)}`;
}

/**
 * Collapse consecutive duplicate events in the live feed.
 * @param {Array<Record<string, unknown> & { timestamp: number }>} events
 */
export function collapseTimelineEvents(events) {
  /** @type {Array<{ event: typeof events[number], count: number, latest: number }>} */
  const collapsed = [];

  for (const event of events) {
    const identity = eventIdentity(event);
    const prev = collapsed[collapsed.length - 1];
    if (
      prev &&
      eventIdentity(prev.event) === identity &&
      prev.latest - event.timestamp < 30_000
    ) {
      prev.count += 1;
      prev.latest = Math.max(prev.latest, event.timestamp);
      continue;
    }
    collapsed.push({ event, count: 1, latest: event.timestamp });
  }

  return collapsed;
}

export function highlightMatch(text, query) {
  if (!text || !query.trim()) return text;
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const regex = new RegExp(`(${escaped})`, 'gi');
  return text.replace(regex, '<mark class="bg-teal-500/20 text-teal-700 dark:text-teal-300 rounded px-0.5">$1</mark>');
}
