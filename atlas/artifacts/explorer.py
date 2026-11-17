"""Curated source traces and a separate, metadata-derived dependency view.

Coordinates and tour order are explanatory choices, not execution telemetry.
The Cargo view includes every normal edge among its selected packages.
"""
import html
import json
import textwrap


def models(revision, graph):
    code = f"https://github.com/CathedralOS/Omega/blob/{revision}/"
    omega = "omega-rust/omega/"
    pipeline = omega + "compilation/native-compiler/src/pipeline.rs#L1"
    settlement = omega + "compilation/compiler/src/execution_settlement.rs#L202"
    runtime = omega + "application/src/execution/compilation.rs#L96-L132"

    def node(key, label, kind, x, y, page, description, source):
        return dict(id=key, label=label, kind=kind, x=x, y=y, page=page,
                    description=description, source=code + source)

    def edge(key, start, end, label, path, x, y, kind="handoff"):
        return dict(id=key, start=start, end=end, label=label, path=path,
                    x=x, y=y, kind=kind)

    def step(title, text, selected, *edges):
        return dict(title=title, text=text, node=selected, edges=list(edges))

    common = [
        node("build", "Build & packages", "around", 20, 80, "build-and-packages", "Build admits package inputs and returns source choices, a target and provider bindings. The CLI requests the product; package custody remains part of the request.", omega + "build-system/orchestration/src/compile/native_completion.rs#L73-L182"),
        node("source", "Psi source stages", "psi", 270, 80, "psi", "Source preparation crosses Psi 00–06 and publishes accepted code. The source workflow can pause for evaluation or discover more selected bodies; it is not one uninterrupted pass over the whole program.", "omega-rust/psi/compilation/terminal-production/src/lib.rs#L1"),
        node("terminal", "Checked Terminal Psi", "hub", 520, 80, "psi", "The checker accepts the portable program and its evidence. Terminal Psi names required operations, such as Console::write; it does not choose the target provider. Decoding bytes alone grants no execution authority.", "omega-rust/psi/compilation/terminal-production/src/lib.rs#L1"),
        node("settle", "Provider closure", "around", 770, 80, "omega", "Build supplies provider choices. Settlement follows reached exact bodies and repeats when they reveal new requirements. For linux_x86_64, the library selects LinuxX86Console, whose raw LinuxX86ConsoleIo::write boundary is bound to syscall 1. This is a source trace, not an observed successful run.", settlement),
    ]
    common[-1]["example_source"] = code + "source/library/std/build.omg#L677"
    shared_edges = [
        edge("request", "build", "source", "source request", "M200 118 H270", 235, 68),
        edge("accepted", "source", "terminal", "lower & check", "M450 118 H520", 485, 68, "checked"),
        edge("requirements", "terminal", "settle", "requirements", "M700 118 H770", 735, 68),
        edge("bindings", "build", "settle", "target & provider bindings", "M110 80 V22 H860 V80", 485, 16),
        edge("closure", "settle", "source", "new requirements → prepare reached bodies", "M820 156 V204 H360 V156", 590, 194),
    ]
    shared_steps = [
        step("Begin with a Build request", "Source choices, target and provider bindings enter through Build. This map follows selected handoffs in the Rust reference producer.", "build", "request", "bindings"),
        step("Prepare and check the program", "Psi source stages prepare the portable program. The lowered-form checker establishes acceptance; publication retains its code and evidence.", "terminal", "request", "accepted"),
        step("Close the provider choices", "Reached requirements meet Build's bindings. A selected provider can expose more requirements, so source preparation and settlement can repeat until choices stabilize. The loop is conditional, not one measured iteration.", "settle", "requirements", "bindings", "closure"),
    ]
    native_nodes = common + [
        node("early", "Root: Omega 00–02", "native", 770, 280, "omega", "Omega 00 admits selected plans and creates abstract operations; 01 optimizes and 02 lowers for the target. The root then pauses before 03 while required handler objects are compiled.", pipeline),
        node("handlers", "Handlers, if needed", "native", 520, 280, "omega", "Required handler objects are compiled only when needed, and that compilation may recurse. Their requirements include transitive body demand, bridge bytes, call bias and fault evidence, not just the local frame size. Cyclic resource dependencies are refused.", omega + "compilation/backend/images/image-emission/src/object_artifact.rs#L1521"),
        node("bind", "Bind installations", "around", 270, 280, "omega", "Prepared obligations meet measured handler requirements. The driver binds installation authority before root Omega 03; these measurements are unavailable at initial provider settlement.", omega + "compilation/native-compiler/src/realization/callback_thunks.rs#L78-L168"),
        node("emit", "Root: 03–09 & emission", "native", 20, 280, "omega", "Selection, allocation, encoding and layout lead to fragment and object emission. Emission also receives retained EmissionFacts beside the machine forms; the detailed Omega map shows that separate path and distinguishes explicit audits.", omega + "compilation/native-compiler/src/pipeline.rs#L265"),
        node("image", "Image & entry bridges", "native", 20, 460, "omega", "Image construction still creates entry and adapter machine code. It does more than package finished bytes. Moving all generated bodies into an earlier common pipeline is a proposal, not today's route.", omega + "compilation/backend/images/image-emission/src/entries.rs#L1296"),
    ]
    native_edges = shared_edges + [
        edge("selected", "settle", "early", "settled native inputs", "M860 156 V280", 900, 235),
        edge("handler-demand", "early", "handlers", "handlers, if needed", "M770 318 H700", 735, 268),
        edge("measured", "handlers", "bind", "measured requirements", "M520 318 H450", 485, 260),
        edge("obligations", "early", "bind", "prepared installation obligations", "M860 356 V410 H360 V356", 610, 400),
        edge("bound", "bind", "emit", "bound facts", "M270 318 H200", 235, 268),
        edge("objects", "emit", "image", "objects & entry plans", "M110 356 V460", 205, 415),
    ]
    native_steps = shared_steps + [
        step("Admit and lower the root", "Settled native inputs reach Omega 00–02. This is the beginning of the root body's native route, not the end of provider-related admission.", "early", "selected"),
        step("Pause for handlers, if needed", "When handlers are required, the root waits after 02. Their objects are compiled, possibly recursively, to obtain full installation requirements. This conditional detour interleaves root and handler work; not every compile takes it.", "handlers", "handler-demand"),
        step("Bind measured installation facts", "Prepared root obligations and measured handler requirements meet here, before root 03. Early settlement cannot supply values that later native work has yet to measure.", "bind", "measured", "obligations"),
        step("Continue the root and emit objects", "Bound facts enter root 03. Later stages select, allocate, encode and lay out machine forms before emission. This overview omits the separate EmissionFacts path; open the Omega page for it.", "emit", "bound"),
        step("Construct the image and its bridges", "Objects and entry plans enter image construction, which still creates entry/adapter machine code. This walkthrough describes source order, not successful execution or elapsed time.", "image", "objects"),
    ]
    psi_nodes = common + [
        node("admission", "Runtime admission", "around", 770, 280, "interpreter", "The Psi product route calls admit_runtime. It returns admitted runtime provisions without emitting a native image; provider selection alone does not grant host authority.", omega + "build-system/orchestration/src/compile/native_completion.rs#L100-L159"),
        node("instance", "Load & bind Psi", "around", 520, 280, "interpreter", "The retained artifact is loaded or borrowed, supplies are admitted, and an instance is bound to its declared imports. Artifact identity and supplied capabilities must match.", runtime),
        node("invoke", "Invoke the instance", "around", 270, 280, "interpreter", "The interpreter invokes or resumes the bound instance under a work meter. Host adapters supply capabilities; decoded code by itself is not execution authority.", runtime),
        node("outcome", "Runtime observation", "around", 20, 280, "cli", "The caller receives execution status, output and resource observations. This source walkthrough is not a recording of an actual run or a native/Psi agreement result.", runtime),
    ]
    psi_edges = shared_edges + [
        edge("runtime-request", "settle", "admission", "selected runtime input", "M860 156 V280", 900, 235),
        edge("provisions", "admission", "instance", "admitted provisions", "M770 318 H700", 735, 260),
        edge("artifact", "terminal", "instance", "retained artifact", "M610 156 V280", 670, 235),
        edge("bound-instance", "instance", "invoke", "bound instance", "M520 318 H450", 485, 268),
        edge("observed", "invoke", "outcome", "status & output", "M270 318 H200", 235, 268),
    ]
    psi_steps = shared_steps + [
        step("Admit runtime provisions", "The Psi product route requests runtime admission. The result supplies runtime capabilities, rather than an emitted native image.", "admission", "runtime-request"),
        step("Bring code and capabilities together", "The retained Psi artifact and admitted provisions meet when the runtime loads and binds an instance. Neither an artifact decode nor a provider name alone authorizes execution.", "instance", "artifact", "provisions"),
        step("Invoke under a work meter", "The interpreter invokes the bound instance and accounts for its work. The detailed interpreter page shows the embedding and host adapter boundaries.", "invoke", "bound-instance"),
        step("Observe the result", "The caller receives status and output. These arrows explain the source route; they do not report that a sample succeeded.", "outcome", "observed"),
    ]
    selected = [
        ("compiler", "Compiler driver", 20, 210, "build-and-packages"),
        ("source-compiler", "Source compiler", 360, 40, "psi"),
        ("native-compiler", "Native compiler", 360, 210, "omega"),
        ("provider-planning", "Provider planning", 360, 400, "omega"),
        ("selected-dispatch", "Selected dispatch", 20, 400, "omega"),
        ("terminal-psi", "Terminal Psi form", 770, 210, "psi"),
    ]
    dependency_nodes = [node(name, label, "around", x, y, page,
        f"Cargo package {name}. This view shows normal path dependency declarations among six selected packages. A declaration is not proof of a call, an active feature configuration, or runtime order.", graph["packages"][name] + "/Cargo.toml")
        for name, label, x, y, page in selected]
    positions = {n["id"]: n for n in dependency_nodes}
    dependency_edges = []
    for record in graph["edges"]:
        start, end = record["from"], record["to"]
        if record["kind"] != "normal" or start not in positions or end not in positions:
            continue
        a, b = positions[start], positions[end]
        x1, y1, x2, y2 = a["x"]+180, a["y"]+38, b["x"], b["y"]+38
        # The direct compiler-to-representation declaration takes the upper lane.
        path = f"M{x1} {y1} C{x1+70} {y1} {x2-70} {y2} {x2} {y2}"
        if start == "compiler" and end == "terminal-psi":
            path = "M110 210 V12 H860 V210"
        elif a["x"] == b["x"]:
            path = f'M{a["x"]+90} {a["y"]+76} V{b["y"]}'
        dependency_edges.append(edge(start+"--"+end, start, end, "declares dependency", path, 0, 0))
        dependency_edges[-1]["declaration"] = record
    return [
        dict(id="native", label="Native compilation", category="Current program flow", claim="Selected native handoffs: provider closure precedes root lowering, handler work interleaves before 03, and image construction still creates bridge code.", note="Source trace of the Rust reference producer. Arrows name values or requests, not measured execution. Some paths are condensed; open a subsystem for its full account.", nodes=native_nodes, edges=native_edges, steps=native_steps, height=560),
        dict(id="psi", label="Psi execution", category="Current program flow", claim="The Psi product route joins retained checked code with admitted runtime provisions before binding and invocation.", note="Source trace of run --engine psi. Runtime admission is separate from provider selection. This is not a recording of a successful run.", nodes=psi_nodes, edges=psi_edges, steps=psi_steps, height=400),
        dict(id="dependencies", label="Cargo dependencies", category="Declared dependencies · not calls", claim="Normal Cargo path declarations among six selected packages; arrows point from the declaring package to its dependency, not through execution order.", note="Derived from graph.json at the compiler snapshot. Every normal declaration among these selected packages is shown. Build/dev declarations and other packages are outside this view; import uses and runtime calls are not inferred.", nodes=dependency_nodes, edges=dependency_edges, steps=[], height=510),
    ]


def render_explorer(revision, graph, prefix):
    data = models(revision, graph)
    esc = html.escape
    parts = ['<section class="flow-explorer" id="explore" aria-labelledby="explore-title"><div class="explorer-heading"><h2 id="explore-title">Follow a program through the codebase</h2><a href="'+prefix+'questions.html">Proposed changes & open questions →</a></div>',
             '<p class="explorer-intro">Select a box to see its inputs and outputs. Use Next to follow a route.</p>',
             '<div class="view-switch" role="group" aria-label="Map view" hidden>' + ''.join(f'<button type="button" data-view="{v["id"]}" aria-pressed="false">{v["label"]}</button>' for v in data) + '</div>']
    for view in data:
        key = view["id"]
        ids = {n["id"] for n in view["nodes"]}
        edge_ids = {e["id"] for e in view["edges"]}
        assert all(e["start"] in ids and e["end"] in ids for e in view["edges"])
        assert all(s["node"] in ids and set(s["edges"]) <= edge_ids for s in view["steps"])
        parts.append(f'<section class="flow-view" data-view-panel="{key}" aria-label="{view["label"]}"><h3 class="view-static-title">{esc(view["label"])}</h3><details class="view-evidence"><summary>{esc(view["category"])} — how to read this map</summary><p class="view-note">{esc(view["note"])}</p></details>')
        parts.append('<div class="flow-layout"><div class="flow-canvas"><figure class="diagram overview-diagram"><p class="diagram-hint">Scroll the map horizontally on a narrow screen. Selecting a step brings its box into view.</p><div class="diagram-frame" tabindex="0" role="region" aria-label="Scrollable '+esc(view["label"])+' map">')
        parts.append(f'<svg viewBox="0 0 1000 {view["height"]}" role="img" aria-label="{esc(view["claim"])}"><title>{esc(view["claim"])}</title><defs>')
        for kind in ("handoff", "checked"):
            parts.append(f'<marker id="flow-{key}-{kind}" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="7" markerHeight="7" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L10 5 L0 10 z" class="arrow-{kind}"/></marker>')
        parts.append('</defs>')
        for e in view["edges"]:
            parts.append(f'<g class="edge {e["kind"]}" data-edge="{e["id"]}" data-from="{e["start"]}" data-to="{e["end"]}"><title>{esc(e["label"])}</title><path d="{e["path"]}" marker-end="url(#flow-{key}-{e["kind"]})"/>')
            if key != "dependencies":
                for i, line in enumerate(textwrap.wrap(e["label"], 24)):
                    parts.append(f'<text x="{e["x"]}" y="{e["y"]+i*15}" text-anchor="middle">{esc(line)}</text>')
            parts.append('</g>')
        for n in view["nodes"]:
            parts.append(f'<g class="node {n["kind"]}" data-node="{n["id"]}" transform="translate({n["x"]} {n["y"]})"><rect width="180" height="76"/><title>{esc(n["label"])}</title>')
            lines = textwrap.wrap(n["label"], 23)
            for i, line in enumerate(lines):
                parts.append(f'<text x="90" y="{39-(len(lines)-1)*8+i*16}" text-anchor="middle">{esc(line)}</text>')
            parts.append('</g>')
        parts.append(f'</svg></div><figcaption>{esc(view["claim"])}</figcaption></figure><div class="tour" hidden><div class="tour-controls"><button type="button" data-action="previous">← Previous</button><button type="button" data-action="play" aria-pressed="false">Play walkthrough</button><button type="button" data-action="next">Next →</button></div><p class="tour-position"></p><div class="tour-copy" aria-live="polite"></div><p class="tour-note">A guided reading, not execution timing. Starts paused and stops at the last step.</p></div></div><aside class="flow-inspector" aria-label="Selected subsystem" hidden></aside></div>')
        parts.append('<details class="flow-reference"><summary>All handoffs and source notes</summary><ol>')
        for s in view["steps"]:
            parts.append(f'<li><strong>{esc(s["title"])}</strong> — {esc(s["text"])}</li>')
        parts.append('</ol><dl>')
        for n in view["nodes"]:
            outgoing = [f'{next(b["label"] for b in view["nodes"] if b["id"]==e["end"])}: {e["label"]}' for e in view["edges"] if e["start"]==n["id"]]
            parts.append(f'<dt>{esc(n["label"])}</dt><dd><p>{esc(n["description"])}</p><p>Supplies / declares: {esc("; ".join(outgoing) or "No outgoing edge in this selected view.")}</p><a href="{prefix}{n["page"]}.html">Read the subsystem</a> · <a href="{n["source"]}">Pinned source</a></dd>')
        parts.append('</dl></details></section>')
    parts.append('<script type="application/json" id="flow-model">'+json.dumps(data, ensure_ascii=False).replace('<', '\\u003c')+'</script></section>')
    return ''.join(parts), len(data)
