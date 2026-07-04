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

Top-level sample domains:

- `cli/`: console and terminal-oriented programs, grouped again by pressure domain.
- `gui/`: real windowed host/UI experiments.
- `uefi/`: firmware-targeted samples.

CLI subdomains:

- `cli/basics/`: small onboarding and utility programs.
- `cli/arithmetic/`: numeric domains, checksums, counters, and transforms.
- `cli/text/`: parsing, formatting, strings, and string-like algorithms.
- `cli/collections/`: arrays, slices, buffers, inventories, and container pressure.
- `cli/algorithms/`: classic algorithm demonstrations.
- `cli/simulation/`: evolving state systems and time-ish models.
- `cli/games/`: interactive game or game-adjacent flows.
- `cli/rendering/`: terminal and pixel-style visual output.
- `cli/systems/`: host-ish, protocol, logging, task, ledger, and atomic samples.
- `cli/proofs/`: proof/domain-surface samples.
- `cli/probes/`: compact compiler/runtime behavior probes.
- `cli/interpreters/`: calculators, stack/token interpreters, and VM-ish samples.
