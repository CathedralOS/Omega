#[derive(Clone, Debug)]
struct PhaseDiagramNode {
    id: String,
    label: String,
    scoped_label: Option<String>,
    details: Option<String>,
    kind: String,
    rank: usize,
    scope_target: Option<String>,
    service_reaches: Vec<String>,
}

#[derive(Clone, Debug)]
struct PhaseDiagramEdge {
    from: String,
    to: String,
    kind: String,
}

pub struct PipelineEmbeddedPage<'a> {
    pub number: &'a str,
    pub label: &'a str,
    pub id: &'a str,
    pub html: &'a str,
}

pub struct PhaseDiagramBuilder {
    title: String,
    nodes: Vec<PhaseDiagramNode>,
    edges: Vec<PhaseDiagramEdge>,
}

const PIPELINE_PAGES: &[(&str, &str, &str)] = &[
    ("00", "Timings", "timings"),
    ("02", "Syntax", "syntax"),
    ("03", "Symbols", "symbols"),
    ("04", "Typed", "typed"),
    ("05", "Checked", "checked"),
    ("cap", "Capabilities", "capabilities"),
    ("06", "State Graph", "state-graph"),
    ("07", "Control Flow", "control-flow"),
    ("08", "Abstract Operations", "abstract-operations"),
    ("09", "Target Operations", "target-operations"),
    (
        "10",
        "Assigned Target Operations",
        "assigned-target-operations",
    ),
    ("11", "Machine Instructions", "machine-instructions"),
    ("12", "Emission", "emission"),
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
            scoped_label: None,
            details: None,
            kind: kind.into(),
            rank,
            scope_target: None,
            service_reaches: Vec::new(),
        });
        id
    }

    pub fn scoped_node(
        &mut self,
        id: impl AsRef<str>,
        label: impl Into<String>,
        kind: impl Into<String>,
        rank: usize,
        scope_target: impl Into<String>,
    ) -> String {
        let id = sanitize_id(id.as_ref());
        self.nodes.push(PhaseDiagramNode {
            id: id.clone(),
            label: label.into(),
            scoped_label: None,
            details: None,
            kind: kind.into(),
            rank,
            scope_target: Some(scope_target.into()),
            service_reaches: Vec::new(),
        });
        id
    }

    pub fn node_service_reaches(
        &mut self,
        id: &str,
        services: impl IntoIterator<Item = impl Into<String>>,
    ) {
        let Some(node) = self.nodes.iter_mut().find(|node| node.id == id) else {
            return;
        };

        node.service_reaches = services.into_iter().map(Into::into).collect();
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
    html.push_str("<div class=\"buttons\"><button id=\"fit\">Fit</button><button id=\"reset\">Reset</button><button id=\"clear-scope\">Clear Scope</button></div>\n");
    html.push_str(
        "<label><input id=\"show-sequence\" type=\"checkbox\" checked> Statement flow</label>\n",
    );
    html.push_str(
        "<label><input id=\"show-data\" type=\"checkbox\" checked> Data definitions</label>\n",
    );
    html.push_str("<div id=\"service-reach-filter\" class=\"hidden\"><h2>Service reach</h2><div id=\"service-reach-buttons\"></div></div>\n");
    html.push_str("<p id=\"counts\"></p>\n");
    html.push_str("<div id=\"details-actions\" class=\"hidden\"></div>\n");
    html.push_str("<pre id=\"details\">Click a node for details.</pre>\n");
    html.push_str("</aside>\n");
    html.push_str("<main><svg id=\"canvas\" role=\"img\" aria-label=\"Phase graph\"><g id=\"viewport\"><g id=\"edges\"></g><g id=\"nodes\"></g></g></svg></main>\n");
    html.push_str("<aside id=\"scope-panel\">\n<h2 id=\"scope-title\">Scopes</h2>\n<input id=\"scope-search\" type=\"search\" placeholder=\"Filter scopes...\" autocomplete=\"off\">\n<nav id=\"scope-outline\" aria-label=\"Primary scopes\"></nav>\n</aside>\n");
    html.push_str("<script>\nconst GRAPH = ");
    push_graph_json(&mut html, title, nodes, edges);
    html.push_str(";\n");
    html.push_str("const SERVICE_REACH_NAMES = ");
    push_service_reach_names_json(&mut html, nodes);
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
    for (number, label, id) in PIPELINE_PAGES {
        html.push_str("<a class=\"card\" target=\"_top\" href=\"00_pipeline.html#");
        html.push_str(&escape_html(id));
        html.push_str("\"><span>");
        html.push_str(&escape_html(number));
        html.push_str("</span><strong>");
        html.push_str(&escape_html(label));
        html.push_str("</strong><small>");
        html.push('#');
        html.push_str(&escape_html(id));
        html.push_str("</small></a>\n");
    }
    html.push_str("</main>\n</body>\n</html>\n");
    html
}

pub fn pipeline_shell_html(pages: &[PipelineEmbeddedPage<'_>]) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Pipeline Visualizations</title>\n<style>\n");
    html.push_str(SHELL_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<main><iframe id=\"stage\" title=\"Pipeline stage\"></iframe></main>\n<script>\nconst PAGES = [");
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            html.push(',');
        }
        html.push_str("{\"number\":");
        push_json_string(&mut html, page.number);
        html.push_str(",\"label\":");
        push_json_string(&mut html, page.label);
        html.push_str(",\"id\":");
        push_json_string(&mut html, page.id);
        html.push_str(",\"html\":");
        push_json_string(&mut html, page.html);
        html.push('}');
    }
    html.push_str("];\n");
    html.push_str(SHELL_SCRIPT);
    html.push_str("\n</script>\n</body>\n</html>\n");
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
    html.push_str("<nav class=\"phase-nav\" aria-label=\"Pipeline stages\"><a target=\"_top\" href=\"00_pipeline.html\">Index</a>");
    for (number, label, id) in PIPELINE_PAGES {
        html.push_str("<a target=\"_top\" href=\"00_pipeline.html#");
        html.push_str(&escape_html(id));
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
        if let Some(scoped_label) = &node.scoped_label {
            output.push_str(",\"scopedLabel\":");
            push_json_string(output, scoped_label);
        }
        if let Some(details) = &node.details {
            output.push_str(",\"details\":");
            push_json_string(output, details);
        }
        output.push_str(",\"kind\":");
        push_json_string(output, &node.kind);
        output.push_str(",\"rank\":");
        output.push_str(&node.rank.to_string());
        if !node.service_reaches.is_empty() {
            output.push_str(",\"serviceReaches\":[");
            for (service_index, service) in node.service_reaches.iter().enumerate() {
                if service_index > 0 {
                    output.push(',');
                }
                push_json_string(output, service);
            }
            output.push(']');
        }
        if let Some(scope_target) = &node.scope_target {
            output.push_str(",\"scopeTarget\":");
            push_json_string(output, scope_target);
        }
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

fn push_service_reach_names_json(output: &mut String, nodes: &[PhaseDiagramNode]) {
    let mut names = nodes
        .iter()
        .flat_map(|node| node.service_reaches.iter().map(String::as_str))
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();

    output.push('[');
    for (index, name) in names.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(output, name);
    }
    output.push(']');
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '<' => output.push_str("\\u003c"),
            '>' => output.push_str("\\u003e"),
            '&' => output.push_str("\\u0026"),
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
body.has-scopes {
  grid-template-columns: minmax(280px, 22vw) 1fr minmax(280px, 24vw);
}
#panel,
#scope-panel {
  border-right: 1px solid var(--panel-border);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  padding: 18px;
  overflow: auto;
  box-shadow: 12px 0 40px rgba(0, 0, 0, 0.25);
  z-index: 2;
}
#scope-panel {
  border-left: 1px solid var(--panel-border);
  border-right: 0;
  box-shadow: -12px 0 40px rgba(0, 0, 0, 0.22);
}
h1 { margin: 0 0 16px; font-size: 18px; letter-spacing: 0.04em; }
h2 {
  color: var(--muted);
  font-size: 13px;
  letter-spacing: 0.08em;
  margin: 0 0 12px;
  text-transform: uppercase;
}
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
#scope-outline {
  border: 1px solid #283343;
  border-radius: 12px;
  background: #0d1117;
  margin: 12px 0;
  max-height: 34vh;
  overflow: auto;
  padding: 8px;
}
#scope-outline {
  max-height: calc(100vh - 150px);
}
#scope-outline button {
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
#scope-outline button:hover { background: #202a38; }
#scope-outline button.scoped { background: #263247; color: #ffffff; }
#scope-outline summary {
  color: var(--muted);
  cursor: pointer;
  margin: 3px 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
#scope-outline details { margin-left: 12px; }
#scope-outline summary {
  font-size: 12px;
  letter-spacing: 0.02em;
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
  overflow-wrap: anywhere;
  word-break: break-word;
  border: 1px solid #283343;
  border-radius: 12px;
  background: #0d1117;
  color: #d8e2ef;
  padding: 12px;
  min-height: 120px;
}
#details-actions {
  display: flex;
  gap: 8px;
  margin: 10px 0;
}
#details-actions.hidden { display: none; }
#service-reach-filter {
  border: 1px solid #283343;
  border-radius: 12px;
  background: #0d1117;
  margin: 12px 0;
  padding: 10px;
}
#service-reach-filter.hidden { display: none; }
#service-reach-filter h2 { margin-bottom: 8px; }
#service-reach-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
#service-reach-buttons button {
  border-radius: 999px;
  font-size: 11px;
  padding: 6px 8px;
}
#service-reach-buttons button.active {
  border-color: var(--match);
  color: #ffffff;
}
main { min-width: 0; min-height: 0; position: relative; }
#canvas {
  width: 100%;
  height: 100%;
  cursor: grab;
  user-select: none;
}
#canvas.dragging { cursor: grabbing; }
.edge { stroke: var(--edge); stroke-width: 1.2; fill: none; opacity: 0.45; pointer-events: none; }
.edge.sequence { stroke: var(--sequence); opacity: 0.55; stroke-dasharray: 6 5; }
.edge.field_type { stroke: #75b57c; opacity: 0.75; stroke-dasharray: 2 4; }
.edge.owned_data { stroke: #ffd166; opacity: 0.8; }
.edge.contained_object { stroke: #ff9f7f; opacity: 0.72; }
.edge.implements_data { stroke: #b089f0; opacity: 0.82; stroke-dasharray: 10 5; }
.edge.satisfies_trait { stroke: #4dd4c6; opacity: 0.82; stroke-dasharray: 8 3; }
.edge.requires_trait { stroke: #ffcf5c; opacity: 0.82; stroke-dasharray: 3 3; }
.edge.call { stroke: #ff9f7f; opacity: 0.9; stroke-width: 1.8; stroke-dasharray: 8 2 2 2; }
.edge.transition_target { stroke: #4dd4c6; opacity: 0.85; stroke-width: 1.8; stroke-dasharray: 12 4 2 4; }
.edge.transition_continuation { stroke: #8ab4ff; opacity: 0.78; stroke-width: 1.6; stroke-dasharray: 5 4; }
.edge.transition_target_loopback { stroke: #73f7b8; opacity: 0.95; stroke-width: 2.2; stroke-dasharray: 3 3; }
.edge.transition_continuation_loopback { stroke: #a2c7ff; opacity: 0.9; stroke-width: 2; stroke-dasharray: 2 6; }
.node rect {
  fill: #151c26;
  stroke: #405168;
  stroke-width: 1;
  rx: 10;
}
.node { cursor: pointer; pointer-events: all; }
.node.root rect { fill: #263247; stroke: #8ab4ff; }
.node.data rect { fill: #1b2a22; stroke: #75b57c; }
.node.domain rect { fill: #272517; stroke: #d8b65c; }
.node.trait rect { fill: #162b33; stroke: #4dd4c6; }
.node.machine rect { fill: #272132; stroke: #b089f0; }
.node.object rect { fill: #2e291b; stroke: #ffd166; }
.node.external_call rect { fill: #302816; stroke: #ffcf5c; stroke-dasharray: 7 4; }
.node.machine_ref rect { fill: #302816; stroke: #ffcf5c; stroke-dasharray: 7 4; }
.node.state rect { fill: #1f2b3d; stroke: #70a5d8; }
.node.state_block rect { fill: #142637; stroke: #6fbce6; }
.node.statement rect { fill: #241e1c; stroke: #db8f61; }
.node.scoped_block rect { fill: #1c1715; stroke: #c47b52; stroke-dasharray: 6 3; }
.node.external rect {
  stroke: #ffcf5c;
  stroke-dasharray: 7 4;
}
.node text { fill: var(--text); font-size: 12px; pointer-events: none; }
.node .subtitle { fill: var(--muted); font-size: 11px; }
.node text tspan.token-number { fill: #f0c674; }
.node text tspan.token-symbol { fill: #8fc7ff; }
.node text tspan.token-borrow { fill: #79dfb4; font-weight: 600; }
.node text tspan.token-keyword { fill: #c7d2e3; }
.node text tspan.token-muted { fill: #91a3bb; }
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
.edge.related.transition_target_loopback,
.edge.related.transition_continuation_loopback {
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

const SHELL_STYLE: &str = r#"
:root {
  --bg: #101318;
}
* { box-sizing: border-box; }
html, body { height: 100%; margin: 0; overflow: hidden; }
body {
  background: radial-gradient(circle at 20% 0%, #253144 0, #101318 42%);
}
main { min-width: 0; min-height: 0; }
iframe {
  border: 0;
  display: block;
  height: 100vh;
  width: 100%;
}
"#;

const SHELL_SCRIPT: &str = r##"
const frame = document.getElementById("stage");
const pagesById = new Map(PAGES.map(page => [page.id, page]));

function selectedId() {
  const hash = window.location.hash.slice(1);
  return pagesById.has(hash) ? hash : PAGES[0]?.id;
}

function renderSelectedPage() {
  const id = selectedId();
  const page = pagesById.get(id);
  if (!page) return;
  frame.srcdoc = page.html;
}

window.addEventListener("hashchange", renderSelectedPage);
if (!window.location.hash && PAGES.length > 0) {
  history.replaceState(null, "", `#${PAGES[0].id}`);
}
renderSelectedPage();
"##;

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
const detailsActions = document.getElementById("details-actions");
const counts = document.getElementById("counts");
const serviceReachFilter = document.getElementById("service-reach-filter");
const serviceReachButtons = document.getElementById("service-reach-buttons");
const scopePanel = document.getElementById("scope-panel");
const scopeTitle = document.getElementById("scope-title");
const scopeSearch = document.getElementById("scope-search");
const scopeOutline = document.getElementById("scope-outline");
const showSequence = document.getElementById("show-sequence");
const showData = document.getElementById("show-data");
const clearScope = document.getElementById("clear-scope");
const NS = "http://www.w3.org/2000/svg";

const nodeById = new Map(GRAPH.nodes.map(node => [node.id, node]));
const containmentChildren = new Map();
const containmentParent = new Map();
for (const edge of GRAPH.edges) {
  if (edge.kind === "contains") {
    if (!containmentChildren.has(edge.from)) containmentChildren.set(edge.from, []);
    containmentChildren.get(edge.from).push(edge.to);
    containmentParent.set(edge.to, edge.from);
  }
}

const NODE_W = 250;
const NODE_H = 76;
const NODE_MAX_W = 560;
const NODE_MAX_LINES = 14;
const LINE_H = 16;
const RANK_GAP = 70;
const ROW_GAP = 18;
const TREE_LEVEL_GAP = 96;
const TREE_SIBLING_GAP = 34;
const LEFT = 80;
const TOP = 60;
const SECTION_GAP_Y = 70;
const OWNER_COLUMNS = 2;
const DATA_COLUMNS = 4;
const nodeBoxes = new Map();
const positions = new Map();
let graphBounds = { x: 0, y: 0, width: 1000, height: 800 };
let selectedId = null;
let hoveredId = null;
let scopedId = null;
let visibleNodeIds = new Set(GRAPH.nodes.map(node => node.id));
let transform = { x: 0, y: 0, scale: 1 };
let lastActivation = { id: null, time: 0 };
let activeServiceReach = null;

function displayLabel(node) {
  if (node.scopedLabel && scopedId && containedNodeSet(scopedId).has(node.id)) {
    return node.scopedLabel;
  }
  return node.label;
}

function isExpandedScopedLabel(node) {
  return Boolean(node.scopedLabel && scopedId && containedNodeSet(scopedId).has(node.id));
}

function layoutGraph(allowedIds = null) {
  positions.clear();
  const allowed = allowedIds && allowedIds.size > 0
    ? new Set(Array.from(allowedIds).filter(id => nodeById.has(id)))
    : new Set(GRAPH.nodes.map(node => node.id));
  if (allowed.size === 0) return fallbackLayout(allowed);

  const roots = Array.from(allowed)
    .filter(id => !hasContainmentParentIn(id, allowed))
    .sort((a, b) => (nodeById.get(a)?.rank || 0) - (nodeById.get(b)?.rank || 0));
  const hasTreeShape = roots.some(id => containmentChildrenFor(id, allowed).length > 0);
  if (!hasTreeShape) return fallbackLayout(allowed);

  if (roots.length === 1) {
    const rootId = roots[0];
    layoutSubtree(rootId, LEFT, TOP, new Set(), allowed);
    return;
  }

  let cursorY = TOP;
  for (const rootId of roots) {
    layoutSubtree(rootId, LEFT, cursorY, new Set(), allowed);
    cursorY += layoutSubtreeHeight(rootId, new Set(), allowed) + SECTION_GAP_Y;
  }
}

function containmentChildrenFor(id, allowed) {
  return (containmentChildren.get(id) || [])
    .filter(childId => allowed.has(childId) && nodeById.has(childId));
}

function hasContainmentParentIn(id, allowed) {
  for (const [parent, children] of containmentChildren.entries()) {
    if (!allowed.has(parent)) continue;
    if (children.some(childId => childId === id && allowed.has(childId))) return true;
  }
  return false;
}

function usesContainedGraphLayout(id, children) {
  if (children.length === 0) return false;
  const node = nodeById.get(id);
  if (!node || node.kind !== "state_block") return false;
  return children.every(childId => nodeById.get(childId)?.kind === "scoped_block");
}

function graphChildrenLayout(children, allowed) {
  const childSet = new Set(children.filter(childId => allowed.has(childId) && nodeById.has(childId)));
  if (childSet.size === 0) {
    return { positions: new Map(), width: 0, height: 0 };
  }

  const outgoing = new Map(Array.from(childSet, id => [id, []]));
  const incomingCount = new Map(Array.from(childSet, id => [id, 0]));
  for (const edge of GRAPH.edges) {
    if (!childSet.has(edge.from) || !childSet.has(edge.to)) continue;
    if (edge.kind === "contains") continue;
    outgoing.get(edge.from).push(edge.to);
    incomingCount.set(edge.to, (incomingCount.get(edge.to) || 0) + 1);
  }

  const ranks = new Map();
  const roots = Array.from(childSet).filter(id => (incomingCount.get(id) || 0) === 0);
  const pendingRoots = roots.length > 0 ? roots : Array.from(childSet).slice(0, 1);

  for (const root of pendingRoots) assignFallbackRanks(root, 0, ranks, outgoing);
  for (const id of childSet) {
    if (!ranks.has(id)) assignFallbackRanks(id, 0, ranks, outgoing);
  }

  const ids = Array.from(childSet).sort((a, b) => {
    const rankDiff = (ranks.get(a) || 0) - (ranks.get(b) || 0);
    if (rankDiff !== 0) return rankDiff;
    return a.localeCompare(b);
  });

  const columns = new Map();
  for (const id of ids) {
    const rank = ranks.get(id) || 0;
    const column = columns.get(rank) || [];
    column.push(id);
    columns.set(rank, column);
  }

  const orderedRanks = Array.from(columns.keys()).sort((a, b) => a - b);
  const columnWidths = orderedRanks.map(rank => maxNodeWidth(columns.get(rank) || []));
  const positions = new Map();
  let cursorX = 0;
  let totalHeight = 0;

  orderedRanks.forEach((rank, rankIndex) => {
    const idsInColumn = columns.get(rank) || [];
    const columnWidth = columnWidths[rankIndex] || NODE_W;
    let cursorY = 0;
    idsInColumn.forEach((childId, childIndex) => {
      const box = nodeBox(childId);
      positions.set(childId, {
        x: cursorX + Math.max(0, (columnWidth - box.width) / 2),
        y: cursorY,
        width: box.width,
        height: box.height
      });
      cursorY += box.height + (childIndex + 1 < idsInColumn.length ? ROW_GAP : 0);
    });
    totalHeight = Math.max(totalHeight, cursorY);
    cursorX += columnWidth + (rankIndex + 1 < orderedRanks.length ? RANK_GAP : 0);
  });

  return {
    positions,
    width: Math.max(0, cursorX),
    height: totalHeight
  };
}

function layoutSubtreeWidth(id, seen, allowed) {
  if (seen.has(id)) {
    return nodeBox(id).width;
  }
  seen.add(id);

  const box = nodeBox(id);
  const children = containmentChildrenFor(id, allowed);

  if (children.length === 0) {
    seen.delete(id);
    return box.width;
  }

  if (usesContainedGraphLayout(id, children)) {
    const graph = graphChildrenLayout(children, allowed);
    seen.delete(id);
    return Math.max(box.width, graph.width);
  }

  const childrenWidth = children
    .map(childId => layoutSubtreeWidth(childId, seen, allowed))
    .reduce((sum, width) => sum + width, 0)
    + TREE_SIBLING_GAP * (children.length - 1);
  seen.delete(id);
  return Math.max(box.width, childrenWidth);
}

function layoutSubtreeHeight(id, seen, allowed) {
  if (seen.has(id)) {
    return nodeBox(id).height;
  }
  seen.add(id);

  const box = nodeBox(id);
  const children = containmentChildrenFor(id, allowed);
  if (children.length === 0) {
    seen.delete(id);
    return box.height;
  }

  if (usesContainedGraphLayout(id, children)) {
    const graph = graphChildrenLayout(children, allowed);
    seen.delete(id);
    return box.height + TREE_LEVEL_GAP + graph.height;
  }

  const childHeight = Math.max(...children.map(childId => layoutSubtreeHeight(childId, seen, allowed)));
  seen.delete(id);
  return box.height + TREE_LEVEL_GAP + childHeight;
}

function layoutSubtree(id, x, y, seen, allowed) {
  if (seen.has(id)) {
    placeNode(id, x, y);
    return nodeBox(id).width;
  }
  seen.add(id);

  const box = nodeBox(id);
  const children = containmentChildrenFor(id, allowed);
  const subtreeWidth = layoutSubtreeWidth(id, new Set(), allowed);
  placeNode(id, x + Math.max(0, (subtreeWidth - box.width) / 2), y);

  if (children.length === 0) {
    seen.delete(id);
    return subtreeWidth;
  }

  if (usesContainedGraphLayout(id, children)) {
    const graph = graphChildrenLayout(children, allowed);
    const childY = y + box.height + TREE_LEVEL_GAP;
    const graphX = x + Math.max(0, (subtreeWidth - graph.width) / 2);
    for (const childId of children) {
      const relative = graph.positions.get(childId);
      if (!relative) continue;
      placeNode(childId, graphX + relative.x, childY + relative.y);
    }
    seen.delete(id);
    return subtreeWidth;
  }

  let cursorX = x;
  const childY = y + box.height + TREE_LEVEL_GAP;
  for (const childId of children) {
    const childWidth = layoutSubtreeWidth(childId, seen, allowed);
    layoutSubtree(childId, cursorX, childY, seen, allowed);
    cursorX += childWidth + TREE_SIBLING_GAP;
  }

  seen.delete(id);
  return subtreeWidth;
}

function fallbackLayout(allowedIds = null) {
  const allowed = allowedIds && allowedIds.size > 0
    ? allowedIds
    : new Set(GRAPH.nodes.map(node => node.id));
  const ids = Array.from(allowed).filter(id => nodeById.has(id));
  const outgoing = new Map(ids.map(id => [id, []]));
  const incomingCount = new Map(ids.map(id => [id, 0]));
  for (const edge of GRAPH.edges) {
    if (!allowed.has(edge.from) || !allowed.has(edge.to)) continue;
    if (!outgoing.has(edge.from) || !incomingCount.has(edge.to)) continue;
    outgoing.get(edge.from).push(edge.to);
    incomingCount.set(edge.to, incomingCount.get(edge.to) + 1);
  }

  const ranks = new Map();
  const roots = ids.filter(id => (incomingCount.get(id) || 0) === 0);
  const pendingRoots = roots.length > 0 ? roots : ids.slice(0, 1);

  for (const root of pendingRoots) assignFallbackRanks(root, 0, ranks, outgoing);
  for (const id of ids) {
    const node = nodeById.get(id);
    if (!ranks.has(id)) assignFallbackRanks(id, node.rank, ranks, outgoing);
  }

  const rowsByRank = new Map();
  const rowHeightsByRank = new Map();
  ids.forEach(id => {
    const node = nodeById.get(id);
    const rank = ranks.get(node.id) || 0;
    const row = rowsByRank.get(rank) || 0;
    rowsByRank.set(rank, row + 1);
    const rankHeights = rowHeightsByRank.get(rank) || [];
    rankHeights[row] = Math.max(rankHeights[row] || 0, nodeBox(node.id).height);
    rowHeightsByRank.set(rank, rankHeights);
  });

  ids.forEach(id => {
    const node = nodeById.get(id);
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
  const lines = displayLabel(node).split("\n");
  const measuredLines = isExpandedScopedLabel(node) ? lines : lines.slice(0, NODE_MAX_LINES);
  const shownLines = measuredLines.length;
  const maxChars = measuredLines.reduce((max, line) => Math.max(max, line.length), 0);
  return {
    width: Math.min(NODE_MAX_W, Math.max(NODE_W, 28 + maxChars * 7.2)),
    height: Math.max(
      NODE_H,
      30 + shownLines * LINE_H + (!isExpandedScopedLabel(node) && lines.length > NODE_MAX_LINES ? LINE_H : 0)
    )
  };
}

function refreshNodePresentation() {
  for (const node of GRAPH.nodes) {
    const box = measureNode(node);
    nodeBoxes.set(node.id, box);
    const element = document.querySelector(`.node[data-id="${CSS.escape(node.id)}"]`);
    if (!element) continue;
    const rect = element.querySelector("rect");
    if (rect) {
      rect.setAttribute("width", box.width);
      rect.setAttribute("height", box.height);
    }
    const title = element.querySelector("title");
    const label = displayLabel(node);
    if (title) title.textContent = label;
    element.querySelectorAll("text").forEach(text => text.remove());
    populateNodeText(element, node, box, label);
  }
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

refreshNodePresentation();
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
  renderServiceReachFilters();
  renderScopeOutline();
  calculateBounds();
  applyFilters();
}

function renderServiceReachFilters() {
  serviceReachButtons.replaceChildren();
  const presentServices = SERVICE_REACH_NAMES.filter(service =>
    GRAPH.nodes.some(node => (node.serviceReaches || []).includes(service))
  );
  serviceReachFilter.classList.toggle("hidden", presentServices.length === 0);
  if (presentServices.length === 0) return;

  const allButton = document.createElement("button");
  allButton.textContent = "all";
  allButton.classList.toggle("active", !activeServiceReach);
  allButton.addEventListener("click", () => {
    activeServiceReach = null;
    renderServiceReachFilters();
    applyFilters();
  });
  serviceReachButtons.appendChild(allButton);

  for (const service of presentServices) {
    const button = document.createElement("button");
    button.textContent = service;
    button.classList.toggle("active", activeServiceReach === service);
    button.addEventListener("click", () => {
      activeServiceReach = activeServiceReach === service ? null : service;
      renderServiceReachFilters();
      applyFilters();
    });
    serviceReachButtons.appendChild(button);
  }
}

function renderScopeOutline() {
  scopeOutline.replaceChildren();
  const primary = primaryScopes();
  scopeTitle.textContent = primary.title;
  scopeSearch.placeholder = primary.placeholder;
  document.body.classList.toggle("has-scopes", primary.ids.length > 0);
  scopePanel.classList.toggle("hidden", primary.ids.length === 0);
  if (primary.ids.length === 0) return;

  const query = scopeSearch.value.trim().toLowerCase();
  const visibleIds = primary.ids.filter(id => {
    const path = scopePath(id);
    return !query || path.join("/").toLowerCase().includes(query);
  });
  if (visibleIds.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "No matching scopes.";
    scopeOutline.appendChild(empty);
    return;
  }

  const tree = buildScopeTree(visibleIds);
  appendScopeTree(scopeOutline, tree, 0, Boolean(query));
}

function primaryScopes() {
  const fileIds = GRAPH.nodes.filter(node => node.kind === "file").map(node => node.id);
  if (fileIds.length > 0) {
    return { title: "Files", placeholder: "Filter files...", ids: fileIds };
  }

  if (nodeById.has("root")) {
    const topLevel = (containmentChildren.get("root") || [])
      .filter(id => nodeById.has(id) && nodeById.get(id).kind !== "root");
    if (topLevel.length > 0) {
      return { title: "Scopes", placeholder: "Filter scopes...", ids: topLevel };
    }
  }

  const fragments = GRAPH.nodes
    .filter(node => ["data", "trait", "machine", "file"].includes(node.kind))
    .map(node => node.id);
  if (fragments.length > 0) {
    return { title: "Scopes", placeholder: "Filter scopes...", ids: fragments };
  }

  const stateBlocks = GRAPH.nodes
    .filter(node => node.kind === "state_block")
    .map(node => node.id);
  if (stateBlocks.length > 0) {
    return { title: "Blocks", placeholder: "Filter blocks...", ids: stateBlocks };
  }

  const fallback = GRAPH.nodes
    .filter(node => node.id !== "root")
    .map(node => node.id);
  return { title: "Scopes", placeholder: "Filter scopes...", ids: fallback };
}

function buildScopeTree(ids) {
  const root = { name: "", dirs: new Map(), files: [] };
  for (const id of ids) {
    const parts = scopePath(id);
    const name = parts.pop() || outlineLabel(nodeById.get(id));
    let branch = root;
    for (const part of parts) {
      if (!branch.dirs.has(part)) {
        branch.dirs.set(part, { name: part, dirs: new Map(), files: [] });
      }
      branch = branch.dirs.get(part);
    }
    branch.files.push({ id, name, path: scopePath(id).join("/") });
  }
  return root;
}

function appendScopeTree(parent, tree, depth, forceOpen) {
  const dirs = Array.from(tree.dirs.values()).sort((a, b) => a.name.localeCompare(b.name));
  for (const dir of dirs) {
    const detailsElement = document.createElement("details");
    detailsElement.open = forceOpen || depth < 1;
    const summary = document.createElement("summary");
    summary.textContent = dir.name;
    detailsElement.appendChild(summary);
    appendScopeTree(detailsElement, dir, depth + 1, forceOpen);
    parent.appendChild(detailsElement);
  }

  const entries = tree.files.sort((a, b) => a.name.localeCompare(b.name));
  for (const entry of entries) {
    const button = scopeButton(entry.id, nodeById.get(entry.id));
    button.textContent = entry.name;
    button.title = entry.path;
    parent.appendChild(button);
  }
}

function scopeButton(id, node) {
  const button = document.createElement("button");
  button.textContent = scopePath(id).slice(-1)[0] || outlineLabel(node);
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

function outlineLabel(node) {
  return node.label.split("\n")[0];
}

function fileLabel(node) {
  const firstLine = outlineLabel(node);
  return firstLine.startsWith("file ") ? firstLine.slice(5) : firstLine;
}

function scopePath(id) {
  const node = nodeById.get(id);
  if (!node) return [id];
  if (node.kind === "file") {
    return fileLabel(node).split(/[\\/]+/).filter(Boolean);
  }

  const label = outlineLabel(node);
  if (node.kind === "state_block" && label.includes("::")) {
    const [owner, rest] = label.split("::", 2);
    return [owner, rest];
  }

  const stripped = label.replace(/^(boundary trait|trait|data|machine|state)\s+/, "");
  return [scopeKindLabel(node.kind), stripped];
}

function scopeKindLabel(kind) {
  switch (kind) {
    case "data": return "Data";
    case "trait": return "Traits";
    case "machine": return "Machines";
    case "state":
    case "state_block": return "States";
    case "object": return "Objects";
    default: return "Other";
  }
}

function edgePath(from, to, kind) {
  const a = positions.get(from);
  const b = positions.get(to);
  if (!a || !b) return "";
  const ax = a.x + a.width / 2;
  const ay = a.y + a.height;
  const bx = b.x + b.width / 2;
  const by = b.y;
  if (kind === "call") {
    const targetOnRight = bx >= ax;
    const sx = targetOnRight ? a.x + a.width : a.x;
    const sy = a.y + a.height / 2;
    const tx = targetOnRight ? b.x : b.x + b.width;
    const ty = b.y + b.height / 2;
    const dx = Math.max(42, Math.abs(tx - sx) * 0.35);
    const c1x = sx + (targetOnRight ? dx : -dx);
    const c2x = tx - (targetOnRight ? dx : -dx);
    return `M ${sx} ${sy} C ${c1x} ${sy}, ${c2x} ${ty}, ${tx} ${ty}`;
  }
  if (kind === "sequence") {
    const sx = a.x + a.width / 2;
    const sy = a.y + a.height;
    const tx = b.x + b.width / 2;
    return `M ${sx} ${sy} C ${sx} ${sy + 18}, ${tx} ${by - 18}, ${tx} ${b.y}`;
  }
  const midY = ay + Math.max(34, (by - ay) / 2);
  return `M ${ax} ${ay} C ${ax} ${midY}, ${bx} ${midY}, ${bx} ${by}`;
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
  const label = displayLabel(node);
  title.textContent = label;
  group.appendChild(title);
  populateNodeText(group, node, box, label);
  group.addEventListener("pointerdown", event => {
    event.stopPropagation();
  });
  group.addEventListener("mousedown", event => {
    event.stopPropagation();
    if (event.detail >= 2) {
      activateNode(node.id);
    }
  });
  group.addEventListener("mouseup", event => {
    event.stopPropagation();
    if (event.detail <= 1) {
      selectNode(node.id, false);
    }
  });
  group.addEventListener("click", event => {
    event.stopPropagation();
    if (event.detail >= 2) {
      activateNode(node.id);
      return;
    }
    selectNode(node.id, false);
  });
  group.addEventListener("dblclick", event => {
    event.stopPropagation();
    activateNode(node.id);
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

function populateNodeText(group, node, box, label) {
  const allLines = label.split("\n");
  const lines = isExpandedScopedLabel(node)
    ? allLines
    : allLines.slice(0, NODE_MAX_LINES);
  if (!isExpandedScopedLabel(node) && allLines.length > NODE_MAX_LINES) {
    lines.push(`... ${allLines.length - NODE_MAX_LINES} more lines`);
  }
  const maxChars = Math.max(8, Math.floor((box.width - 28) / 7.2));
  lines.forEach((line, index) => {
    const text = el("text", {
      x: 14,
      y: 22 + index * 16,
      "class": index === 0 ? "" : "subtitle"
    });
    appendStyledLine(text, fitLine(line, maxChars));
    group.appendChild(text);
  });
}

function fitLine(line, max) {
  return line.length > max ? line.slice(0, max - 1) + "..." : line;
}

const LABEL_TOKEN_RE = /(active loan|borrow call|activation|weakening|borrow|contracts?|params|ops|transitions|direct service reach|reached service reach|suspension|blocking|attached data|states|contains|owned data|mutable params|created|last use|instructions|control|calls|terminator|call|ctrl|data)|(#\d+(?:\.\d+)?|\b\d+\b)|((?:self|mut)\b)|([A-Za-z_][A-Za-z0-9_]*(?:(?:::[A-Za-z_][A-Za-z0-9_]*)|(?:\.[A-Za-z_][A-Za-z0-9_]*)|(?:\[[^\]]*\]))+)/g;

function appendStyledLine(textNode, line) {
  let lastIndex = 0;
  LABEL_TOKEN_RE.lastIndex = 0;
  for (const match of line.matchAll(LABEL_TOKEN_RE)) {
    const index = match.index || 0;
    if (index > lastIndex) {
      textNode.appendChild(document.createTextNode(line.slice(lastIndex, index)));
    }
    const token = match[0];
    let className = "";
    if (match[1]) {
      className = /borrow|loan|activation|weakening/.test(token)
        ? "token-borrow"
        : "token-keyword";
    } else if (match[2]) {
      className = "token-number";
    } else if (match[3]) {
      className = "token-muted";
    } else if (match[4]) {
      className = "token-symbol";
    }
    if (className) {
      const span = el("tspan", { "class": className });
      span.textContent = token;
      textNode.appendChild(span);
    } else {
      textNode.appendChild(document.createTextNode(token));
    }
    lastIndex = index + token.length;
  }
  if (lastIndex < line.length) {
    textNode.appendChild(document.createTextNode(line.slice(lastIndex)));
  }
}

function activateNode(id) {
  const now = performance.now();
  if (lastActivation.id === id && now - lastActivation.time < 300) return;
  lastActivation = { id, time: now };
  const scopeTarget = scopeTargetFor(id);
  const kind = nodeById.get(id)?.kind;
  if (kind === "external_call" && scopeTarget) {
    if (scopedId) {
      setScope(scopeTarget, true);
    } else {
      selectNode(scopeTarget, true);
    }
    return;
  }

  if (scopeTarget && scopeTarget !== scopedId && isScopeNode(id)) {
    setScope(scopeTarget, true);
    return;
  }

  const targetId = followTargetFor(id);
  if (targetId) {
    selectNode(targetId, true);
    return;
  }

  if (scopeTarget && scopeTarget !== scopedId) {
    setScope(scopeTarget, true);
    return;
  }

  selectNode(id, true);
}

function selectNode(id, center) {
  selectedId = id;
  markSelectedNode(id);
  const node = nodeById.get(id);
  const targetId = followTargetFor(id);
  const targetText = targetId ? `\n\nfollow target: ${outlineLabel(nodeById.get(targetId))}` : "";
  const body = node.details || node.label;
  details.textContent = `${node.id}\nkind: ${node.kind}\nrank: ${node.rank}${targetText}\n\n${body}`;
  renderDetailsActions(id);
  if (center) centerOn(id);
}

function markSelectedNode(id) {
  document.querySelectorAll(".node.selected").forEach(node => node.classList.remove("selected"));
  const element = document.querySelector(`.node[data-id="${CSS.escape(id)}"]`);
  if (element) element.classList.add("selected");
}

function setScope(id, fit) {
  scopedId = id === "root" ? null : id;
  selectedId = id;
  details.textContent = scopeDetails(id);
  renderDetailsActions(id);
  applyFilters();
  markSelectedNode(id);
  if (fit) fitGraph();
}

function clearGraphScope(fit) {
  scopedId = null;
  selectedId = null;
  details.textContent = "Full graph scope. Click a scope item to focus a slice.";
  detailsActions.replaceChildren();
  detailsActions.classList.add("hidden");
  applyFilters();
  if (fit) fitGraph();
}

function renderDetailsActions(id) {
  detailsActions.replaceChildren();
  const actions = [];
  const scopeTarget = scopeTargetFor(id);
  if (scopeTarget && scopeTarget !== scopedId) {
    actions.push({ label: "Open Scope", action: () => setScope(scopeTarget, true) });
  }

  const targetId = followTargetFor(id);
  if (targetId) actions.push({ label: "Follow", action: () => selectNode(targetId, true) });

  if (scopedId) actions.push({ label: "Clear Scope", action: () => clearGraphScope(true) });

  detailsActions.classList.toggle("hidden", actions.length === 0);
  for (const action of actions) {
    const button = document.createElement("button");
    button.textContent = action.label;
    button.addEventListener("click", event => {
      event.stopPropagation();
      action.action();
    });
    detailsActions.appendChild(button);
  }
}

function defaultScopeId() {
  const primary = primaryScopes();
  if (primary.ids.length === 0) return null;
  const mainFile = primary.ids
    .map(id => nodeById.get(id))
    .find(node => node?.kind === "file" && (outlineLabel(node).endsWith("/main.omg") || outlineLabel(node).endsWith(" main.omg")));
  if (mainFile) return mainFile.id;
  return primary.ids[0];
}

function scopeDetails(id) {
  const node = nodeById.get(id);
  const scopedNodes = scopedNodeSet(id);
  return `Scope:\n  ${outlineLabel(node)}\n\n${scopedNodes.size} nodes visible including one-hop relationships\n\n${node.id}\nkind: ${node.kind}\nrank: ${node.rank}\n\n${node.label}`;
}

function containedNodeSet(id) {
  if (!id || id === "root") return new Set(GRAPH.nodes.map(node => node.id));
  const result = new Set();
  const stack = [id];
  while (stack.length > 0) {
    const next = stack.pop();
    if (result.has(next)) continue;
    result.add(next);
    for (const childId of containmentChildren.get(next) || []) stack.push(childId);
  }
  return result;
}

function scopedNodeSet(id) {
  const result = containedNodeSet(id);
  if (!id || id === "root") return result;
  const contained = new Set(result);
  for (const edge of GRAPH.edges) {
    if (edge.kind === "contains" || edge.kind === "sequence") continue;
    if (contained.has(edge.from)) result.add(edge.to);
    if (contained.has(edge.to)) result.add(edge.from);
  }
  return result;
}

function isExternalInCurrentScope(id) {
  return scopedId && visibleNodeIds.has(id) && !containedNodeSet(scopedId).has(id);
}

function scopeTargetFor(id) {
  const explicitTarget = nodeById.get(id)?.scopeTarget;
  if (explicitTarget && nodeById.has(explicitTarget)) return explicitTarget;
  let current = id;
  while (current && !isScopeNode(current)) {
    const parent = containmentParent.get(current);
    if (!parent || isGraphRoot(parent)) break;
    current = parent;
  }
  return current;
}

function isGraphRoot(id) {
  return id === "root" || nodeById.get(id)?.kind === "root";
}

function isScopeNode(id) {
  const kind = nodeById.get(id)?.kind;
  return ["file", "data", "trait", "machine", "state_block", "object"].includes(kind);
}

function updateGeometry() {
  for (const node of GRAPH.nodes) {
    const box = positions.get(node.id);
    const element = document.querySelector(`.node[data-id="${CSS.escape(node.id)}"]`);
    if (element && box) element.setAttribute("transform", `translate(${box.x} ${box.y})`);
  }
  for (const edgeElement of document.querySelectorAll(".edge")) {
    edgeElement.setAttribute(
      "d",
      edgePath(edgeElement.dataset.from, edgeElement.dataset.to, edgeElement.dataset.kind)
    );
  }
}

function applyFilters() {
  refreshNodePresentation();
  const query = search.value.trim().toLowerCase();
  const showDataNodes = showData.checked;
  const scopedNodes = scopedNodeSet(scopedId);
  const containedNodes = containedNodeSet(scopedId);
  const matched = new Set();
  visibleNodeIds = new Set();
  for (const node of GRAPH.nodes) {
    const inScope = scopedNodes.has(node.id);
    const requiresExplicitScope = node.kind === "scoped_block";
    const scopeAllowsNode = !requiresExplicitScope || (scopedId && containedNodes.has(node.id));
    const visible = inScope
      && scopeAllowsNode
      && (showDataNodes || node.kind !== "data")
      && (!activeServiceReach || (node.serviceReaches || []).includes(activeServiceReach));
    const isMatch = !query || node.id.toLowerCase().includes(query) || node.label.toLowerCase().includes(query);
    const element = document.querySelector(`.node[data-id="${CSS.escape(node.id)}"]`);
    if (!element) continue;
    element.classList.toggle("hidden", !visible);
    element.classList.toggle("dim", query && !isMatch);
    element.classList.toggle("match", query && isMatch);
    element.classList.toggle("selected", node.id === selectedId);
    element.classList.toggle("external", Boolean(scopedId) && visible && !containedNodes.has(node.id));
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
  scopeOutline.querySelectorAll("button[data-scope-id]").forEach(button => {
    button.classList.toggle("scoped", button.dataset.scopeId === scopedId);
  });
  layoutGraph(visibleNodeIds);
  updateGeometry();
  calculateBounds();
  const visibleEdges = Array.from(document.querySelectorAll(".edge")).filter(edge => !edge.classList.contains("hidden")).length;
  const scopeLabel = scopedId ? ` scoped to ${outlineLabel(nodeById.get(scopedId))}` : "";
  const serviceLabel = activeServiceReach ? ` service ${activeServiceReach}` : "";
  counts.textContent = `${visibleNodeIds.size}/${GRAPH.nodes.length} nodes, ${visibleEdges}/${GRAPH.edges.length} edges${scopeLabel}${serviceLabel}`;
  applyRelationshipHighlight();
}

function followTargetFor(id) {
  const transitionTarget = GRAPH.edges.find(
    edge => edge.kind.startsWith("transition_target") && edge.from === id
  )?.to;
  if (transitionTarget) return transitionTarget;
  return null;
}

function activeFocusId() {
  return hoveredId || selectedId;
}

function applyRelationshipHighlight() {
  const focusId = activeFocusId();
  const proxyFocusIds = focusIdsForRelationship(focusId);
  document.querySelectorAll(".node").forEach(node => {
    node.classList.remove("hovered", "related", "unrelated");
  });
  document.querySelectorAll(".edge").forEach(edge => {
    edge.classList.remove("related", "unrelated");
  });
  if (!focusId) return;

  const relatedNodeIds = new Set(proxyFocusIds);
  const relatedEdgeElements = [];
  for (const edge of document.querySelectorAll(".edge")) {
    if (edge.classList.contains("hidden")) continue;
    const related = proxyFocusIds.has(edge.dataset.from) || proxyFocusIds.has(edge.dataset.to);
    if (!related) continue;
    relatedEdgeElements.push(edge);
    relatedNodeIds.add(edge.dataset.from);
    relatedNodeIds.add(edge.dataset.to);
  }

  for (const node of document.querySelectorAll(".node")) {
    if (node.classList.contains("hidden")) continue;
    const id = node.dataset.id;
    node.classList.toggle("hovered", proxyFocusIds.has(id));
    node.classList.toggle("related", relatedNodeIds.has(id) && !proxyFocusIds.has(id));
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

function focusIdsForRelationship(id) {
  const result = new Set();
  if (!id) return result;
  result.add(id);
  const explicitTarget = nodeById.get(id)?.scopeTarget;
  if (explicitTarget && nodeById.has(explicitTarget)) {
    result.add(explicitTarget);
  }
  return result;
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
  if (isNodeEventTarget(event.target)) return;
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
  applyRelationshipHighlight();
});

function isNodeEventTarget(target) {
  return Boolean(target?.closest?.(".node"));
}
search.addEventListener("input", applyFilters);
scopeSearch.addEventListener("input", () => {
  renderScopeOutline();
  applyFilters();
});
showSequence.addEventListener("change", applyFilters);
showData.addEventListener("change", applyFilters);
document.getElementById("fit").addEventListener("click", fitGraph);
document.getElementById("reset").addEventListener("click", () => setTransform({ x: 0, y: 0, scale: 1 }));
clearScope.addEventListener("click", () => clearGraphScope(true));
window.addEventListener("resize", fitGraph);

render();
fitGraph();
"#;

#[cfg(test)]
mod tests {
    use super::PhaseDiagramBuilder;

    #[test]
    fn service_filters_are_derived_from_canonical_node_rows() {
        let mut diagram = PhaseDiagramBuilder::new("services");
        let first = diagram.node("first", "first", "machine", 1);
        let second = diagram.node("second", "second", "machine", 1);
        diagram.node_service_reaches(&first, ["PortIo", "Console"]);
        diagram.node_service_reaches(&second, ["Console"]);

        let html = diagram.finish();

        assert!(html.contains("const SERVICE_REACH_NAMES = [\"Console\",\"PortIo\"]"));
        assert!(html.contains("\"serviceReaches\":[\"PortIo\",\"Console\"]"));
        assert!(html.contains("<h2>Service reach</h2>"));
        assert!(!html.contains("EffectSet"));
        assert!(!html.contains("\"effects\":"));
        assert!(!html.contains("stdout_io"));
    }
}
