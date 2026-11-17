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

Each page traces inputs, outputs, consumers, checks and failures. The
[pipeline diagrams](artifacts/pipeline-map/map.md) distinguish current and
proposed routes. The [migration plan](plans/foundation-and-backend.md) orders
contracts and deletions. [Questions](questions.md) remain open;
[owner rulings](rulings.md) govern the destination.

## Pages and publication status

The [local atlas](artifacts/out/index.html) includes every subsystem,
[pipeline diagrams](artifacts/out/pipeline.html), the
[plan](artifacts/out/plan.html), questions and rulings. Navigation and page
numbers come from one registry; counts and revision labels come from evidence.

The [original pipeline map](https://claude.ai/artifact/CbgTtPhFpp1FXGAidywf9P)
and [original plan](https://claude.ai/artifact/4Eff6G2RrUR4vdu5WjQVwM) are
historical publications. These builds do not update those URLs.

## Reproduce the evidence

From the repository root, with Python 3.10+ and the repository's Cargo toolchain:

```bash
python3 -m unittest discover -s atlas/evidence/tests -v
python3 atlas/evidence/scripts/regenerate.py --source-revision 3f9ce43f0c9d179048911d18faa91ba93f60176d
```

The generator runs `cargo metadata --format-version 1 --no-deps --locked`
without compiling. It checks the source pin and records scanner/output hashes
and counting rules in [manifest.json](evidence/data/manifest.json). Add
`--output-dir /tmp/atlas-evidence` for comparison output. The
[evidence guide](evidence/README.md) documents individual scanners and queries.

Text scans are not a Rust resolver. `unobserved_dependencies` does not mean
unused. Cargo edges do not prove runtime calls. Counts distinguish all
tracked `.rs` files from `src/` and include the backend `target/` source crate.

## Rebuild all pages

Dependencies: Python 3.10+, Graphviz (`dot` on `PATH`) and Markdown 3.7:

```bash
python3 -m venv /tmp/omega-atlas-venv
/tmp/omega-atlas-venv/bin/python -m pip install -r atlas/artifacts/requirements.txt
/tmp/omega-atlas-venv/bin/python atlas/artifacts/build.py
python3 atlas/artifacts/check.py
python3 -m http.server 8765 --bind 127.0.0.1 --directory atlas
```

Open <http://127.0.0.1:8765/artifacts/out/index.html>. Generated
`atlas/artifacts/out/` is ignored. Serve all of `atlas/` for working evidence
and source links. Builds need only that folder and the tools above; source
citations link to GitHub. The check additionally uses repository files to
verify cited paths and line bounds, not their meaning.

The original entrypoints call the shared renderer and report their pages:

```bash
/tmp/omega-atlas-venv/bin/python atlas/artifacts/pipeline-map/build_map.py
/tmp/omega-atlas-venv/bin/python atlas/artifacts/foundation-backend-plan/build_fplan.py
```

One template preserves Limestone/Ink, square stone corners and Omega Serif
when installed (Georgia fallback), without external font or script requests.
Graphviz renders inline SVG from editable Mermaid. Supported syntax:
`flowchart LR/RL/TD/TB/BT`, `node[label]`, `-->`, `-.->`, optional
`|edge labels|`. Unsupported syntax fails the build.

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
