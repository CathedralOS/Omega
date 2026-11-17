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
| [Pipeline diagrams](artifacts/pipeline-map/map.md) | Current routes and marked proposals. |
| [Foundation and backend plan](plans/foundation-and-backend.md) | Migration contracts and deletion conditions. |
| [Questions](questions.md) | Open decisions and recommendations. |
| [Owner rulings](rulings.md) | Dated decisions governing the destination. |

Each page traces inputs, outputs, consumers, checks and failures. The
[pipeline diagrams](artifacts/pipeline-map/map.md) distinguish current and
proposed routes. The [migration plan](plans/foundation-and-backend.md) orders
contracts and deletions. [Questions](questions.md) remain open;
[owner rulings](rulings.md) govern the destination.

## Pages and publication status

The [atlas index](index.html) links all 13 subject pages, including the
[pipeline diagrams](pages/pipeline-map.html), the
[plan](pages/foundation-backend-plan.html), questions and rulings. Navigation
and page numbers come from one registry; counts come from generated evidence.
The two established page URLs remain unchanged.

The publishing workflow copies only `index.html`, `pages/` and `design/` to
<https://cathedralos.github.io/Omega/atlas/>. A branch build or draft PR is a
review candidate; it does not update that site. The generated pages link
Markdown sources and evidence on GitHub because those files are not deployed.

Every page distinguishes two revisions. **Compiler snapshot** is the source
reviewed for the map: `3f9ce43f0c9d179048911d18faa91ba93f60176d`. **Atlas content**
is the committed prose, evidence and rendering sources recorded in
[content-revision.txt](content-revision.txt). Publishing newer atlas pages does
not refresh the compiler observations or imply a compiler run.

The [original pipeline map](https://claude.ai/artifact/CbgTtPhFpp1FXGAidywf9P)
and [original plan](https://claude.ai/artifact/4Eff6G2RrUR4vdu5WjQVwM) are
historical publications. These builds do not update those URLs.

## Reproduce the evidence

From the repository root, with Python 3.10+ and the repository's Cargo toolchain:

```bash
python3 -m unittest discover -s atlas/evidence/tests -v
python3 atlas/evidence/scripts/regenerate.py --source-revision 3f9ce43f0c9d179048911d18faa91ba93f60176d
```

The generator extracts the pinned Git tree to a temporary directory, checks
every archived blob against Git, and runs
`cargo metadata --format-version 1 --no-deps --locked` there without compiling.
Current checkout changes do not enter the snapshot. It records scanner/output
hashes and counting rules in [manifest.json](evidence/data/manifest.json).
Compare a separate reproduction with:

```bash
python3 atlas/evidence/scripts/regenerate.py --source-revision 3f9ce43f0c9d179048911d18faa91ba93f60176d --output-dir /tmp/atlas-evidence
diff -rq atlas/evidence/data /tmp/atlas-evidence
```

The
[evidence guide](evidence/README.md) documents individual scanners and queries.

Text scans are not a Rust resolver. `unobserved_dependencies` does not mean
unused. Cargo edges do not prove runtime calls. Counts distinguish all
tracked `.rs` files from `src/` and include the backend `target/` source crate.

## Rebuild all pages

Dependencies: Python 3.10+, Graphviz (`dot` on `PATH`) and Markdown 3.7:

```bash
python3 -m venv /tmp/omega-atlas-venv
/tmp/omega-atlas-venv/bin/python -m pip install -r atlas/artifacts/requirements.txt
/tmp/omega-atlas-venv/bin/python atlas/build_pages.py
python3 atlas/artifacts/check.py
python3 -m http.server 8765 --bind 127.0.0.1 --directory atlas
```

Open <http://127.0.0.1:8765/>. The canonical build writes tracked `index.html`,
`pages/*.html` and `pages/build.json`. It first verifies that atlas sources
match their recorded content revision; the checker also verifies links and
source line bounds against the **cited Git objects**, not the current files.
Neither check proves the prose's interpretation of that source.

Full verification needs both recorded commits locally. A shallow clone, or a
fresh clone after a squash merge, may lack them. Fetch the missing SHA named
by the checker (`git fetch origin <sha>`) before verifying; offline mode does
not claim those objects are present.

After changing Markdown, evidence, the renderer or the design kit:

1. Preview with `python3 atlas/build_pages.py --draft` using the venv Python.
   It writes ignored `artifacts/out/` and labels the pages as an uncommitted
   draft. Serve all of `atlas/` and open `/artifacts/out/` for working local
   source links.
2. Commit those source changes. Run
   `python3 atlas/build_pages.py --content-revision HEAD` using the venv Python.
   This records the source commit and rebuilds the publication files.
3. Run `python3 atlas/artifacts/check.py`, review both themes and screen sizes,
   then commit `content-revision.txt`, `index.html` and `pages/`. A later normal
   build must reproduce those files without a diff.

A standalone copy of `atlas/` can render without Git objects:

```bash
/tmp/omega-atlas-venv/bin/python atlas/build_pages.py --offline
python3 atlas/artifacts/check.py --local-only
```

Offline mode keeps the recorded GitHub links but explicitly skips verification
of the content pin and compiler citations. Use the repository mode before a
publication update. The same static check accepts a deployed subset through
`--site /path/to/copied/atlas --local-only`; the checker script stays in the
full source folder.

For browser checks, install Playwright separately and use a local Chromium:

```bash
python3 atlas/artifacts/browser_check.py --site atlas --browser /usr/bin/chromium --base-url http://127.0.0.1:8765
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

The landing and repository pages include native and Psi walkthroughs. They
start paused; Next and Previous step through explanatory handoffs. Selecting
a box shows its incoming and outgoing connections and pinned source. Play is
a guided reading aid, not a measured execution trace. Reduced motion disables
automatic play. Existing subject diagrams also let readers select immediate
connections.

The Cargo view is separate: it derives normal dependency declarations among
selected packages from `evidence/data/graph.json`, not calls or runtime order.
The curated program routes, explanations and source anchors live in
`artifacts/explorer.py`; behavior is in `artifacts/explorer.js`. Proposed
architecture remains in the marked planning pages. Static diagrams and source
notes remain usable without JavaScript.

For a local iteration, build with `--draft` and serve `atlas/` as above. Keep
`content-revision.txt`, `index.html` and tracked `pages/` unchanged until a
publication is requested. Check the interaction paths against the running
preview with:

```bash
python3 atlas/artifacts/check_interactions.py --base-url http://127.0.0.1:8765/artifacts/out/
```

## Rules for working here

- **Evidence first.** Cite generated data or pinned source lines. Mark
  unchecked claims *unverified* and classification choices *judgment*.
- **Follow the rulings.** One linear pipeline is the target. Current maps
  still show drivers, retained inputs and work outside numbered stages.
- **Trace values.** Name producers, storage, consumers and authorizing checks.
  Separate ordinary execution, tests and explicit audits.
- **Plain language.** Follow [AGENTS.md](../AGENTS.md#explain-the-change).
- **Show mechanisms.** Use the Omega design system; explain what each arrow
  carries and mark proposals.
