#[derive(Clone, Debug)]
struct PhaseDiagramNode {
    id: String,
    label: String,
    kind: String,
    rank: usize,
}

#[derive(Clone, Debug)]
struct PhaseDiagramEdge {
    from: String,
    to: String,
    kind: String,
}

pub struct PhaseDiagramBuilder {
    title: String,
    nodes: Vec<PhaseDiagramNode>,
    edges: Vec<PhaseDiagramEdge>,
}

const PIPELINE_PAGES: &[(&str, &str, &str)] = &[
    ("00", "Timings", "00_timings.html"),
    ("02", "Syntax", "02_syntax_trees.html"),
    ("03", "Symbols", "03_symbol_resolved_trees.html"),
    ("04", "Typed", "04_typed_trees.html"),
    ("06", "State Graph", "06_state_graph.html"),
    ("07", "Control Flow", "07_control_flow.html"),
    ("09", "Backend", "09_backend_report.html"),
    ("11", "Emission", "11_emission.html"),
];

impl PhaseDiagramBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn node(
        &mut self,
        id: impl AsRef<str>,
        label: impl Into<String>,
        kind: impl Into<String>,
        rank: usize,
    ) -> String {
        let id = sanitize_id(id.as_ref());
        self.nodes.push(PhaseDiagramNode {
            id: id.clone(),
            label: label.into(),
            kind: kind.into(),
            rank,
        });
        id
    }

    pub fn containment_edge(&mut self, from: &str, to: &str) {
        self.edge(from, to, "contains");
    }

    pub fn edge(&mut self, from: &str, to: &str, kind: impl Into<String>) {
        self.edges.push(PhaseDiagramEdge {
            from: from.to_owned(),
            to: to.to_owned(),
            kind: kind.into(),
        });
    }

    pub fn finish(self) -> String {
        render_html(&self.title, &self.nodes, &self.edges)
    }
}

fn render_html(title: &str, nodes: &[PhaseDiagramNode], edges: &[PhaseDiagramEdge]) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>");
    html.push_str(&escape_html(title));
    html.push_str("</title>\n<style>\n");
    html.push_str(STYLE);
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<aside id=\"panel\">\n");
    html.push_str("<h1>");
    html.push_str(&escape_html(title));
    html.push_str("</h1>\n");
    push_pipeline_nav(&mut html);
    html.push_str("<input id=\"search\" type=\"search\" placeholder=\"Search labels, symbols, states...\" autocomplete=\"off\">\n");
    html.push_str("<div class=\"buttons\"><button id=\"fit\">Fit</button><button id=\"reset\">Reset</button><button id=\"clear-scope\">Clear Scope</button><button id=\"follow-target\" disabled>Follow Target</button></div>\n");
    html.push_str(
        "<label><input id=\"show-sequence\" type=\"checkbox\" checked> Statement flow</label>\n",
    );
    html.push_str(
        "<label><input id=\"show-data\" type=\"checkbox\" checked> Data definitions</label>\n",
    );
    html.push_str("<p id=\"counts\"></p>\n");
    html.push_str("<nav id=\"outline\" aria-label=\"Graph outline\"></nav>\n");
    html.push_str("<pre id=\"details\">Click a node for details.</pre>\n");
    html.push_str("</aside>\n");
    html.push_str("<main><svg id=\"canvas\" role=\"img\" aria-label=\"Phase graph\"><g id=\"viewport\"><g id=\"edges\"></g><g id=\"nodes\"></g></g></svg></main>\n");
    html.push_str("<script>\nconst GRAPH = ");
    push_graph_json(&mut html, title, nodes, edges);
    html.push_str(";\n");
    html.push_str(SCRIPT);
    html.push_str("\n</script>\n</body>\n</html>\n");
    html
}

pub fn pipeline_index_html() -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Pipeline Visualizations</title>\n<style>\n");
    html.push_str(INDEX_STYLE);
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<header><p>Omega build report</p><h1>Pipeline Visualizations</h1>");
    html.push_str("<p class=\"lede\">One landing page for the generated compiler artifacts. Open a phase directly, then use the embedded phase nav to move sideways.</p></header>\n");
    html.push_str("<main class=\"grid\">\n");
    for (number, label, href) in PIPELINE_PAGES {
        html.push_str("<a class=\"card\" href=\"");
        html.push_str(&escape_html(href));
        html.push_str("\"><span>");
        html.push_str(&escape_html(number));
        html.push_str("</span><strong>");
        html.push_str(&escape_html(label));
        html.push_str("</strong><small>");
        html.push_str(&escape_html(href));
        html.push_str("</small></a>\n");
    }
    html.push_str("</main>\n</body>\n</html>\n");
    html
}

pub fn text_report_html(title: &str, contents: &str) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>");
    html.push_str(&escape_html(title));
    html.push_str("</title>\n<style>\n");
    html.push_str(TEXT_REPORT_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<aside>\n<h1>");
    html.push_str(&escape_html(title));
    html.push_str("</h1>\n");
    push_pipeline_nav(&mut html);
    html.push_str("</aside>\n<main><pre>");
    html.push_str(&escape_html(contents));
    html.push_str("</pre></main>\n</body>\n</html>\n");
    html
}

fn push_pipeline_nav(html: &mut String) {
    html.push_str("<nav class=\"phase-nav\" aria-label=\"Pipeline stages\"><a href=\"00_pipeline.html\">Index</a>");
    for (number, label, href) in PIPELINE_PAGES {
        html.push_str("<a href=\"");
        html.push_str(&escape_html(href));
        html.push_str("\"><span>");
        html.push_str(&escape_html(number));
        html.push_str("</span> ");
        html.push_str(&escape_html(label));
        html.push_str("</a>");
    }
    html.push_str("</nav>\n");
}

fn push_graph_json(
    output: &mut String,
    title: &str,
    nodes: &[PhaseDiagramNode],
    edges: &[PhaseDiagramEdge],
) {
    output.push_str("{\"title\":");
    push_json_string(output, title);
    output.push_str(",\"nodes\":[");
    for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        push_json_string(output, &node.id);
        output.push_str(",\"label\":");
        push_json_string(output, &node.label);
        output.push_str(",\"kind\":");
        push_json_string(output, &node.kind);
        output.push_str(",\"rank\":");
        output.push_str(&node.rank.to_string());
        output.push('}');
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in edges.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"from\":");
        push_json_string(output, &edge.from);
        output.push_str(",\"to\":");
        push_json_string(output, &edge.to);
        output.push_str(",\"kind\":");
        push_json_string(output, &edge.kind);
        output.push('}');
    }
    output.push_str("]}");
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => {
                output.push_str("\\u");
                output.push_str(&format!("{:04x}", ch as u32));
            }
            ch => output.push(ch),
        }
    }
    output.push('"');
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = r#"
:root {
  --bg: #101318;
  --panel: #171d25;
  --panel-border: #2a3442;
  --text: #eef3fb;
  --muted: #9caaba;
  --edge: #52616f;
  --sequence: #7fb4ff;
  --match: #ffd166;
}
* { box-sizing: border-box; }
html, body { height: 100%; margin: 0; overflow: hidden; }
body {
  display: grid;
  grid-template-columns: minmax(280px, 22vw) 1fr;
  background: radial-gradient(circle at 20% 0%, #253144 0, #101318 42%);
  color: var(--text);
  font: 14px/1.4 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
#panel {
  border-right: 1px solid var(--panel-border);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  padding: 18px;
  overflow: auto;
  box-shadow: 12px 0 40px rgba(0, 0, 0, 0.25);
  z-index: 2;
}
h1 { margin: 0 0 16px; font-size: 18px; letter-spacing: 0.04em; }
input[type="search"] {
  width: 100%;
  border: 1px solid #354459;
  border-radius: 10px;
  background: #0d1117;
  color: var(--text);
  padding: 10px 12px;
  outline: none;
}
input[type="search"]:focus { border-color: var(--match); }
.buttons { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin: 12px 0; }
button {
  border: 1px solid #354459;
  border-radius: 10px;
  background: #212b38;
  color: var(--text);
  padding: 8px 10px;
  cursor: pointer;
}
button:hover { background: #2b3747; }
label { display: block; color: var(--muted); margin: 10px 0; }
#counts { color: var(--muted); }
#outline {
  border: 1px solid #283343;
  border-radius: 12px;
  background: #0d1117;
  margin: 12px 0;
  max-height: 34vh;
  overflow: auto;
  padding: 8px;
}
#outline button {
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: #d8e2ef;
  display: block;
  font: inherit;
  overflow: hidden;
  padding: 5px 7px;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}
#outline button:hover { background: #202a38; }
#outline button.scoped { background: #263247; color: #ffffff; }
#outline details { margin-left: 10px; }
#outline summary {
  color: var(--muted);
  cursor: pointer;
  margin: 3px 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.phase-nav {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: -4px 0 14px;
}
.phase-nav a {
  border: 1px solid #303d50;
  border-radius: 999px;
  color: #d8e2ef;
  font-size: 11px;
  line-height: 1;
  padding: 7px 9px;
  text-decoration: none;
}
.phase-nav a:hover { background: #263247; border-color: #8ab4ff; }
.phase-nav span { color: var(--muted); }
#details {
  white-space: pre-wrap;
  border: 1px solid #283343;
  border-radius: 12px;
  background: #0d1117;
  color: #d8e2ef;
  padding: 12px;
  min-height: 120px;
}
main { min-width: 0; min-height: 0; position: relative; }
#canvas {
  width: 100%;
  height: 100%;
  cursor: grab;
  user-select: none;
}
#canvas.dragging { cursor: grabbing; }
.edge { stroke: var(--edge); stroke-width: 1.2; fill: none; opacity: 0.45; }
.edge.sequence { stroke: var(--sequence); opacity: 0.55; stroke-dasharray: 6 5; }
.edge.field_type { stroke: #75b57c; opacity: 0.75; stroke-dasharray: 2 4; }
.edge.owned_data { stroke: #ffd166; opacity: 0.8; }
.edge.contained_object { stroke: #ff9f7f; opacity: 0.72; }
.edge.implements_data { stroke: #b089f0; opacity: 0.82; stroke-dasharray: 10 5; }
.edge.satisfies_trait { stroke: #4dd4c6; opacity: 0.82; stroke-dasharray: 8 3; }
.edge.requires_trait { stroke: #ffcf5c; opacity: 0.82; stroke-dasharray: 3 3; }
.edge.call { stroke: #ff9f7f; opacity: 0.82; stroke-dasharray: 8 2 2 2; }
.edge.transition_target { stroke: #4dd4c6; opacity: 0.85; stroke-width: 1.8; stroke-dasharray: 12 4 2 4; }
.edge.transition_continuation { stroke: #8ab4ff; opacity: 0.78; stroke-width: 1.6; stroke-dasharray: 5 4; }
.node rect {
  fill: #151c26;
  stroke: #405168;
  stroke-width: 1;
  rx: 10;
}
.node.root rect { fill: #263247; stroke: #8ab4ff; }
.node.data rect { fill: #1b2a22; stroke: #75b57c; }
.node.trait rect { fill: #162b33; stroke: #4dd4c6; }
.node.machine rect { fill: #272132; stroke: #b089f0; }
.node.object rect { fill: #2e291b; stroke: #ffd166; }
.node.state rect { fill: #1f2b3d; stroke: #70a5d8; }
.node.state_block rect { fill: #142637; stroke: #6fbce6; }
.node.statement rect { fill: #241e1c; stroke: #db8f61; }
.node text { fill: var(--text); font-size: 12px; pointer-events: none; }
.node .subtitle { fill: var(--muted); font-size: 11px; }
.node.dim { opacity: 0.16; }
.edge.dim { opacity: 0.05; }
.node.unrelated { opacity: 0.24; }
.edge.unrelated { opacity: 0.04; }
.edge.related {
  opacity: 1;
  stroke-width: 3;
  filter: drop-shadow(0 0 5px rgba(77, 212, 198, 0.65));
}
.edge.related.transition_target {
  stroke: #6fffea;
  stroke-width: 4;
}
.node.related rect {
  stroke: #6fffea;
  stroke-width: 2.4;
}
.node.hovered rect {
  stroke: #ffffff;
  stroke-width: 3.4;
}
.node.match rect { stroke: var(--match); stroke-width: 3; }
.node.selected rect { stroke: #ffffff; stroke-width: 3; }
.hidden { display: none; }
"#;

const INDEX_STYLE: &str = r#"
:root {
  --bg: #101318;
  --panel: #171d25;
  --panel-border: #2a3442;
  --text: #eef3fb;
  --muted: #9caaba;
  --accent: #4dd4c6;
}
* { box-sizing: border-box; }
body {
  min-height: 100vh;
  margin: 0;
  background:
    radial-gradient(circle at 20% 0%, rgba(77, 212, 198, 0.18), transparent 34rem),
    radial-gradient(circle at 90% 20%, rgba(138, 180, 255, 0.14), transparent 30rem),
    var(--bg);
  color: var(--text);
  font: 14px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
header {
  max-width: 1180px;
  margin: 0 auto;
  padding: 64px 28px 24px;
}
header p:first-child {
  color: var(--accent);
  letter-spacing: 0.16em;
  margin: 0 0 12px;
  text-transform: uppercase;
}
h1 { font-size: clamp(36px, 7vw, 86px); line-height: 0.92; margin: 0; }
.lede { color: var(--muted); max-width: 740px; margin-top: 18px; }
.grid {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  max-width: 1180px;
  margin: 0 auto;
  padding: 20px 28px 72px;
}
.card {
  min-height: 160px;
  border: 1px solid var(--panel-border);
  border-radius: 24px;
  background: linear-gradient(145deg, rgba(23, 29, 37, 0.94), rgba(13, 17, 23, 0.84));
  color: var(--text);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 22px;
  text-decoration: none;
  transition: transform 160ms ease, border-color 160ms ease, background 160ms ease;
}
.card:hover {
  border-color: var(--accent);
  background: linear-gradient(145deg, rgba(30, 44, 54, 0.98), rgba(13, 17, 23, 0.9));
  transform: translateY(-3px);
}
.card span { color: var(--accent); font-size: 13px; }
.card strong { display: block; font-size: 22px; margin-top: auto; }
.card small { color: var(--muted); margin-top: 10px; }
"#;

const TEXT_REPORT_STYLE: &str = r#"
:root {
  --bg: #101318;
  --panel: #171d25;
  --panel-border: #2a3442;
  --text: #eef3fb;
  --muted: #9caaba;
}
* { box-sizing: border-box; }
body {
  min-height: 100vh;
  margin: 0;
  background: radial-gradient(circle at 20% 0%, #253144 0, #101318 42%);
  color: var(--text);
  display: grid;
  grid-template-columns: minmax(280px, 22vw) 1fr;
  font: 14px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
aside {
  border-right: 1px solid var(--panel-border);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  min-height: 100vh;
  padding: 18px;
}
h1 { margin: 0 0 16px; font-size: 18px; letter-spacing: 0.04em; }
.phase-nav {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.phase-nav a {
  border: 1px solid #303d50;
  border-radius: 999px;
  color: #d8e2ef;
  font-size: 11px;
  line-height: 1;
  padding: 7px 9px;
  text-decoration: none;
}
.phase-nav a:hover { background: #263247; border-color: #8ab4ff; }
.phase-nav span { color: var(--muted); }
main {
  min-width: 0;
  overflow: auto;
  padding: 28px;
}
pre {
  background: rgba(13, 17, 23, 0.82);
  border: 1px solid #283343;
  border-radius: 18px;
  color: #d8e2ef;
  line-height: 1.45;
  margin: 0;
  min-height: calc(100vh - 56px);
  overflow: auto;
  padding: 24px;
  white-space: pre;
}
"#;

const SCRIPT: &str = r#"
const svg = document.getElementById("canvas");
const viewport = document.getElementById("viewport");
const edgeLayer = document.getElementById("edges");
const nodeLayer = document.getElementById("nodes");
const search = document.getElementById("search");
const details = document.getElementById("details");
const counts = document.getElementById("counts");
const outline = document.getElementById("outline");
const showSequence = document.getElementById("show-sequence");
const showData = document.getElementById("show-data");
const clearScope = document.getElementById("clear-scope");
const followTarget = document.getElementById("follow-target");
const NS = "http://www.w3.org/2000/svg";

const nodeById = new Map(GRAPH.nodes.map(node => [node.id, node]));
const containmentChildren = new Map();
for (const edge of GRAPH.edges) {
  if (edge.kind !== "contains") continue;
  if (!containmentChildren.has(edge.from)) containmentChildren.set(edge.from, []);
  containmentChildren.get(edge.from).push(edge.to);
}

const NODE_W = 250;
const NODE_H = 76;
const NODE_MAX_W = 560;
const NODE_MAX_LINES = 14;
const LINE_H = 16;
const RANK_GAP = 70;
const ROW_GAP = 18;
const LEFT = 80;
const TOP = 60;
const SECTION_GAP_X = 110;
const SECTION_GAP_Y = 70;
const OWNER_COLUMNS = 2;
const DATA_COLUMNS = 4;
const nodeBoxes = new Map(GRAPH.nodes.map(node => [node.id, measureNode(node)]));
const positions = new Map();
let graphBounds = { x: 0, y: 0, width: 1000, height: 800 };
let selectedId = null;
let hoveredId = null;
let scopedId = null;
let visibleNodeIds = new Set(GRAPH.nodes.map(node => node.id));
let transform = { x: 0, y: 0, scale: 1 };

function layoutGraph() {
  positions.clear();
  if (!nodeById.has("root")) return fallbackLayout();
  const roots = ["root"];

  placeNode(roots[0], LEFT, TOP);
  const rootChildren = containmentChildren.get(roots[0]) || [];
  const dataChildren = rootChildren.filter(id => nodeById.get(id)?.kind === "data");
  const ownerChildren = rootChildren.filter(id => nodeById.get(id)?.kind !== "data");

  let cursorY = TOP + NODE_H + SECTION_GAP_Y;
  if (dataChildren.length > 0) {
    cursorY += layoutDataGrid(dataChildren, LEFT, cursorY, DATA_COLUMNS) + SECTION_GAP_Y;
  }
  layoutOwnerGrid(ownerChildren, LEFT, cursorY);

  for (const rootId of roots.slice(1)) {
    if (!positions.has(rootId)) layoutStandalone(rootId);
  }
}

function layoutDataGrid(ids, x, y, columns) {
  const columnWidth = maxNodeWidth(ids) + ROW_GAP;
  const rowHeights = [];
  ids.forEach((id, index) => {
    const row = Math.floor(index / columns);
    rowHeights[row] = Math.max(rowHeights[row] || 0, nodeBox(id).height);
  });
  ids.forEach((id, index) => {
    const column = index % columns;
    const row = Math.floor(index / columns);
    placeNode(id, x + column * columnWidth, y + rowOffset(rowHeights, row));
  });
  return rowHeights.reduce((sum, height) => sum + height + ROW_GAP, 0);
}

function layoutOwnerGrid(ownerIds, x, y) {
  const widestOwner = maxNodeWidth(ownerIds);
  const widestChild = maxNodeWidth(ownerIds.flatMap(id => containmentChildren.get(id) || []));
  const sectionWidth = widestOwner + widestChild + RANK_GAP;
  const columns = Math.min(OWNER_COLUMNS, Math.max(1, ownerIds.length));
  const columnHeights = new Array(columns).fill(y);
  ownerIds.forEach(ownerId => {
    const column = columnHeights.indexOf(Math.min(...columnHeights));
    const sectionX = x + column * (sectionWidth + SECTION_GAP_X);
    const sectionY = columnHeights[column];
    const sectionHeight = layoutOwnerSection(ownerId, sectionX, sectionY);
    columnHeights[column] += sectionHeight + SECTION_GAP_Y;
  });
}

function layoutOwnerSection(ownerId, x, y) {
  const ownerBox = placeNode(ownerId, x, y);
  const childIds = containmentChildren.get(ownerId) || [];
  const objectIds = childIds.filter(id => nodeById.get(id)?.kind === "object");
  const stateIds = childIds.filter(id => nodeById.get(id)?.kind === "state");
  const objectX = x + ownerBox.width + RANK_GAP;
  objectIds.forEach((objectId, index) => {
    const previousIds = objectIds.slice(0, index);
    const offset = previousIds.reduce((sum, id) => sum + nodeBox(id).height + ROW_GAP, 0);
    placeNode(objectId, objectX, y + offset);
  });
  const objectHeight = objectIds.reduce((sum, id) => sum + nodeBox(id).height + ROW_GAP, 0);
  if (stateIds.length === 0) return Math.max(ownerBox.height, objectHeight);

  let cursorY = y + objectHeight;
  for (const stateId of stateIds) {
    const stateBox = placeNode(stateId, objectX, cursorY);
    cursorY += stateBox.height + ROW_GAP;
  }

  return Math.max(ownerBox.height, cursorY - y - ROW_GAP);
}

function layoutStandalone(id) {
  const y = TOP + Array.from(positions.values()).reduce((sum, box) => sum + box.height + ROW_GAP, 0);
  placeNode(id, LEFT, y);
}

function fallbackLayout() {
  const outgoing = new Map(GRAPH.nodes.map(node => [node.id, []]));
  const incomingCount = new Map(GRAPH.nodes.map(node => [node.id, 0]));
  for (const edge of GRAPH.edges) {
    if (!outgoing.has(edge.from) || !incomingCount.has(edge.to)) continue;
    outgoing.get(edge.from).push(edge.to);
    incomingCount.set(edge.to, incomingCount.get(edge.to) + 1);
  }

  const ranks = new Map();
  const roots = GRAPH.nodes
    .filter(node => (incomingCount.get(node.id) || 0) === 0)
    .map(node => node.id);
  const pendingRoots = roots.length > 0 ? roots : GRAPH.nodes.slice(0, 1).map(node => node.id);

  for (const root of pendingRoots) assignFallbackRanks(root, 0, ranks, outgoing);
  for (const node of GRAPH.nodes) {
    if (!ranks.has(node.id)) assignFallbackRanks(node.id, node.rank, ranks, outgoing);
  }

  const rowsByRank = new Map();
  const rowHeightsByRank = new Map();
  GRAPH.nodes.forEach(node => {
    const rank = ranks.get(node.id) || 0;
    const row = rowsByRank.get(rank) || 0;
    rowsByRank.set(rank, row + 1);
    const rankHeights = rowHeightsByRank.get(rank) || [];
    rankHeights[row] = Math.max(rankHeights[row] || 0, nodeBox(node.id).height);
    rowHeightsByRank.set(rank, rankHeights);
  });

  GRAPH.nodes.forEach(node => {
    const rank = ranks.get(node.id) || 0;
    const rankHeights = rowHeightsByRank.get(rank) || [];
    const placedInRank = Array.from(positions.keys())
      .filter(id => (ranks.get(id) || 0) === rank)
      .length;
    placeNode(
      node.id,
      LEFT + rank * (NODE_MAX_W + RANK_GAP),
      TOP + rowOffset(rankHeights, placedInRank)
    );
  });
}

function measureNode(node) {
  const lines = node.label.split("\n");
  const shownLines = Math.min(lines.length, NODE_MAX_LINES);
  const maxChars = lines.slice(0, NODE_MAX_LINES).reduce((max, line) => Math.max(max, line.length), 0);
  return {
    width: Math.min(NODE_MAX_W, Math.max(NODE_W, 28 + maxChars * 7.2)),
    height: Math.max(NODE_H, 30 + shownLines * LINE_H + (lines.length > NODE_MAX_LINES ? LINE_H : 0))
  };
}

function nodeBox(id) {
  return nodeBoxes.get(id) || { width: NODE_W, height: NODE_H };
}

function placeNode(id, x, y) {
  const box = nodeBox(id);
  const positioned = { x, y, width: box.width, height: box.height };
  positions.set(id, positioned);
  return positioned;
}

function maxNodeWidth(ids) {
  return ids.reduce((max, id) => Math.max(max, nodeBox(id).width), NODE_W);
}

function rowOffset(rowHeights, row) {
  let offset = 0;
  for (let index = 0; index < row; index += 1) offset += (rowHeights[index] || NODE_H) + ROW_GAP;
  return offset;
}

function assignFallbackRanks(root, rootRank, ranks, outgoing) {
  const queue = [{ id: root, rank: rootRank }];
  while (queue.length > 0) {
    const next = queue.shift();
    if (ranks.has(next.id)) continue;
    ranks.set(next.id, next.rank);
    for (const target of outgoing.get(next.id) || []) {
      queue.push({ id: target, rank: next.rank + 1 });
    }
  }
}

layoutGraph();

function calculateBounds() {
  calculateBoundsFor(visibleNodeIds);
}

function calculateBoundsFor(nodeIds) {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  const ids = nodeIds && nodeIds.size > 0 ? nodeIds : new Set(GRAPH.nodes.map(node => node.id));
  for (const id of ids) {
    const box = positions.get(id);
    if (!box) continue;
    minX = Math.min(minX, box.x);
    minY = Math.min(minY, box.y);
    maxX = Math.max(maxX, box.x + box.width);
    maxY = Math.max(maxY, box.y + box.height);
  }
  if (!Number.isFinite(minX)) {
    graphBounds = { x: 0, y: 0, width: 1000, height: 800 };
    return;
  }
  graphBounds = {
    x: minX - 80,
    y: minY - 80,
    width: maxX - minX + 160,
    height: maxY - minY + 160
  };
}

function el(name, attrs = {}) {
  const node = document.createElementNS(NS, name);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  return node;
}

function render() {
  edgeLayer.replaceChildren();
  nodeLayer.replaceChildren();
  for (const edge of GRAPH.edges) drawEdge(edge);
  for (const node of GRAPH.nodes) drawNode(node);
  renderOutline();
  calculateBounds();
  applyFilters();
}

function renderOutline() {
  outline.replaceChildren();
  if (!nodeById.has("root")) return renderFlatOutline();
  const rootId = "root";
  const topLevel = containmentChildren.get(rootId) || [];
  const groups = [
    ["Data", topLevel.filter(id => nodeById.get(id)?.kind === "data")],
    ["Traits", topLevel.filter(id => nodeById.get(id)?.kind === "trait")],
    ["Machines", topLevel.filter(id => nodeById.get(id)?.kind === "machine")],
    ["Other", topLevel.filter(id => !["data", "trait", "machine"].includes(nodeById.get(id)?.kind))]
  ];
  for (const [title, ids] of groups) {
    if (ids.length === 0) continue;
    const section = document.createElement("details");
    section.open = title !== "Data";
    const summary = document.createElement("summary");
    summary.textContent = `${title} (${ids.length})`;
    section.appendChild(summary);
    for (const id of ids) section.appendChild(outlineNode(id, 0));
    outline.appendChild(section);
  }
}

function renderFlatOutline() {
  const groups = new Map();
  for (const node of GRAPH.nodes) {
    const group = outlineLabel(node).includes("::")
      ? outlineLabel(node).split("::")[0]
      : node.kind;
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group).push(node.id);
  }

  for (const [title, ids] of groups) {
    const section = document.createElement("details");
    section.open = true;
    const summary = document.createElement("summary");
    summary.textContent = `${title} (${ids.length})`;
    section.appendChild(summary);
    for (const id of ids) section.appendChild(outlineNode(id, 0));
    outline.appendChild(section);
  }
}

function outlineNode(id, depth) {
  const node = nodeById.get(id);
  const children = (containmentChildren.get(id) || [])
    .filter(childId => ["object", "state"].includes(nodeById.get(childId)?.kind));
  if (children.length === 0 || depth >= 2) {
    const button = document.createElement("button");
    button.textContent = outlineLabel(node);
    button.title = node.label;
    button.dataset.scopeId = id;
    button.addEventListener("click", event => {
      if (event.metaKey || event.ctrlKey) {
        selectNode(id, true);
      } else {
        setScope(id, true);
      }
    });
    return button;
  }
  const detailsElement = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = outlineLabel(node);
  summary.title = node.label;
  summary.addEventListener("click", event => {
    if (event.metaKey || event.ctrlKey) {
      event.preventDefault();
      selectNode(id, true);
    }
  });
  detailsElement.appendChild(summary);
  const scopeButton = document.createElement("button");
  scopeButton.textContent = `Scope ${outlineLabel(node)}`;
  scopeButton.title = node.label;
  scopeButton.dataset.scopeId = id;
  scopeButton.addEventListener("click", () => setScope(id, true));
  detailsElement.appendChild(scopeButton);
  for (const childId of children) detailsElement.appendChild(outlineNode(childId, depth + 1));
  return detailsElement;
}

function outlineLabel(node) {
  return node.label.split("\n")[0];
}

function edgePath(from, to, kind) {
  const a = positions.get(from);
  const b = positions.get(to);
  if (!a || !b) return "";
  const ax = a.x + a.width;
  const ay = a.y + a.height / 2;
  const bx = b.x;
  const by = b.y + b.height / 2;
  if (kind === "sequence") {
    const sx = a.x + a.width / 2;
    const sy = a.y + a.height;
    const tx = b.x + b.width / 2;
    return `M ${sx} ${sy} C ${sx} ${sy + 18}, ${tx} ${by - 18}, ${tx} ${b.y}`;
  }
  const mid = ax + Math.max(40, (bx - ax) / 2);
  return `M ${ax} ${ay} C ${mid} ${ay}, ${mid} ${by}, ${bx} ${by}`;
}

function drawEdge(edge) {
  const path = el("path", {
    "class": `edge ${edge.kind}`,
    "data-from": edge.from,
    "data-to": edge.to,
    "data-kind": edge.kind,
    "d": edgePath(edge.from, edge.to, edge.kind)
  });
  edgeLayer.appendChild(path);
}

function drawNode(node) {
  const box = positions.get(node.id);
  if (!box) return;
  const group = el("g", {
    "class": `node ${node.kind}`,
    "data-id": node.id,
    "data-kind": node.kind,
    "transform": `translate(${box.x} ${box.y})`
  });
  group.appendChild(el("rect", { width: box.width, height: box.height }));
  const title = el("title");
  title.textContent = node.label;
  group.appendChild(title);

  const allLines = node.label.split("\n");
  const lines = allLines.slice(0, NODE_MAX_LINES);
  if (allLines.length > NODE_MAX_LINES) lines.push(`... ${allLines.length - NODE_MAX_LINES} more lines`);
  const maxChars = Math.max(8, Math.floor((box.width - 28) / 7.2));
  lines.forEach((line, index) => {
    const text = el("text", {
      x: 14,
      y: 22 + index * 16,
      "class": index === 0 ? "" : "subtitle"
    });
    text.textContent = fitLine(line, maxChars);
    group.appendChild(text);
  });
  group.addEventListener("click", event => {
    event.stopPropagation();
    selectNode(node.id, true);
  });
  group.addEventListener("dblclick", event => {
    event.stopPropagation();
    const targetId = transitionTargetFor(node.id);
    selectNode(targetId || node.id, true);
  });
  group.addEventListener("pointerenter", () => {
    hoveredId = node.id;
    applyRelationshipHighlight();
  });
  group.addEventListener("pointerleave", () => {
    if (hoveredId === node.id) hoveredId = null;
    applyRelationshipHighlight();
  });
  nodeLayer.appendChild(group);
}

function fitLine(line, max) {
  return line.length > max ? line.slice(0, max - 1) + "..." : line;
}

function selectNode(id, center) {
  selectedId = id;
  document.querySelectorAll(".node.selected").forEach(node => node.classList.remove("selected"));
  const element = document.querySelector(`.node[data-id="${CSS.escape(id)}"]`);
  if (element) element.classList.add("selected");
  const node = nodeById.get(id);
  const targetId = transitionTargetFor(id);
  const targetText = targetId ? `\n\ntransition target: ${outlineLabel(nodeById.get(targetId))}` : "";
  details.textContent = `${node.id}\nkind: ${node.kind}\nrank: ${node.rank}${targetText}\n\n${node.label}`;
  updateFollowTarget();
  if (center) centerOn(id);
}

function setScope(id, fit) {
  scopedId = id === "root" ? null : id;
  selectedId = id;
  details.textContent = scopeDetails(id);
  updateFollowTarget();
  applyFilters();
  if (fit) fitGraph();
}

function clearGraphScope(fit) {
  scopedId = null;
  selectedId = null;
  details.textContent = "Full graph scope. Click an outline item to focus a slice.";
  updateFollowTarget();
  applyFilters();
  if (fit) fitGraph();
}

function scopeDetails(id) {
  const node = nodeById.get(id);
  const scopedNodes = scopedNodeSet(id);
  return `Scope: ${outlineLabel(node)}\n${scopedNodes.size} nodes visible including one-hop relationships\n\n${node.id}\nkind: ${node.kind}\nrank: ${node.rank}\n\n${node.label}`;
}

function scopedNodeSet(id) {
  if (!id || id === "root") return new Set(GRAPH.nodes.map(node => node.id));
  const result = new Set();
  const stack = [id];
  while (stack.length > 0) {
    const next = stack.pop();
    if (result.has(next)) continue;
    result.add(next);
    for (const childId of containmentChildren.get(next) || []) stack.push(childId);
  }
  const contained = new Set(result);
  for (const edge of GRAPH.edges) {
    if (edge.kind === "contains" || edge.kind === "sequence") continue;
    if (contained.has(edge.from)) result.add(edge.to);
    if (contained.has(edge.to)) result.add(edge.from);
  }
  return result;
}

function applyFilters() {
  const query = search.value.trim().toLowerCase();
  const showDataNodes = showData.checked;
  const scopedNodes = scopedNodeSet(scopedId);
  const matched = new Set();
  visibleNodeIds = new Set();
  for (const node of GRAPH.nodes) {
    const inScope = scopedNodes.has(node.id);
    const visible = inScope && (showDataNodes || node.kind !== "data");
    const isMatch = !query || node.id.toLowerCase().includes(query) || node.label.toLowerCase().includes(query);
    const element = document.querySelector(`.node[data-id="${CSS.escape(node.id)}"]`);
    if (!element) continue;
    element.classList.toggle("hidden", !visible);
    element.classList.toggle("dim", query && !isMatch);
    element.classList.toggle("match", query && isMatch);
    element.classList.toggle("selected", node.id === selectedId);
    if (visible) visibleNodeIds.add(node.id);
    if (isMatch && visible) matched.add(node.id);
  }
  for (const edgeElement of document.querySelectorAll(".edge")) {
    const kind = edgeElement.dataset.kind;
    const visibleKind = kind !== "sequence" || showSequence.checked;
    const fromVisible = visibleNodeIds.has(edgeElement.dataset.from);
    const toVisible = visibleNodeIds.has(edgeElement.dataset.to);
    const visibleSearch = !query || matched.has(edgeElement.dataset.from) || matched.has(edgeElement.dataset.to);
    edgeElement.classList.toggle("hidden", !visibleKind || !fromVisible || !toVisible);
    edgeElement.classList.toggle("dim", query && !visibleSearch);
  }
  outline.querySelectorAll("button[data-scope-id]").forEach(button => {
    button.classList.toggle("scoped", button.dataset.scopeId === scopedId);
  });
  calculateBounds();
  const visibleEdges = Array.from(document.querySelectorAll(".edge")).filter(edge => !edge.classList.contains("hidden")).length;
  const scopeLabel = scopedId ? ` scoped to ${outlineLabel(nodeById.get(scopedId))}` : "";
  counts.textContent = `${visibleNodeIds.size}/${GRAPH.nodes.length} nodes, ${visibleEdges}/${GRAPH.edges.length} edges${scopeLabel}`;
  applyRelationshipHighlight();
}

function transitionTargetFor(id) {
  return GRAPH.edges.find(edge => edge.kind === "transition_target" && edge.from === id)?.to || null;
}

function updateFollowTarget() {
  followTarget.disabled = !transitionTargetFor(selectedId);
}

function activeFocusId() {
  return hoveredId || selectedId;
}

function applyRelationshipHighlight() {
  const focusId = activeFocusId();
  document.querySelectorAll(".node").forEach(node => {
    node.classList.remove("hovered", "related", "unrelated");
  });
  document.querySelectorAll(".edge").forEach(edge => {
    edge.classList.remove("related", "unrelated");
  });
  if (!focusId) return;

  const relatedNodeIds = new Set([focusId]);
  const relatedEdgeElements = [];
  for (const edge of document.querySelectorAll(".edge")) {
    if (edge.classList.contains("hidden")) continue;
    const related = edge.dataset.from === focusId || edge.dataset.to === focusId;
    if (!related) continue;
    relatedEdgeElements.push(edge);
    relatedNodeIds.add(edge.dataset.from);
    relatedNodeIds.add(edge.dataset.to);
  }

  for (const node of document.querySelectorAll(".node")) {
    if (node.classList.contains("hidden")) continue;
    const id = node.dataset.id;
    node.classList.toggle("hovered", id === focusId);
    node.classList.toggle("related", relatedNodeIds.has(id) && id !== focusId);
    node.classList.toggle("unrelated", !relatedNodeIds.has(id));
  }
  for (const edge of document.querySelectorAll(".edge")) {
    if (edge.classList.contains("hidden")) continue;
    const related = relatedEdgeElements.includes(edge);
    edge.classList.toggle("related", related);
    edge.classList.toggle("unrelated", !related);
    if (related) edgeLayer.appendChild(edge);
  }
  for (const id of relatedNodeIds) {
    const node = document.querySelector(`.node[data-id="${CSS.escape(id)}"]`);
    if (node) nodeLayer.appendChild(node);
  }
}

function setTransform(next) {
  transform = next;
  viewport.setAttribute("transform", `translate(${transform.x} ${transform.y}) scale(${transform.scale})`);
}

function fitGraph() {
  const rect = svg.getBoundingClientRect();
  const scale = Math.min(rect.width / graphBounds.width, rect.height / graphBounds.height) * 0.92;
  setTransform({
    scale,
    x: (rect.width - graphBounds.width * scale) / 2 - graphBounds.x * scale,
    y: (rect.height - graphBounds.height * scale) / 2 - graphBounds.y * scale
  });
}

function centerOn(id) {
  const box = positions.get(id);
  if (!box) return;
  const rect = svg.getBoundingClientRect();
  setTransform({
    ...transform,
    x: rect.width / 2 - (box.x + box.width / 2) * transform.scale,
    y: rect.height / 2 - (box.y + box.height / 2) * transform.scale
  });
}

let drag = null;
svg.addEventListener("pointerdown", event => {
  if (event.target.closest(".node")) return;
  drag = { x: event.clientX, y: event.clientY, tx: transform.x, ty: transform.y };
  svg.setPointerCapture(event.pointerId);
  svg.classList.add("dragging");
});
svg.addEventListener("pointermove", event => {
  if (!drag) return;
  setTransform({
    ...transform,
    x: drag.tx + event.clientX - drag.x,
    y: drag.ty + event.clientY - drag.y
  });
});
svg.addEventListener("pointerup", () => {
  drag = null;
  svg.classList.remove("dragging");
});
svg.addEventListener("wheel", event => {
  event.preventDefault();
  const rect = svg.getBoundingClientRect();
  const mx = event.clientX - rect.left;
  const my = event.clientY - rect.top;
  const factor = Math.exp(-event.deltaY * 0.003);
  const scale = Math.min(8, Math.max(0.02, transform.scale * factor));
  const gx = (mx - transform.x) / transform.scale;
  const gy = (my - transform.y) / transform.scale;
  setTransform({ scale, x: mx - gx * scale, y: my - gy * scale });
}, { passive: false });

svg.addEventListener("click", () => {
  selectedId = null;
  document.querySelectorAll(".node.selected").forEach(node => node.classList.remove("selected"));
  updateFollowTarget();
  applyRelationshipHighlight();
});
search.addEventListener("input", applyFilters);
showSequence.addEventListener("change", applyFilters);
showData.addEventListener("change", applyFilters);
document.getElementById("fit").addEventListener("click", fitGraph);
document.getElementById("reset").addEventListener("click", () => setTransform({ x: 0, y: 0, scale: 1 }));
clearScope.addEventListener("click", () => clearGraphScope(true));
followTarget.addEventListener("click", () => {
  const targetId = transitionTargetFor(selectedId);
  if (targetId) selectNode(targetId, true);
});
window.addEventListener("resize", fitGraph);

render();
fitGraph();
"#;
