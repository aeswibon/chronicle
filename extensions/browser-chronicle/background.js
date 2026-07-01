const INGRESS = 'http://127.0.0.1:9713/v1/events';
const DEFAULT_DOMAINS = ['github.com', 'stackoverflow.com', 'docs.rs', 'developer.mozilla.org'];

async function allowedDomains() {
  const stored = await chrome.storage.sync.get({ domains: DEFAULT_DOMAINS });
  return stored.domains.filter(Boolean);
}

function hostAllowed(hostname, domains) {
  if (!domains.length) return true;
  return domains.some(
    (d) => hostname === d || hostname.endsWith('.' + d),
  );
}

async function emitPageFocus(tab) {
  if (!tab?.url?.startsWith('http')) return;
  let url;
  try {
    url = new URL(tab.url);
  } catch {
    return;
  }
  const domains = await allowedDomains();
  if (!hostAllowed(url.hostname, domains)) return;

  const event = {
    version: '1.0',
    id: crypto.randomUUID(),
    timestamp: Date.now(),
    source: 'browser-chronicle',
    category: 'browser',
    type: 'page.focus',
    project: null,
    workspace: null,
    duration_ms: null,
    metadata: {
      domain: url.hostname,
      path: url.pathname,
      title: tab.title ?? '',
    },
  };

  try {
    await fetch(INGRESS, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(event),
    });
  } catch {
    // daemon offline — ignore
  }
}

chrome.tabs.onActivated.addListener(async (info) => {
  const tab = await chrome.tabs.get(info.tabId);
  emitPageFocus(tab);
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.status === 'complete') emitPageFocus(tab);
});
