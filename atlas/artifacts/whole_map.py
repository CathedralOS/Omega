"""The complete architecture overview. Positions are presentation, not stage order.

Relationships carry explicit meanings; only numbered transformation edges are
pipeline order. Source citations reuse the reviewed subsystem documents.
"""
import html
import json
import re
import textwrap

FAMILIES = {
    "psi": "Psi", "native": "Native & realization", "build": "Build & packages",
    "runtime": "Runtime", "library": "Library", "omega-source": "Omega-written compiler",
    "bootstrap": "Bootstrap", "support": "Shared code, tools & docs",
}
RELATIONS = {
    "data": "Data / transformation", "invoke": "Invocation / request",
    "dependency": "Declared dependency", "construction": "Construction / correspondence",
    "validation": "Validation / measurement", "normative": "Normative contract",
    "bypass": "Current separate value path",
}


def architecture(atlas):
    citations = {}
    for p in (atlas / "subsystems").glob("*.md"):
        citations[p.stem] = dict(re.findall(r'^\[([^]]+)\]: (https://\S+)', p.read_text(), re.M))
    nodes = []

    def add(key, title, family, column, row, page, ref, description):
        nodes.append(dict(id=key, title=title, family=family, x=32+212*column,
                          y=68+154*row, width=180, height=80, page=page,
                          source=citations[page][ref], description=description))

    add("cli", "CLI", "build", 0, 0, "repository", "r02", "The Rust reference application's typed commands invoke Build, execution, inspection and package owners. Inspection and test commands have distinct routes.")
    add("packages", "Packages &\ncustody", "build", 1, 0, "library", "l05", "Acquisition, package review, admitted graph and exact source custody constrain compilation. A directory or a manifest edge alone grants no package authority.")
    add("build", "Build orchestration", "build", 2, 0, "library", "l09", "Executes Build entries and collects source choices, target, providers and requested product. The compiler's current settlement still retains source-side authority.")
    add("library", "Bundled library", "library", 3, 0, "library", "l01", "Core toolchain declarations, std/alloc packages and target provider bindings enter through package and Build ownership. Exceptional sealed intrinsics require compiler custody, not names alone.")
    add("source", "Source driver", "psi", 4, 0, "psi", "source-state", "Coordinates fresh source and retained continuations. Resolution, typing, preparation and compile-time evaluation can resume; this is not one uninterrupted pass over all source.")
    add("evaluation", "Compile-time execution", "runtime", 5, 0, "interpreter", "source-execute", "Source and calling-policy owners prepare checked invocations for execution. The executor runs admitted artifacts; it does not call source stages itself. Current preparation is still coupled to source work.")
    add("foundation", "Foundation crates", "support", 6, 0, "psi", "source-manifest", "Supporting ownership, not a processing stage. Current foundation includes primitives and semantic/planning code. The proposed migration retains only justified generic primitives.")
    add("docs", "Specification & atlas", "support", 7, 0, "repository", "r03", "The specification states required contracts. Guides and atlas explain them; proposals and this migration plan are not adopted implementation behavior.")
    psi = [
        ("Lex", "Loaded source to tokens; the public facade can fuse lexing and parsing per source."),
        ("Parse", "Tokens to syntax trees, retaining source identity and metadata."),
        ("Resolve", "Syntax trees to symbol-resolved trees. Retained continuations can resume selected bodies."),
        ("Type", "Resolved trees to typed trees. Authored declaration-selection values already travel in the typed tables."),
        ("Prepare", "Specialization, domain applications and record patterns produce frozen typed trees; preparation can require evaluation."),
        ("Lower", "Frozen typed trees produce TerminalCompilation and source correspondence."),
        ("Check", "The lowered-form checker establishes acceptance from the lowered program. A decode alone is not proof acceptance."),
    ]
    for i, (title, description) in enumerate(psi):
        add(f"p{i}", f"Psi {i:02}\n{title}", "psi", i, 1, "psi", f"p{i:02}", description)
    add("checked", "Checked\nTerminal Psi", "psi", 7, 1, "psi", "publication", "Accepted portable code, evidence and exact publication identity form the hub. It states reached outside requirements; target/provider choice is separate.")
    add("providers", "Provider closure", "native", 0, 2, "omega", "closure", "Planning, selection and trust reconstruction follow reached exact bodies and Build bindings. Newly reached requirements may require source preparation and repeat settlement. Available, demanded and retained code are different sets.")
    add("selected", "Selected\nexecution", "native", 1, 2, "omega", "selected", "Retains selected bodies and plans for this program and target. This is early closure, not completion of all later native admission and installation work.")
    add("handlers", "Handlers\nif required", "native", 2, 2, "omega", "handler-demand", "The root can pause after Omega 02 while required handler objects are compiled, recursively if needed. Requirements include transitive body demand, bridge bytes, bias and fault evidence; cycles are refused.")
    add("binding", "Bind installations", "native", 3, 2, "omega", "bound", "Measured handler requirements meet admitted cohorts and prepared obligations before root Omega 03. The current implementation interleaves dependent native bodies.")
    add("runtime-admission", "Runtime\nadmission", "runtime", 4, 2, "omega", "psi-admission", "The Psi product route admits target/provider and entry provisions without emitting a native image. Provider choice alone does not confer host authority.")
    add("host", "Omega runtime\n& host adapters", "runtime", 5, 2, "interpreter", "host-owner", "Target-aware host ownership joins code identity, runtime provisions and external capabilities. It requests execution from the target-neutral Psi engine.")
    add("engine", "Psi interpreter", "runtime", 6, 2, "interpreter", "engine", "Loads or borrows code, admits supplies, binds imports and invokes/resumes the instance under a work meter. It never invokes the source compiler.")
    add("worker", "Native-provider\nworker", "runtime", 7, 2, "interpreter", "native-owner", "A private native provider process serves host-adapted external calls. It is distinct from running the user's emitted native executable.")
    native = [
        ("Admit & lower", "admission", "Selected Psi and native inputs become an admitted abstract operation plan; entry and provider evidence are checked here."),
        ("Optimize abstract", "pipeline", "Optimizes abstract operations while retaining projection/work identities."),
        ("Lower to target", "target", "Produces target operations and prepared execution facts. Handler demands and later EmissionFacts originate from this phase."),
        ("Select instructions", "selection", "Reads prepared target operations and bound installation facts to select instructions and register constraints."),
        ("Optimize selected", "physical", "Rewrites selected instructions under the current analysis/custody contract."),
        ("Allocate registers", "allocation", "Assigns register homes for physical lowering."),
        ("Physical operations", "machine", "Chooses physical operations and preservation requirements before frame planning."),
    ]
    for i, (title, ref, description) in enumerate(native):
        add(f"o{i}", f"Omega {i:02}\n{title}", "native", i, 3, "omega", ref, description)
    add("frame-plan", "Plan frames", "native", 7, 3, "omega", "frame-route", "Current unnumbered frame planning fixes geometry and protocols before encoding.")
    add("o7", "Omega 07\nEncode", "native", 7, 4, "omega", "encoding", "Encodes framed physical operations into bytes/encoding rows, carrying instruction and frame columns.")
    add("o8", "Omega 08\nPlace bytes", "native", 6, 4, "omega", "frame-route", "Resolves function-relative placement and encodings. Current later code generation must still be accounted for in the migration.")
    add("o9", "Omega 09\nOptimize layout", "native", 5, 4, "omega", "pipeline", "Rewrites the resolved machine layout under its carried selection/budget contract.")
    add("fragments", "Emit fragments", "native", 4, 4, "omega", "fragment-route", "Emits unplaced function fragments and fixups. Today it also receives EmissionFacts, including a shared optimized abstract program, beside current machine forms.")
    add("frames", "Apply frames", "native", 3, 4, "omega", "fragment-route", "Inserts prologue/epilogue behavior, reflows bytes and repairs displacements. This is current post-encoding production work.")
    add("text", "Place text", "native", 2, 4, "omega", "fragment-route", "Builds a text section with function, block and instruction spans; resolves internal calls.")
    add("objects", "Build objects", "native", 1, 4, "omega", "fragment-route", "Produces relocation-free object containers with bytes, layout, semantic attribution and composition evidence.")
    add("compose", "Compose\nprivate bodies", "native", 0, 4, "omega", "callback-composition", "Joins root and referenced callback objects. Selected handler and callback bodies can be lowered under the same native driver at different times.")
    add("bridges", "Entry & adapter\ncode generation", "native", 0, 5, "omega", "image", "Image construction still creates entry and adapter machine code. This is not merely packaging bytes, and is a remaining late producer in the proposed migration.")
    add("image", "Construct image", "native", 1, 5, "omega", "formats", "Builds the requested executable/component image and target container format. Late bridge production is part of this current route.")
    add("products", "Publish product", "build", 2, 5, "omega", "publish", "Product ownership writes requested outputs, records publication receipts and handles product packaging. It is distinct from forming a native image.")
    add("native-run", "Native program\n& operating\nsystem", "runtime", 3, 5, "repository", "r02", "The CLI can launch the emitted program and observe its status/output. The private native-provider worker is a separate execution route.")
    add("omega-source", "Omega-written\ncompiler", "omega-source", 4, 5, "repository", "r05", "source/psi and source/omega implement the product compiler in Omega, outside the bootstrap variants. The supported compiler subset does not yet establish complete self-rebuild.")
    add("bootstrap", "Bootstrap peers", "bootstrap", 5, 5, "repository", "r07", "Interpreted, compiled-VM and independent native construction routes are peers. Proof experiments have their own scope. Constructing the complete product compiler and its self-rebuild remains an unfinished objective.")
    add("tests", "Tests & samples", "support", 6, 5, "repository", "r08", "Named tests and samples supply witnesses, expected results and intended refusals. A source definition or diagram is not a fresh passing runtime observation.")
    add("tools", "Tools, CI &\nmeasurements", "support", 7, 5, "repository", "r12", "xtask, CI, benchmarks, release/editor tools and agent coordination maintain or assess the repository. Their results have narrower authority than language acceptance.")

    by_id = {n["id"]: n for n in nodes}
    edges = []

    def edge(a, b, label, kind="data", main=False, source=None):
        edges.append(dict(id=f"{a}--{b}--{len(edges)}", start=a, end=b, label=label,
                          kind=kind, main=main, source=source or by_id[a]["source"]))

    edge("cli", "build", "typed product request", "invoke")
    edge("packages", "build", "admitted graph and exact source")
    edge("library", "packages", "toolchain and ordinary package source")
    edge("library", "build", "target provider bindings")
    edge("library", "p2", "sealed intrinsic custody", "normative")
    edge("build", "source", "source request and retained authority", "invoke")
    edge("source", "p0", "fresh source", "invoke")
    edge("source", "p2", "retained resolution/typing continuation", "invoke")
    edge("source", "evaluation", "prepared source invocation", "invoke")
    edge("evaluation", "engine", "admitted compile-time artifact", "invoke")
    edge("engine", "evaluation", "value, effects and work usage")
    edge("evaluation", "source", "execution result for continuation")
    for i, label in enumerate(("tokens", "syntax trees", "resolved trees", "typed trees", "frozen trees", "lowered program")):
        edge(f"p{i}", f"p{i+1}", label, main=True)
    edge("p6", "checked", "accepted code", main=True)
    edge("checked", "providers", "reached requirements and accepted publication")
    edge("build", "providers", "Build supplies and target")
    edge("providers", "source", "newly reached bodies: repeat preparation", "invoke")
    edge("providers", "evaluation", "calling-policy invocation", "invoke", source=citations["omega"]["policy-evaluator"])
    edge("evaluation", "providers", "validated policy result and work usage")
    edge("providers", "selected", "selected closure", main=True)
    edge("selected", "o0", "settled native inputs")
    edge("selected", "runtime-admission", "Psi product admission request", "invoke")
    edge("checked", "host", "retained portable artifact")
    edge("runtime-admission", "host", "admitted provisions", main=True)
    edge("host", "engine", "bind and invoke", "invoke", main=True)
    edge("engine", "host", "external calls and execution status")
    edge("host", "worker", "private provider calls", "invoke")
    edge("worker", "host", "checked replies")
    for i, label in enumerate(("abstract plan", "optimized plan", "target operations", "selected code", "allocation input", "register homes")):
        edge(f"o{i}", f"o{i+1}", label, main=True)
    edge("o2", "handlers", "compile required handlers, if any", "invoke")
    edge("handlers", "binding", "measured requirements", main=True)
    edge("o2", "binding", "prepared installation obligations")
    edge("binding", "o3", "bound installation authority")
    edge("handlers", "o0", "recursive dependent-body compilation", "invoke")
    edge("o6", "frame-plan", "physical program", main=True)
    edge("frame-plan", "o7", "framed program", main=True)
    for a, b, label in (("o7","o8","encoded forms"),("o8","o9","resolved layout"),("o9","fragments","machine forms"),("fragments","frames","fragments"),("frames","text","framed fragments"),("text","objects","text and spans"),("objects","compose","objects"),("compose","bridges","composed bodies"),("bridges","image","entry/adapter bytes"),("image","products","retained image")):
        edge(a,b,label,main=True)
    edge("o2", "fragments", "EmissionFacts: separate retained abstract program", "bypass", source=citations["omega"]["emission-facts"])
    edge("products", "native-run", "requested executable", main=True)
    edge("cli", "native-run", "launch when requested", "invoke")
    edge("source", "foundation", "representative Cargo dependency", "dependency")
    edge("o0", "foundation", "shared vocabulary and primitives", "dependency")
    edge("docs", "source", "required language contracts", "normative")
    edge("docs", "omega-source", "specification governs both implementations", "normative")
    edge("source", "omega-source", "implementation correspondence, not runtime flow", "construction")
    edge("bootstrap", "omega-source", "full construction/self-rebuild is unfinished", "construction", main=True)
    edge("tests", "engine", "named execution and refusal witnesses", "validation")
    edge("tests", "native-run", "native/Psi observations", "validation")
    edge("tools", "tests", "run gates and record measurements", "validation", main=True)
    edge("tools", "products", "release and artifact inspection", "validation")
    # Keep the overview connected across owners. The optional layer adds
    # supporting declarations/contracts/witnesses, not missing pipeline links.
    spine = {
        ("cli","build"), ("packages","build"), ("library","packages"),
        ("build","source"), ("build","providers"), ("source","p0"),
        ("source","evaluation"), ("evaluation","engine"),
        ("checked","providers"), ("providers","source"),
        ("selected","o0"), ("selected","runtime-admission"),
        ("checked","host"), ("host","worker"), ("o2","handlers"),
        ("o2","binding"), ("binding","o3"), ("handlers","o0"),
        ("o2","fragments"),
    }
    for e in edges:
        e["main"] = e["main"] or (e["start"],e["end"]) in spine
    return dict(width=1760, height=950, nodes=nodes, edges=edges,
                families=FAMILIES, relations=RELATIONS)


def path_for(a, b, index):
    ax, ay, bx, by = a["x"], a["y"], b["x"], b["y"]
    if ay == by and abs(ax-bx) == 212:
        start, end = (ax+180,bx) if bx>ax else (ax,bx+180)
        return f"M{start} {ay+40} H{end}", ((start+end)/2,ay-9)
    if ax == bx and abs(ay-by) == 154:
        start, end = (ay+80,by) if by>ay else (ay,by+80)
        return f"M{ax+90} {start} V{end}", (ax+100,(start+end)/2)
    lane = 13 + index % 3 * 5
    if ay == by:
        y = ay-24-index%3*5
        return f"M{ax+90} {ay} V{y} H{bx+90} V{by}", ((ax+bx)/2+90,y-5)
    channel = ax+180+lane if bx>=ax else ax-lane
    end_y = by-25 if by>ay else by+105
    end = by if by>ay else by+80
    start = ax+180 if bx>=ax else ax
    return f"M{start} {ay+40} H{channel} V{end_y} H{bx+90} V{end}", ((channel+bx+90)/2,end_y-5)


def render_architecture(atlas, prefix):
    model = architecture(atlas)
    esc = html.escape
    claim = "Current Omega architecture connects source and native pipelines to Build, realization, runtime, construction and supporting owners. Relationship styles distinguish data, invocation, dependencies and non-execution links."
    pieces = ['<section class="architecture" id="architecture" aria-labelledby="architecture-title"><header class="arch-header"><div><span class="arch-badge">Current architecture · source map</span><h2 id="architecture-title">The whole system</h2></div><a href="'+prefix+'foundation-backend-plan.html">Read the proposed migration plan →</a></header>',
              '<div class="arch-legend" aria-label="Subsystem families">' + ''.join(f'<span class="family-{key}"><i aria-hidden="true"></i>{name}</span>' for key,name in FAMILIES.items()) + '</div>',
              '<div class="arch-toolbar" hidden><div class="arch-zoom"><button type="button" data-map="out" aria-label="Zoom out">−</button><button type="button" data-map="natural" aria-label="Show at natural scale"><output class="arch-scale">100%</output></button><button type="button" data-map="in" aria-label="Zoom in">+</button><button type="button" data-map="fit">Fit all</button><button type="button" data-map="reset">Reset</button><button type="button" data-map="fullscreen">Fullscreen</button></div><label class="arch-extra"><input class="arch-all-relations" type="checkbox"> All relationships</label><label class="arch-jump">Jump to <select aria-label="Find a stage or subsystem"><option value="">Choose a box…</option>' + ''.join(f'<optgroup label="{name}">'+''.join(f'<option value="{n["id"]}">{esc(n["title"].replace(chr(10)," · "))}</option>' for n in model['nodes'] if n['family']==key)+'</optgroup>' for key,name in FAMILIES.items()) + '</select></label></div>',
              '<details class="arch-key"><summary>Relationship legend · primary paths shown; select a box for every connection</summary><div class="arch-relations">' + ''.join(f'<span><svg class="relation-{key}" viewBox="0 0 40 12" aria-hidden="true"><path d="M0 6 H40"/></svg>{name}</span>' for key,name in RELATIONS.items()) + '</div></details>',
              '<div class="arch-workspace"><figure class="architecture-figure"><div class="arch-viewport" tabindex="0" role="region" aria-label="Full architecture map. Drag or scroll to pan; Control plus wheel zooms. Arrow keys pan, Home resets, F fits all. Use Jump to select any stage or subsystem."><canvas class="arch-canvas" aria-hidden="true" hidden></canvas><svg class="arch-svg" viewBox="0 0 1760 950" role="img" aria-label="'+claim+'"><title>'+claim+'</title><defs>'+''.join(f'<marker id="arch-arrow{suffix}" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L10 5 L0 10 z" style="fill:var(--{token})"/></marker>' for suffix,token in (("","ink-muted"),("-trap","trap"),("-quiet","rule")))+'</defs><g class="arch-scene">']
    nodes = {n["id"]:n for n in model["nodes"]}
    for i,e in enumerate(model["edges"]):
        path,(x,y)=path_for(nodes[e["start"]],nodes[e["end"]],i)
        pieces.append(f'<g class="arch-edge relation-{e["kind"]}{" main-edge" if e["main"] else ""}" data-edge="{e["id"]}" data-from="{e["start"]}" data-to="{e["end"]}"><title>{esc(e["label"])}</title><path d="{path}" marker-end="url(#arch-arrow{"-trap" if e["kind"] == "bypass" else ""})"/><text x="{x}" y="{y}" text-anchor="middle">{esc(e["label"])}</text></g>')
    for row,label in enumerate(("Inputs, coordination & supporting ownership", "Source: Psi 00–06", "Realization & execution", "Native lowering: Omega 00–06", "Native encoding & emission", "Products, construction & verification")):
        pieces.append(f'<text class="arch-row-label" x="32" y="{28+154*row}">{esc(label)}</text>')
    for n in model["nodes"]:
        pieces.append(f'<g class="arch-node family-{n["family"]}" data-node="{n["id"]}" transform="translate({n["x"]} {n["y"]})"><title>{esc(n["title"])}</title><rect width="180" height="80"/><path class="family-stripe" d="M0 0 H180 V7 H0 Z"/>')
        lines=[line for part in n["title"].splitlines() for line in textwrap.wrap(part, width=18, break_long_words=False, break_on_hyphens=False)]
        for i,line in enumerate(lines):
            pieces.append(f'<text x="90" y="{47-(len(lines)-1)*12+i*24}" text-anchor="middle">{esc(line)}</text>')
        pieces.append('</g>')
    pieces.append('</g></svg></div><div class="arch-minimap" hidden><span>Whole map · click to navigate</span><canvas aria-hidden="true"></canvas></div><figcaption>'+claim+'</figcaption></figure><aside class="arch-detail" aria-label="Architecture selection" hidden></aside></div><p class="arch-help">Scroll or drag to pan · Ctrl/⌘ + scroll to zoom · select a box for connections · minimap shows your position. Reset restores readable labels; Fit all shows the whole map. No automatic motion.</p><details class="arch-outline"><summary>Read the complete map as text</summary><dl>')
    for n in model['nodes']:
        pieces.append(f'<dt>{esc(n["title"].replace(chr(10)," · "))}</dt><dd><p>{esc(n["description"])}</p><a href="{prefix}{n["page"]}.html">Subsystem page</a> · <a href="{n["source"]}">Pinned source</a><ul>')
        for e in model['edges']:
            if e['start']==n['id']:
                pieces.append(f'<li>{esc(RELATIONS[e["kind"]])} → {esc(nodes[e["end"]]["title"].replace(chr(10)," · "))}: {esc(e["label"])} · <a href="{e["source"]}">Relationship source</a></li>')
        pieces.append('</ul></dd>')
    pieces.append('</dl></details><script type="application/json" id="architecture-model">'+json.dumps(model,ensure_ascii=False).replace('<','\\u003c')+'</script></section>')
    return ''.join(pieces)
