# Atlas

Atlas maps the codebase and plans the owner's linear pipeline. Observations
cite pinned source; recommendations are *judgment* and unchecked claims are
*unverified*. This folder changes neither compiler behavior nor owner rulings.

## Read the map

| Page | Scope |
| --- | --- |
| [Repository](subsystems/repository.md) | Rust reference, Omega source, products and tests. |
| [Psi](subsystems/psi.md) | Source stages, checking and publication. |
| [Omega](subsystems/omega.md) | Realization, native stages, installations and emission. |
| [Build and packages](subsystems/build-and-packages.md) | Custody, evaluation, requests and providers. |
| [CLI](subsystems/cli.md) | Dispatch, compilation, execution and inspection. |
| [Interpreter](subsystems/interpreter.md) | Program loading and runtime admission. |
| [Bootstrap](subsystems/bootstrap.md) | Construction routes and self-rebuild limits. |
| [Tools](subsystems/tools.md) | Repository commands and verification. |
| [Library](subsystems/library.md) | Bundled declarations and target providers. |
| [Original map](artifacts/pipeline-map/original.html) | Preserved authored presentation, clearly labeled historical. |
| [Interactive architecture](artifacts/architecture.md) | One connected SVG canvas with shared components and pinned source. |
| [Reviewed pipeline routes](artifacts/pipeline-map/map.md) | Pre-migration routes and marked proposals. |
| [Foundation and backend plan](plans/foundation-and-backend.md) | Migration contracts and deletion conditions. |
| [Extra-arrow audit](plans/foundation-and-backend.md#start-with-the-extra-arrows) | Source-backed detours, required repairs and a handler before/after view; focused recheck at `fe6e3d2e0104859a25ebfd1b0bfe4b424885e75f`. |
| [Four-host native execution](plans/foundation-and-backend.md#native-execution-on-four-matching-hosts) | The selected-provider fixture built by both compilers and run on matching Linux, macOS and Windows hosts. |
| [Questions](questions.md) | Open decisions and recommendations. |
| [Owner rulings](rulings.md) | Dated decisions governing the destination. |

Each page traces inputs, outputs, consumers, checks and failures. The
[pipeline diagrams](artifacts/pipeline-map/map.md) distinguish current and
proposed routes. The [migration plan](plans/foundation-and-backend.md) orders
contracts and deletions. [Questions](questions.md) remain open;
[owner rulings](rulings.md) govern the destination.

## Pages and publication status

The [atlas index](index.html) starts with the compiler map, repair plan and
subsystem reference. A shared header links those three destinations, a searchable
All pages menu exposes the full directory, and long pages have local section
navigation. The [original map](pages/pipeline-map.html) remains a historical page. The
[interactive architecture](pages/architecture.html) opens on **Stages 00–10**.
Every numbered stage links to the Rust implementation at `c329436ed22`. **Before
migration** preserves the compiler at `3f9ce43f0c9d`. Psi forms are outlined,
native forms filled, checker acceptance green and extra paths red.
Pan and zoom the connected canvas; select boxes or arrows for their connections and source.
The grouped footer also reaches all 15 subject pages, including the
[pre-migration pipeline account](pages/pipeline-current.html) and
[plan](pages/foundation-backend-plan.html). Counts come from generated evidence.

The publishing workflow copies only `index.html`, `pages/` and `design/` to
<https://cathedralos.github.io/Omega/atlas/>. A branch build or draft PR is a
review candidate; it does not update that site. The generated pages link
Markdown sources and evidence on GitHub because those files are not deployed.

The **pre-migration snapshot** at `3f9ce43f0c9d179048911d18faa91ba93f60176d` supplies
the repository inventory and historical routes. The **numbered implementation**
links to `c329436ed22f18b41352985cff202b46129e6538`. Each execution record
names the build it exercised. **Atlas content** identifies the prose, evidence
and rendering sources used to build the pages. [Content pin](content-revision.txt)

The [original pipeline map](https://claude.ai/artifact/CbgTtPhFpp1FXGAidywf9P)
and [original plan](https://claude.ai/artifact/4Eff6G2RrUR4vdu5WjQVwM) are
historical publications. These builds do not update those URLs.

## Reproduce the evidence

From the repository root, with Python 3.10+ and the repository's Cargo toolchain:

```bash
python3 -m unittest discover -s tools/atlas/evidence/tests -v
python3 tools/atlas/evidence/scripts/regenerate.py --source-revision 3f9ce43f0c9d179048911d18faa91ba93f60176d
```

The generator extracts the pinned Git tree to a temporary directory, checks
every archived blob against Git, and runs
`cargo metadata --format-version 1 --no-deps --locked` there without compiling.
Current checkout changes do not enter the snapshot. It records scanner/output
hashes and counting rules in [manifest.json](evidence/data/manifest.json).
Compare a separate reproduction with:

```bash
python3 tools/atlas/evidence/scripts/regenerate.py --source-revision 3f9ce43f0c9d179048911d18faa91ba93f60176d --output-dir /tmp/atlas-evidence
diff -rq tools/atlas/evidence/data /tmp/atlas-evidence
```

The
[evidence guide](evidence/README.md) documents individual scanners and queries.

Text scans are not a Rust resolver. `unobserved_dependencies` does not mean
unused. Cargo edges do not prove runtime calls. Counts distinguish all
tracked `.rs` files from `src/` and include the backend `target/` source crate.

## Rebuild all pages

Dependencies: Python 3.10+, Node.js 18+, Bun 1.4.4, Graphviz (`dot` on `PATH`) and Markdown 3.7:

```bash
bun install --cwd tools/atlas/artifacts --frozen-lockfile
python3 -m venv /tmp/omega-atlas-venv
/tmp/omega-atlas-venv/bin/python -m pip install -r tools/atlas/artifacts/requirements.txt
/tmp/omega-atlas-venv/bin/python tools/atlas/build_pages.py
python3 tools/atlas/artifacts/check.py
python3 -m http.server 8765 --bind 127.0.0.1 --directory tools/atlas
```

Open <http://127.0.0.1:8765/>. The canonical build writes tracked `index.html`,
`pages/*.html` and `pages/build.json`. It first verifies that atlas sources
match their recorded content revision; the checker also verifies links and
source line bounds against the **cited Git objects**, not the current files.
Neither check proves the prose's interpretation of that source.

Full verification needs the cited commits locally. A shallow clone, or a
fresh clone after a squash merge, may lack them. Fetch the missing SHA named
by the checker (`git fetch origin <sha>`) before verifying; offline mode does
not claim those objects are present.

After changing Markdown, evidence, the renderer or the design kit:

1. Preview with `python3 tools/atlas/build_pages.py --draft` using the venv Python.
   It writes ignored `artifacts/out/` and labels the pages as an uncommitted
   draft. Serve all of `tools/atlas/` and open `/artifacts/out/` for working local
   source links.
2. Commit those source changes. Run
   `python3 tools/atlas/build_pages.py --content-revision HEAD` using the venv Python.
   This records the source commit and rebuilds the publication files.
3. Run `python3 tools/atlas/artifacts/check.py`, review both themes and screen sizes,
   then commit `content-revision.txt`, `index.html` and `pages/`. A later normal
   build must reproduce those files without a diff.

A standalone copy of `tools/atlas/` can render without Git objects:

```bash
/tmp/omega-atlas-venv/bin/python tools/atlas/build_pages.py --offline
python3 tools/atlas/artifacts/check.py --local-only
```

Offline mode keeps the recorded GitHub links but explicitly skips verification
of the content pin and compiler citations. Use the repository mode before a
publication update. The same static check accepts a deployed subset through
`--site /path/to/copied/atlas --local-only`; the checker script stays in the
full source folder.

For browser checks, install Playwright separately and use a local Chromium:

```bash
python3 tools/atlas/artifacts/browser_check.py --site tools/atlas --browser /usr/bin/chromium --base-url http://127.0.0.1:8765
```

This checks all pages at 1440px and 400px in both themes, fonts, figure labels,
navigation, theme switching and scrolling. Reports and screenshots go under
ignored `artifacts/out/browser-check/`. Without `--base-url`, it checks `file://`.
The current review environment blocks `file://` navigation by browser policy;
that transport remains *unverified*. Localhost checks and static checks of the
deployment subset cover rendering and asset paths without bypassing the policy.

One template links the [portable design kit](design/README.md), preserving
Limestone/Ink, square stone corners and the three local Omega Serif weights.
No font or script requires a network request. Graphviz renders inline SVG from
editable Mermaid and explicit [diagram semantics](artifacts/diagrams.json);
colors are never inferred from a node's label. Supported syntax:
`flowchart LR/RL/TD/TB/BT`, `node[label]`, `-->`, `-.->`, optional
`|edge labels|`. The kit lays flows out left to right. Captions and handoff
labels are required; unsupported syntax fails the build. Obsolete parallel
templates and build entrypoints have been removed.

## Explore locally

The [interactive architecture page](pages/architecture.html) has one connected
SVG canvas with a **Stages 00–10 / Before migration** toggle. The first view links each numbered stage
to a pinned implementation. Its operation-level detail retains the design contracts
and the evidence for particular routes. The [verification record](plans/foundation-and-backend.md#compiler-verification) identifies which compiler ran each check.
The second view preserves the earlier source compiler, provider selection, native
execution and interpreter. Trace a detour and switch views to see its repair. Both
views remember your position and zoom.

Drag to pan and scroll to zoom in either view. In Stages 00–10, use Stage to jump to a numbered group. Find reaches individual components in either view.
Select a box or arrow to highlight its connections and open pinned source.
Before migration opens at Build at 100% zoom. Follow the arrows across the stage groups. Orange marks a path in the repair plan. Trace isolates its handoffs and links to the recorded code.
Start returns to Build; the minimap and Fit whole compiler locate the view.
100% restores natural-scale type. Expand
keeps the same graph and camera. Arrow keys pan, F fits all, and Escape clears
selection or closes expansion. Without JavaScript, the same SVG scrolls inside
its frame and the source disclosures remain available.

`artifacts/overview.py` defines the unique nodes and cited connections.
`artifacts/proposed.py` defines the numbered stage contracts, their implementation
sources and their original plan citations. Both models use the same SVG renderer.
Numbered stages use the column layout in `artifacts/stage_flow.py`. Only one graph is mounted in
the interactive canvas. The proposed contract list remains readable without JS.
`artifacts/layout.mjs` places the numbered runs in columns and draws each call and return to its named endpoint.
`artifacts/connected.js` uses D3 Zoom to move the SVG and highlight connections.
`artifacts/architecture.css` keeps the original fonts, colors, boxes and arrows.
The stage and supporting-owner records live in `artifacts/whole_map.py`.
The [design kit](design/README.md) defines their visual meaning.
Source traces, numbered stage implementations and test runs each carry their
own pins. Supporting owners remain in the source disclosures.

The immutable original HTML is `artifacts/pipeline-map/original.html`, copied
from the recorded #3899 publication. The builder preserves every SVG byte and
changes local asset loading and the shared navigation wrapper. Its old
claims and estimates are not refreshed observations; `artifacts/pipeline-map/map.md`
owns the corrected current account.

Install the locked graph dependencies before building. The browser loads the
committed `design/diagram-zoom.js` locally.
After changing `artifacts/zoom-entry.js` or its dependencies, rebuild that
bundle and its license notices with:

```bash
bun run --cwd tools/atlas/artifacts bundle
```

For local iteration, build with `--draft` and serve `tools/atlas/` as above. Keep
`content-revision.txt`, `index.html` and tracked `pages/` unchanged until
publication. Check the graph and its browser interaction with:

```bash
python3 -m unittest tools.atlas.artifacts.test_overview
python3 tools/atlas/artifacts/check_architecture.py --base-url http://127.0.0.1:8765/artifacts/out/
```

## Rules for working here

- **Evidence first.** Cite generated data or pinned source lines. Mark
  unchecked claims *unverified* and classification choices *judgment*.
- **Follow the rulings.** One linear pipeline is the target. Current maps
  still show drivers, retained inputs and work outside numbered stages.
- **Trace values.** Name producers, storage, consumers and authorizing checks.
  Separate ordinary execution, tests and explicit audits.
- **Plain language.** Follow [AGENTS.md](../../AGENTS.md#explain-the-change).
- **Show mechanisms.** Use the Omega design system; explain what each arrow
  carries and mark proposals.
