# Atlas

Atlas is where the owner and agents map the Omega codebase and plan how to
reshape it into one linear pipeline. Every claim here points at evidence a
reader can rerun: a dependency graph from `cargo metadata`, a usage scan, a
line count, or a `file:line` quote. Prose that cannot cite its evidence is a
question, and goes in [questions.md](questions.md).

Nothing in this folder is compiled, shipped or read by the compiler. It is a
planning workspace beside the code, not part of it.

## What is here

| Path | What it holds |
| --- | --- |
| [rulings.md](rulings.md) | The owner's rulings, dated, in the order they were made. Plans follow them. |
| [questions.md](questions.md) | Questions that still need the owner, in plain terms, with the evidence behind each. |
| [plans/](plans/) | Migration plans. Each names a destination for every crate it covers and orders the moves as PRs that each end in a deletion. |
| [evidence/scripts/](evidence/scripts/) | The scanners that produce the evidence. |
| [evidence/data/](evidence/data/) | Their output, with the source revision it was taken at. |
| [artifacts/](artifacts/) | Sources for the generated pages. |
| [pages/](pages/) | Every page as a static HTML file. Open one in any browser; no tools or accounts needed. |
| [design/](design/) | The portable design kit: rules, tokens, fonts and a page template any model can use. |
| [AGENTS.md](AGENTS.md) | Instructions for any agent working here (Codex and other GPT agents read it automatically). |

## Pages

Every page lives in [pages/](pages/) as a static HTML file. The online copies
are the same pages published as Claude artifacts.

| Page | File | Online copy | Source | What it shows |
| --- | --- | --- | --- | --- |
| Omega pipeline map | [pages/pipeline-map.html](pages/pipeline-map.html) | [link](https://claude.ai/artifact/CbgTtPhFpp1FXGAidywf9P) | [artifacts/pipeline-map/](artifacts/pipeline-map/) | The whole compiler as one linear pipeline, with today's reach-arounds as an overlay. |
| Foundation and backend plan | [pages/foundation-backend-plan.html](pages/foundation-backend-plan.html) | [link](https://claude.ai/artifact/4Eff6G2RrUR4vdu5WjQVwM) | [artifacts/foundation-backend-plan/](artifacts/foundation-backend-plan/) | Where every crate in `foundation/` and `backend/` goes, in what order. |

## Working with any model

Nothing here needs Claude's tools. A page is a static HTML file that links
[design/tokens.css](design/tokens.css), so GPT, Codex or any other agent can
read the evidence, edit a plan, and write or change a page with ordinary file
edits. Start from [design/page-template.html](design/page-template.html) and
follow [design/README.md](design/README.md). [AGENTS.md](AGENTS.md) holds the
working rules; Codex reads it automatically when it works in this folder.

## Evidence: how to reproduce it

The data in `evidence/data/` was taken at revision
`6de27da2a36d86f6c3883cb89527d0d44dea646a`. To refresh it from the repository
root (no build needed):

```bash
cargo metadata --format-version 1 --no-deps > /tmp/omega-meta.json
python3 atlas/evidence/scripts/users.py /tmp/omega-meta.json > atlas/evidence/data/users.txt
python3 atlas/evidence/scripts/modscan.py /tmp/omega-meta.json atlas/evidence/data/usage.json
python3 atlas/evidence/scripts/modscan_be.py /tmp/omega-meta.json atlas/evidence/data/usage_be.json
python3 atlas/evidence/scripts/backend_users.py < /tmp/omega-meta.json
python3 atlas/evidence/scripts/stage_dependencies.py < /tmp/omega-meta.json
```

- `users.py`: every foundation crate's production users, grouped by pipeline position.
- `modscan.py` / `modscan_be.py`: which modules and types of foundation and backend crates each crate names in non-test code.
- `backend_users.py`: line counts and users of every backend crate.
- `stage_dependencies.py`: which crates outside the pipeline call a numbered stage.

The module scans read source with regular expressions, not a Rust resolver. A
glob import or a path inside a macro can be missed, so a "declared but unused"
dependency needs a type check before it is deleted. The plans mark those
claims *unverified*.

## Rebuilding the pages

```bash
python3 -m pip install markdown==3.7   # once, for the plan page
python3 atlas/build_pages.py
```

`build_pages.py` runs each builder in `artifacts/` and writes the result to
`pages/<name>.html`, with fonts loaded from `design/fonts/`. Commit the
regenerated pages with the change that caused them.

## Rules for working here

- **Evidence first.** A number comes from a script in `evidence/scripts/`; a
  quote comes with its `file:line`. Mark anything not checked *unverified* and
  every classification choice *judgment*.
- **One linear pipeline is the target.** Stages read one representation and
  write the next. Anything beside the pipeline (foundation beyond primitives,
  the backend, sidecars, stage calls from outside the driver) is something to
  move, and the plan says where.
- **Plain language.** Follow [AGENTS.md "Explain the change"](../AGENTS.md#explain-the-change):
  lead with what changes and why it matters, then the evidence.
- **Pages use the Omega design system** (Limestone and Ink themes, Omega Serif,
  stone corners). A page shows a mechanism, not a name: the arrow that moves,
  the boundary crossed, the before and after.
