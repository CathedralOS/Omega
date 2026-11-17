/* Progressive enhancement: source traces stay readable without scripting. */
(() => {
  const make = (tag, text, className) => {
    const el = document.createElement(tag);
    if (text) el.textContent = text;
    if (className) el.className = className;
    return el;
  };
  const activate = (el, label, callback) => {
    el.setAttribute('tabindex', '0');
    el.setAttribute('role', 'button');
    el.setAttribute('aria-label', label);
    el.setAttribute('aria-pressed', 'false');
    el.addEventListener('click', callback);
    el.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault(); callback();
      }
    });
  };
  const selectConnections = (figure, id, current = []) => {
    figure.classList.toggle('has-selection', Boolean(id));
    const neighbors = new Set([id]);
    figure.querySelectorAll('.edge').forEach(edge => {
      const related = edge.dataset.from === id || edge.dataset.to === id;
      edge.classList.toggle('is-related', related);
      edge.classList.toggle('is-current', current.includes(edge.dataset.edge));
      if (related) { neighbors.add(edge.dataset.from); neighbors.add(edge.dataset.to); }
    });
    figure.querySelectorAll('.node').forEach(node => {
      node.classList.toggle('is-selected', node.dataset.node === id);
      node.classList.toggle('is-neighbor', neighbors.has(node.dataset.node));
      node.setAttribute('aria-pressed', String(node.dataset.node === id));
    });
  };

  document.querySelectorAll('figure.diagram:not(.overview-diagram)').forEach(figure => {
    const nodes = [...figure.querySelectorAll('.node[data-node]')];
    if (!nodes.length) return;
    figure.querySelector('svg').setAttribute('role', 'group');
    const controls = make('div', '', 'diagram-selection');
    const status = make('p', 'Select a box to follow its immediate connections.');
    status.setAttribute('aria-live', 'polite');
    const reset = make('button', 'Show all'); reset.type = 'button'; reset.hidden = true;
    reset.addEventListener('click', () => {
      const selected = figure.querySelector('.node[aria-pressed="true"]') || nodes[0];
      const restore = document.activeElement === reset;
      selectConnections(figure, null); status.textContent = 'Select a box to follow its immediate connections.'; reset.hidden = true;
      if (restore) selected.focus({preventScroll: true});
    });
    controls.append(status, reset); figure.querySelector('.diagram-frame').after(controls);
    nodes.forEach(node => {
      const label = [...node.querySelectorAll('text')].map(t => t.textContent).join(' ');
      activate(node, `Inspect connections for ${label}`, () => {
        const id = node.dataset.node;
        selectConnections(figure, id); reset.hidden = false;
        const connections = [...figure.querySelectorAll('.edge')].filter(e => e.dataset.from === id || e.dataset.to === id).map(e => {
          const incoming = e.dataset.to === id;
          const neighbor = nodes.find(n => n.dataset.node === (incoming ? e.dataset.from : e.dataset.to));
          const name = [...neighbor.querySelectorAll('text')].map(t => t.textContent).join(' ');
          const payload = [...e.querySelectorAll('text')].map(t => t.textContent).join(' ');
          return `${incoming ? 'From' : 'To'} ${name}: ${payload}`;
        });
        status.textContent = label + '. ' + connections.join('. ');
      });
    });
  });
})();
