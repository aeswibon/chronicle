const DEFAULT_DOMAINS = ['github.com', 'stackoverflow.com', 'docs.rs', 'developer.mozilla.org'];

document.getElementById('save').addEventListener('click', async () => {
  const raw = document.getElementById('domains').value;
  const domains = raw
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean);
  await chrome.storage.sync.set({ domains });
  document.getElementById('status').textContent = 'Saved';
});

chrome.storage.sync.get({ domains: DEFAULT_DOMAINS }, ({ domains }) => {
  document.getElementById('domains').value = domains.join('\n');
});
