/* @ds-bundle: {"format":4,"namespace":"Omega","components":[{"name":"Button"},{"name":"Status"},{"name":"Tile"},{"name":"CodeBlock"},{"name":"PlaceGrid"}]} */
(function () {
  var h = window.React.createElement;

  function classes() {
    return Array.prototype.filter.call(arguments, Boolean).join(' ');
  }

  function without(props, names) {
    var rest = {};
    for (var key in props) {
      if (Object.prototype.hasOwnProperty.call(props, key) && names.indexOf(key) < 0) rest[key] = props[key];
    }
    return rest;
  }

  // The two marks the system draws: a check for proved, a cross for a trap.
  function Mark(props) {
    var path = props.kind === 'check' ? 'M4 12.5 L9.5 18 L20 6.5' : 'M6 6 L18 18 M18 6 L6 18';
    return h('svg', { className: 'om-mark', width: 10, height: 10, viewBox: '0 0 24 24', 'aria-hidden': 'true' },
      h('path', { d: path, fill: 'none', stroke: 'currentColor', strokeWidth: 3.5, strokeLinecap: 'square', strokeLinejoin: 'miter' }));
  }

  function Button(props) {
    var variant = props.variant || 'solid';
    var rest = without(props, ['variant', 'className']);
    rest.type = rest.type || 'button';
    rest.className = classes('om-button', 'om-button-' + variant, props.className);
    return h('button', rest);
  }

  var WORDS = { checked: 'Checked', trap: 'Trap', open: 'Open' };

  function Status(props) {
    var state = props.state || 'open';
    var mark = state === 'checked' ? h(Mark, { kind: 'check' }) : state === 'trap' ? h(Mark, { kind: 'cross' }) : null;
    return h('span', { className: classes('om-status', 'om-status-' + state, props.className) },
      h('span', { className: 'om-pip', 'aria-hidden': 'true' }, mark),
      h('span', { className: 'om-status-word' }, props.children || WORDS[state]));
  }

  function Tile(props) {
    var variant = props.variant || 'outline';
    return h('section', { className: classes('om-tile', 'om-tile-' + variant, props.className) },
      props.label ? h('span', { className: 'om-tile-label' }, props.label) : null,
      props.title ? h('h3', { className: 'om-tile-title' }, props.title) : null,
      props.children ? h('div', { className: 'om-tile-body' }, props.children) : null);
  }

  function CodeBlock(props) {
    return h('figure', { className: classes('om-code', props.className) },
      props.caption ? h('figcaption', { className: 'om-code-caption' }, props.caption) : null,
      h('pre', { className: 'om-code-pre' }, h('code', null, props.children)));
  }

  var PLACES = ['Local', 'Field', 'Parameter', 'Call result', 'Match join', 'Const name', 'Nested machine', 'Another machine', 'Another target'];

  function PlaceGrid(props) {
    var results = props.results || [];
    var compact = props.size === 'compact';
    var count = 0;
    for (var i = 0; i < 9; i++) if (results[i] === 'checked') count++;
    var summary = count + ' of 9 places checked';
    var cells = PLACES.map(function (name, index) {
      var result = results[index] || 'open';
      var number = (index + 1 < 10 ? '0' : '') + (index + 1);
      return h('li', { key: name, className: classes('om-place', 'om-place-' + result), title: compact ? name + ': ' + WORDS[result] : undefined },
        compact ? null : h('span', { className: 'om-place-number' }, number),
        compact ? null : h('span', { className: 'om-place-name' }, name),
        compact ? null : h(Status, { state: result }));
    });
    return h('div', { className: classes('om-places', compact && 'om-places-compact', props.className) },
      props.fact && !compact ? h('span', { className: 'om-places-fact' }, props.fact) : null,
      h('ol', { className: 'om-place-grid', 'aria-label': (props.fact ? props.fact + ': ' : '') + summary }, cells),
      compact ? null : h('p', { className: 'om-places-count' }, summary));
  }

  window.Omega = Object.assign(window.Omega || {}, {
    Button: Button,
    Status: Status,
    Tile: Tile,
    CodeBlock: CodeBlock,
    PlaceGrid: PlaceGrid
  });
})();
