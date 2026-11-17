/* Progressive enhancement: source traces stay readable without scripting. */
(() => {
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
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

  const explorer = document.querySelector('.flow-explorer');
  if (explorer) {
    const data = JSON.parse(document.getElementById('flow-model').textContent);
    const switcher = explorer.querySelector('.view-switch');
    switcher.hidden = false;
    explorer.classList.add('is-enhanced');
    let view, panel, index = 0, timer = null, playing = false;
    const stop = () => {
      clearTimeout(timer); timer = null; playing = false;
      explorer.classList.remove('is-playing');
      if (panel) {
        panel.querySelector('[data-action="play"]').textContent = 'Play walkthrough';
        panel.querySelector('[data-action="play"]').setAttribute('aria-pressed', 'false');
      }
    };
    const reveal = id => {
      const node = panel.querySelector(`[data-node="${id}"]`);
      const frame = panel.querySelector('.diagram-frame');
      const box = node.getBoundingClientRect(), viewport = frame.getBoundingClientRect();
      // Pan only the figure, never move keyboard focus or scroll the whole page.
      if (box.left < viewport.left || box.right > viewport.right)
        frame.scrollLeft += (box.left + box.right - viewport.left - viewport.right) / 2;
    };
    const inspect = (id, pan = true) => {
      const selected = view.nodes.find(n => n.id === id);
      const aside = panel.querySelector('.flow-inspector');
      const wasOpen = aside.querySelector('details')?.open ?? matchMedia('(min-width: 1100px)').matches;
      const disclosure = make('details', '', 'inspector-disclosure'); disclosure.open = wasOpen;
      const summary = make('summary');
      summary.append(make('strong', selected.label), make('span', selected.description.split('. ')[0] + '.'), make('span', 'Inputs, outputs and source', 'inspector-prompt'));
      const contents = make('div', '', 'inspector-contents');
      const heading = make('h3', selected.label); heading.tabIndex = -1;
      contents.append(make('p', view.id === 'dependencies' ? 'Package declaration' : 'Selected subsystem', 'eyebrow'), heading, make('p', selected.description));
      disclosure.append(summary, contents); aside.replaceChildren(disclosure);
      if (view.id === 'dependencies') contents.append(make('code', selected.id));
      for (const direction of ['incoming', 'outgoing']) {
        const incoming = direction === 'incoming';
        const edges = view.edges.filter(e => (incoming ? e.end : e.start) === id);
        contents.append(make('h4', view.id === 'dependencies' ? (incoming ? 'Declared by' : 'Declares dependencies on') : (incoming ? 'Receives from' : 'Supplies to')));
        if (!edges.length) contents.append(make('p', 'No connection in this selected view.', 'connection-empty'));
        const list = make('ul');
        for (const edge of edges) {
          const neighbor = view.nodes.find(n => n.id === (incoming ? edge.start : edge.end));
          const item = make('li'), button = make('button', neighbor.label);
          button.type = 'button';
          button.addEventListener('click', () => {
            selectNode(neighbor.id);
            panel.querySelector('.inspector-contents h3').focus({preventScroll: true});
          });
          item.append(button, make('span', edge.label)); list.append(item);
        }
        contents.append(list);
      }
      const links = make('p', '', 'inspector-links');
      const reference = panel.querySelector('.flow-reference');
      const source = make('a', 'Pinned source'); source.href = selected.source;
      const detail = make('a', 'Open subsystem →');
      detail.href = new URL(`${location.pathname.endsWith('index.html') || location.pathname.endsWith('/') ? 'pages/' : ''}${selected.page}.html`, location.href).href;
      links.append(detail, source);
      if (selected.example_source) { const example = make('a', 'Console binding example'); example.href = selected.example_source; links.append(example); }
      contents.append(links);
      selectConnections(panel.querySelector('figure'), id, index >= 0 ? view.steps[index]?.edges || [] : []);
      if (pan) reveal(id);
      reference.dataset.selected = id;
    };
    const controls = () => {
      panel.querySelector('[data-action="previous"]').disabled = index <= 0;
      panel.querySelector('[data-action="next"]').disabled = index === view.steps.length - 1;
      const play = panel.querySelector('[data-action="play"]');
      play.disabled = reduced.matches;
      panel.querySelector('.tour-note').textContent = reduced.matches ? 'Reduced motion is enabled. Use Next and Previous to read at your own pace.' : 'A guided reading, not execution timing. Starts paused and stops at the last step.';
    };
    const showStep = (next, pan = true) => {
      index = Math.max(0, Math.min(view.steps.length - 1, next));
      const step = view.steps[index];
      panel.querySelector('.tour-position').textContent = `Step ${index + 1} of ${view.steps.length}`;
      panel.querySelector('.tour-copy').replaceChildren(make('h3', step.title), make('p', step.text));
      inspect(step.node, pan); controls();
    };
    const selectNode = id => {
      stop(); index = -1;
      panel.querySelector('.tour-position').textContent = 'Exploring connections';
      panel.querySelector('.tour-copy').replaceChildren(make('p', 'The heavier lines touch the selected box. Next starts the walkthrough again.'));
      inspect(id); controls();
    };
    const schedule = () => {
      timer = setTimeout(() => {
        if (!playing) return;
        if (index >= view.steps.length - 1) { stop(); return; }
        showStep(index + 1);
        if (index === view.steps.length - 1) stop(); else schedule();
      }, 5000);
    };
    const changeView = id => {
      stop(); view = data.find(v => v.id === id);
      panel = explorer.querySelector(`[data-view-panel="${id}"]`);
      explorer.querySelectorAll('.flow-view').forEach(p => { p.hidden = p !== panel; });
      panel.querySelector('.diagram-frame').scrollLeft = 0;
      switcher.querySelectorAll('button').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.view === id)));
      if (view.steps.length) showStep(0, false); else { index = -1; inspect(view.nodes[0].id, false); controls(); }
    };
    data.forEach(model => {
      const p = explorer.querySelector(`[data-view-panel="${model.id}"]`);
      p.querySelector('svg').setAttribute('role', 'group');
      p.querySelector('.flow-inspector').hidden = false;
      p.querySelector('.tour').hidden = !model.steps.length;
      p.querySelectorAll('.node').forEach(node => activate(node, `Inspect ${model.nodes.find(n => n.id === node.dataset.node).label}`, () => selectNode(node.dataset.node)));
      p.querySelector('[data-action="previous"]').addEventListener('click', () => { stop(); showStep(index - 1); });
      p.querySelector('[data-action="next"]').addEventListener('click', () => { stop(); showStep(index + 1); });
      p.querySelector('[data-action="play"]').addEventListener('click', () => {
        if (playing) { stop(); return; }
        if (reduced.matches) return;
        if (index < 0 || index === view.steps.length - 1) showStep(0);
        playing = true; explorer.classList.add('is-playing');
        const button = panel.querySelector('[data-action="play"]');
        button.textContent = 'Pause'; button.setAttribute('aria-pressed', 'true'); schedule();
      });
    });
    switcher.querySelectorAll('button').forEach(b => b.addEventListener('click', () => changeView(b.dataset.view)));
    reduced.addEventListener('change', () => { stop(); controls(); });
    matchMedia('(min-width: 1100px)').addEventListener('change', event => {
      explorer.querySelectorAll('.inspector-disclosure').forEach(d => { d.open = event.matches; });
    });
    document.addEventListener('visibilitychange', () => { if (document.hidden) stop(); });
    window.addEventListener('pagehide', stop);
    changeView('native');
  }

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
