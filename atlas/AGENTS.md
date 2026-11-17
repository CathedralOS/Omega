# Atlas: instructions for agents

These apply to every agent working in `atlas/` (Codex, GPT, Claude, any
other), on top of the repository's [AGENTS.md](../AGENTS.md).

Atlas is a planning workspace that maps the Omega codebase and plans how to
make it one linear pipeline. Nothing here is compiled. Read in this order:

1. [rulings.md](rulings.md): the owner's decisions. Plans follow them.
2. [README.md](README.md): what is here and how to regenerate the evidence.
3. [questions.md](questions.md): what is still open.
4. [design/README.md](design/README.md) before making or changing any page.

Rules:

- **Evidence first.** Every number comes from a script in
  `evidence/scripts/` that anyone can rerun; every quote carries `file:line`.
  Mark what you did not check *unverified* and every classification choice
  *judgment*. A claim without evidence goes in `questions.md`, not in a plan.
- **Pages are static HTML in `pages/`**, built to [design/README.md](design/README.md):
  they link `../design/tokens.css`, draw figures as inline SVG, and open from
  disk in any browser. Copy [design/page-template.html](design/page-template.html)
  to start one. Add every new page to the table in `README.md`.
- **Regenerate, don't hand-edit, generated files**: run
  `python3 atlas/build_pages.py` after changing a plan or an artifact source.
- **Change a ruling only in `rulings.md`**, with the date, and only when the
  owner made it.
- Write in plain language ([AGENTS.md "Explain the change"](../AGENTS.md#explain-the-change)).
