(() => {
  const root = document.documentElement;
  const button = document.getElementById('theme');
  try {
    const saved = localStorage.getItem('omega-atlas-theme');
    if (saved === 'light' || saved === 'dark') root.dataset.theme = saved;
  } catch (_) { /* A file preview may disable browser storage. */ }
  const dark = () => root.dataset.theme ? root.dataset.theme === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
  const label = () => { button.textContent = dark() ? 'Ink → Limestone' : 'Limestone → Ink'; };
  button.addEventListener('click', () => {
    root.dataset.theme = dark() ? 'light' : 'dark';
    try { localStorage.setItem('omega-atlas-theme', root.dataset.theme); } catch (_) {}
    label();
  });
  label();
  const wide = matchMedia('(min-width: 980px)');
  const pageList = document.getElementById('page-list');
  const fitNavigation = () => { pageList.open = wide.matches; };
  wide.addEventListener('change', fitNavigation);
  fitNavigation();
  document.getElementById('page-filter').addEventListener('input', event => {
    const query = event.target.value.toLowerCase().trim();
    let count = 0;
    document.querySelectorAll('.atlas-nav li').forEach(item => {
      item.hidden = !item.textContent.toLowerCase().includes(query);
      if (!item.hidden) count++;
    });
    document.getElementById('no-pages').hidden = count > 0;
  });
})();
