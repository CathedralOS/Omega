# Omega Samples

Samples are miniature Omega projects that pressure-test the language from a
user-code point of view.

They are allowed to be rough while the language is moving, but each sample
should still have a clear project shape:

- `main.omg`: the entrypoint the compiler is pointed at.
- `build.omg`: targets and their trusted boundary packages, when the sample
  needs one (a `target` block lists `boundary` lines only — see
  `wiki/design_briefs/extern_boundary_and_format_domains.md` §4).
- `.gitignore`: local sample ignore rules, including `/build/`.
- Domain folders such as `data/`, `platform/`, `rooms/`, or `dungeon/`.

Generated compiler output belongs in local `build/` beside the sample
entrypoint. Do not check it in and do not make sample source depend on it.

Samples should read like code someone might write. If a sample exposes a small
missing compiler behavior, pin that behavior in `canaries/` with the smallest
feature-shaped program, then come back to the sample.

Current samples:

- `cli_mvp/`: smallest console program.
- `dungeon_crawler_cli/`: console input/output and room navigation.
- `math_proofs/`: theorem ladder of empty-body proof machines (check-only).
- `point_and_click/`: windowed game/state-machine sketch.
