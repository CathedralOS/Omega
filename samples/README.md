# Omega Samples

Samples are miniature Omega projects that pressure-test the language from a
user-code point of view.

They are allowed to be rough while the language is moving, but each sample
should still have a clear project shape:

- `main.omg`: the conventional source filename used by the sample harness. The
  filename does not make a machine the program entry.
- `build.omg`: ordinary build/configuration code. An installable sample selects
  its target and binds the target's program-entry slot to one exact source
  machine; dependencies and provider overrides are added only when needed. Do
  do not restore retired `target { boundary ... }` manifests; provider admission and
  selection are derived from the selected target plus explicit overrides.
- `.gitignore`: local sample ignore rules, including `/build/`.
- Domain folders such as `data/`, `platform/`, `rooms/`, or `dungeon/`.

Generated compiler output belongs in local `build/` beside the sample
entrypoint. Do not check it in and do not make sample source depend on it.

`omega refresh-samples <directory>` discovers sample `main.omg` files and
compiles them for the current host, using the same package builds, trust
admission, and native publication as an ordinary compile. It does not run the
executables or accept policy on your behalf. Complete any requested package
review first with `omega update --project <sample-directory>`; unresolved trust
admissions also remain errors. A failed sample is reported without preventing
the other samples from building, and any failure makes the command exit nonzero.

For example, from the repository root in either PowerShell or a macOS shell:

```text
mbx run -p omega -- update --project samples/cli/basics/cli_mvp
mbx run -p omega -- refresh-samples samples/cli/basics/cli_mvp
```

Use `cargo` instead of `mbx` if the wrapper is unavailable. These commands use
the host target; cross-target or firmware compilation uses the ordinary compile
command with an explicit target. Refreshing the whole corpus does not imply
that every sample is currently supported.

Samples should read like code someone might write. If a sample exposes a small
missing compiler behavior, pin that behavior in `tests/omega/` with the smallest
feature-shaped program, then come back to the sample.

The corpus is migrating from temporary `main`/`Main::main` discovery to explicit
entry-slot bindings. A sample may still exercise that unfinished migration path,
but it is not supported language behavior and its documentation must not present the
source filename or machine spelling as the language's entry-selection rule.

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
