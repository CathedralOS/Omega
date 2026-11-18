"""The complete architecture overview. Positions are presentation, not stage order.

Relationships carry explicit meanings; only numbered transformation edges are
pipeline order. Source citations reuse the reviewed subsystem documents.
"""
import html
import json
import re

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
    add("build", "Build orchestration", "build", 2, 0, "build-and-packages", "source-route", "Executes Build entries and collects source choices, target, providers and requested product. The compiler's current settlement still retains source-side authority.")
    add("library", "Bundled library", "library", 3, 0, "library", "l01", "Core toolchain declarations, std/alloc packages and target provider bindings enter through package and Build ownership. Exceptional sealed intrinsics require compiler custody, not names alone.")
    add("source", "Source driver", "psi", 4, 0, "psi", "source-state", "Coordinates fresh source and retained continuations. Resolution, typing, preparation and compile-time evaluation can resume; this is not one uninterrupted pass over all source.")
    add("evaluation", "Source evaluator", "runtime", 5, 0, "interpreter", "source-execute", "The source compiler requests a compile-time value. This owner prepares and admits its own checked invocation, runs it in the Psi interpreter, and returns the value and work usage to the source caller. The interpreter never calls source stages.")
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
    add("providers", "Provider closure", "native", 0, 2, "omega", "closure", "Settlement follows reached exact bodies and Build bindings. When reach changes, it prepares policy inputs from the retained accepted publication and checked source custody, then realizes target supplies. It repeats until bindings stabilize; this loop does not restart source compilation. Available, demanded and retained code are different sets.")
    add("policy-inputs", "Prepare provider\npolicy inputs", "native", 0, 6, "omega", "provider-inputs", "Publishes the retained accepted closure and admits its source-selection packet. Reached requirements and Build bindings determine which calling policies to evaluate. The resulting calling plans return to settlement; publication does not re-lex or re-type the source.")
    add("policy", "Calling-policy\nevaluator", "runtime", 1, 6, "omega", "policy-evaluator", "Build settings prepares and admits a calling-policy invocation, invokes the Psi interpreter, validates the returned ABI calling plan, and accounts for work. This is a different caller from source compile-time evaluation.")
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
    edge("source", "evaluation", "evaluate compile-time value", "invoke", source=citations["psi"]["source-state"].split("/omega-rust/")[0]+"/omega-rust/psi/compilation/source-compiler/src/source_checking.rs#L119")
    edge("evaluation", "engine", "admitted compile-time artifact", "invoke")
    edge("engine", "evaluation", "value, effects and work usage")
    edge("evaluation", "source", "execution result for continuation")
    for i, label in enumerate(("tokens", "syntax trees", "resolved trees", "typed trees", "frozen trees", "lowered program")):
        edge(f"p{i}", f"p{i+1}", label, main=True)
    edge("p6", "checked", "accepted code", main=True)
    edge("checked", "providers", "reached requirements and accepted publication")
    edge("build", "providers", "Build supplies and target")
    edge("providers", "policy-inputs", "changed reach: prepare policy inputs", "invoke", source=citations["omega"]["closure-loop"])
    edge("checked", "policy-inputs", "accepted publication and source-selection custody", source=citations["omega"]["provider-inputs"])
    edge("policy-inputs", "policy", "evaluate demanded calling policies", "invoke")
    edge("policy", "engine", "admitted policy invocation", "invoke")
    edge("engine", "policy", "policy result and work usage", source=citations["omega"]["policy-evaluator"])
    edge("policy", "policy-inputs", "validated calling plan")
    edge("policy-inputs", "providers", "prepared target-supply inputs")
    edge("providers", "selected", "selected closure", main=True)
    edge("selected", "o0", "settled native inputs")
    edge("selected", "runtime-admission", "selected runtime inputs", source=citations["omega"]["psi-admission"])
    edge("checked", "host", "retained portable artifact", source=citations["omega"]["closure"].split("/omega-rust/")[0]+"/omega-rust/omega/application/src/execution/compilation.rs#L96")
    edge("runtime-admission", "host", "admitted provisions", main=True)
    edge("host", "engine", "bind and invoke", "invoke", main=True)
    edge("engine", "host", "execution status, output and work usage", source=citations["interpreter"]["host-owner"].replace("lib.rs#L1", "execution.rs#L421"))
    edge("host", "worker", "private provider calls", "invoke", source=citations["interpreter"]["host-owner"].replace("lib.rs#L1", "execution/host_imports/native_provider.rs#L1"))
    edge("worker", "host", "checked replies", source=citations["interpreter"]["host-owner"].replace("lib.rs#L1", "execution/host_imports/native_provider.rs#L1"))
    for i, label in enumerate(("abstract plan", "optimized plan", "target operations", "selected code", "allocation input", "register homes")):
        edge(f"o{i}", f"o{i+1}", label, main=True)
    edge("o2", "handlers", "compile required handlers, if any", "invoke")
    edge("handlers", "binding", "measured requirements", main=True)
    edge("o2", "binding", "prepared installation obligations")
    edge("binding", "o3", "bound installation authority", source=citations["omega"]["pipeline"].replace("#L84", "#L159"))
    edge("handlers", "o0", "recursive dependent-body compilation", "invoke")
    edge("o6", "frame-plan", "physical program", main=True)
    edge("frame-plan", "o7", "framed program", main=True)
    for a, b, label in (("o7","o8","encoded forms"),("o8","o9","resolved layout"),("o9","fragments","machine forms"),("fragments","frames","fragments"),("frames","text","framed fragments"),("text","objects","text and spans"),("objects","compose","objects"),("compose","bridges","composed bodies"),("bridges","image","entry/adapter bytes"),("image","products","retained image")):
        edge(a,b,label,main=True)
    edge("o2", "fragments", "EmissionFacts: separate retained abstract program", "bypass", source=citations["omega"]["emission-facts"])
    edge("products", "native-run", "requested executable", main=True, source=citations["repository"]["r02"].replace("main.rs#L47-L95", "execution/mod.rs#L244"))
    edge("cli", "native-run", "launch when requested", "invoke", source=citations["repository"]["r02"].replace("main.rs#L47-L95", "execution/mod.rs#L244"))
    edge("source", "foundation", "representative Cargo dependency", "dependency")
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
        ("checked","providers"), ("providers","policy-inputs"),
        ("policy-inputs","policy"), ("policy","engine"),
        ("policy-inputs","providers"),
        ("selected","o0"), ("selected","runtime-admission"),
        ("checked","host"), ("host","worker"), ("o2","handlers"),
        ("o2","binding"), ("binding","o3"), ("handlers","o0"),
        ("o2","fragments"),
    }
    for e in edges:
        e["main"] = e["main"] or (e["start"],e["end"]) in spine
    return dict(width=1760, height=1104, nodes=nodes, edges=edges,
                families=FAMILIES, relations=RELATIONS)


def render_layers(model, prefix):
    """Show the complete route before exposing per-stage explanations.

    Boxes are representations and arrows are transforms, as in the authored
    pipeline map. Return paths and late code production remain visible. Groups
    attach the cited stage records without putting their prose on the diagram.
    """
    esc = html.escape
    groups = [
        ('inputs', 'build', 'Build and packages', ['cli', 'packages', 'library', 'build']),
        ('source', 'psi', 'Source compilation · Psi 00–06', ['source']+[f'p{i}' for i in range(7)]+['checked']),
        ('providers', 'native', 'Provider selection', ['providers', 'policy-inputs', 'selected']),
        ('native', 'native', 'Native instructions · Omega 00–06', [f'o{i}' for i in range(7)]+['frame-plan', 'handlers', 'binding']),
        ('encoding', 'native', 'Encoding and layout · Omega 07–09', ['o7', 'o8', 'o9']),
        ('emission', 'native', 'Emission and objects', ['fragments', 'frames', 'text', 'objects', 'compose']),
        ('image', 'native', 'Native image and publication', ['bridges', 'image', 'products']),
        ('run', 'runtime', 'Native execution', ['native-run']),
        ('runtime', 'runtime', 'Psi runtime', ['runtime-admission', 'host', 'engine', 'worker']),
        ('evaluation', 'runtime', 'Compile-time execution', ['evaluation', 'policy']),
        ('support', 'support', 'Supporting systems', ['omega-source', 'bootstrap', 'foundation', 'docs', 'tests', 'tools']),
    ]
    nodes = {n['id']: n for n in model['nodes']}
    members = [node_id for *_, ids in groups for node_id in ids]
    if len(members) != len(nodes) or set(members) != nodes.keys():
        raise ValueError('Overview groups must include every architecture node exactly once')
    from .overview import render_overview
    parts = [render_overview(model)]
    parts.append('<details class="arch-component-list"><summary>Inspect layers and pinned source</summary>')
    for key, family, title, ids in groups:
        parts.append(f'<details class="arch-layer family-{family}" id="layer-{key}"><summary>{esc(title)}</summary><p><a href="#compiler-overview">↑ Back to the whole compiler</a></p><dl>')
        for node_id in ids:
            n = nodes[node_id]
            parts.append(f'<dt>{esc(n["title"].replace(chr(10), " · "))}</dt><dd><p>{esc(n["description"])}</p><a href="{prefix}{n["page"]}.html">Read subsystem</a> · <a href="{n["source"]}">Pinned source</a></dd>')
        parts.append('</dl></details>')
    parts.append('</details></div>')
    return ''.join(parts)

def render_architecture(atlas, prefix):
    model = architecture(atlas)
    revision = re.search(r'/blob/([0-9a-f]{40})/',model['nodes'][0]['source']).group(1)
    return ('<section class="architecture" data-theme="dark" id="architecture" aria-labelledby="architecture-title">'
            '<header class="arch-header"><div class="map-heading"><a class="atlas-mark" href="../index.html" aria-label="Atlas overview">Ω</a><div class="map-heading-text">'
            '<h2 id="architecture-title">Compiler architecture</h2>'
            f'<span class="arch-badge">Pre-migration snapshot · {revision[:12]}</span></div></div>'
            f'<a class="map-plan" href="{prefix}foundation-backend-plan.html#compiler-verification">Compiler verification <span aria-hidden="true">↗</span></a></header>'
            + render_layers(model, prefix)
            + '<script type="application/json" id="component-catalog">'
            + json.dumps([dict((key,n[key]) for key in ('id','title','family','page','source','description')) for n in model['nodes']],ensure_ascii=False).replace('<','\\u003c')
            + '</script></section>')
