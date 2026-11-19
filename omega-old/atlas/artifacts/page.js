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
  pageList.addEventListener('toggle', () => {
    if(pageList.open)document.getElementById('page-filter').focus({preventScroll:true});
  });
  document.addEventListener('click', event => {
    if(!pageList.contains(event.target))pageList.open=false;
  });
  document.addEventListener('keydown', event => {
    if(event.key==='Escape'&&pageList.open){pageList.open=false;pageList.querySelector('summary').focus();}
  });
  const contents=document.querySelector('.page-sections .contents');
  const fitContents=()=>{if(contents)contents.open=wide.matches;};
  wide.addEventListener('change',fitContents);fitContents();
  const showDiagramHints = () => document.querySelectorAll('.diagram').forEach(figure => {
    const frame = figure.querySelector('.diagram-frame');
    figure.querySelector('.diagram-hint').hidden = frame.scrollWidth <= frame.clientWidth;
  });
  new ResizeObserver(showDiagramHints).observe(document.body);
  document.fonts.ready.then(showDiagramHints);
  document.getElementById('page-filter').addEventListener('input', event => {
    const query = event.target.value.toLowerCase().trim();
    let count = 0;
    document.querySelectorAll('.atlas-nav li').forEach(item => {
      item.hidden = !item.textContent.toLowerCase().includes(query);
      if (!item.hidden) count++;
    });
    document.querySelectorAll('.atlas-nav section').forEach(group=>{
      group.hidden=![...group.querySelectorAll('li')].some(item=>!item.hidden);
    });
    const home=document.querySelector('.atlas-nav .nav-home');
    home.hidden=Boolean(query)&&!home.textContent.toLowerCase().includes(query);
    if(!home.hidden)count++;
    document.getElementById('no-pages').hidden = count > 0;
  });
  if(contents){
    const links=[...contents.querySelectorAll('a[href^="#"]')];
    const selectSection=id=>links.forEach(link=>{
      if(link.getAttribute('href')==='#'+id)link.setAttribute('aria-current','location');
      else link.removeAttribute('aria-current');
    });
    const observer=new IntersectionObserver(entries=>{
      const visible=entries.filter(entry=>entry.isIntersecting).sort((a,b)=>a.boundingClientRect.top-b.boundingClientRect.top);
      if(visible.length)selectSection(visible[0].target.id);
    },{rootMargin:'-8% 0px -65% 0px'});
    links.forEach(link=>{const target=document.getElementById(decodeURIComponent(link.hash.slice(1)));if(target)observer.observe(target);});
    if(location.hash)selectSection(decodeURIComponent(location.hash.slice(1)));
  }
})();
