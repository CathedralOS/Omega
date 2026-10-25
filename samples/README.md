# Omega Samples

Samples are miniature Omega projects that pressure-test the language from a
user-code point of view. A runnable sample's `build.omg`
binds a target's program entry, and its `main.omg` header documents the result
with `// Expected exit: <n>` or `// Expected output contains: <text>`, so
`tools/sample_native_gate.py` can hold it to that result. A check-only probe of
one language behavior is a fixture under `tests/omega/{pass,fail}/<behavior>/`,
not a sample; the existing `cli/probes/` projects predate this rule, and new
probes go to the fixture tree. When the
boards run dry, new samples at the language frontier are how the next real work
is found.

Samples can grow through actual use into complete programs or supply reusable
libraries. Keep useful projects at their current paths; size or maturity alone
does not require a move, deletion or another permanent hierarchy. A stable
contract describes observable behavior and obligations; it does not freeze an
implementation. Extract a shared kernel only after real consumers establish
its contract. One outcome owner extracts it into the existing `source/library`
and migrates its consumers together, retaining each consumer's frozen oracles,
invalid controls and host evidence. Extraction is a separately bounded outcome,
not permission for a broad library rewrite.

## Nested-machine migration

The [ratified machine model](../wiki/spec/language/machines.md#nested-machines-and-identity)
replaces source `state` declarations with arbitrarily nested `machine`
declarations. Sample READMEs describe that end state ahead of the source/compiler
migration. Existing `.omg` files, fixture names and `other_state/` paths may still
use the retired spelling; they are migration inputs, not a compatibility promise
or evidence that arbitrary nesting already works. Historical run records retain
their original producer and meaning.

The nine-place row distinguishes a lexically nested machine (`other_state/`)
from another non-nested machine (`other_machine/`); those paths remain stable
while their programs migrate. Preserve each fact and its oracle. Renaming alone
does not prove nested visibility, ordinary call/return, exact contracts, explicit
loans, cleanup or constant-stack cycles. `NESTED-MACHINE-UNIFICATION` in
[TASKS.md](../TASKS.md) owns that end-to-end migration, including libraries and
affected samples/invalid controls. Do not rewrite a failed witness to evade its
contract, capture its parent's locals, or replace a loop with growing recursion.

## Project roles and discovery

`python3 tools/sample_projects.py` derives the current catalog from existing
build roots. It does not keep another checked-in list of projects. Both sample
gates use this discovery, including nested consumers and invalid controls under
a parent program. W100 groups without their own build root are containers;
their child projects are the discovery results.

| Role | Meaning in repository tooling |
| --- | --- |
| `program` | Complete application declared with `Build::application`. |
| `example` | Runnable demonstration, also usable as ordinary user code. |
| `library-consumer` | Runnable witness of a reusable library contract. |
| `library` | Package declarations; qualify through their consumers, not a native entry invented by the gate. |
| `check-only` | Existing positive probe without a program-entry binding; failure is still a gate failure. New isolated compiler witnesses go in `tests/omega`. |
| `intended-invalid` | Complete source control that must reject for its authored reason. |
| `external` | Pinned submodule with its own workflow; excluded from these gates. |

The defaults use build declaration text, direct invalid-control names and
existing `expected.txt`, `expected_error.txt` or `expected_diagnostic.txt` files.
They are discovery hints, not a parser or an assertion of source validity.
Dependency packages inside an invalid project remain libraries, not additional
negative controls. Squalr is discovered as an external project from `.gitmodules`;
its Git submodule entry retains the pinned revision.

For an ambiguous role or non-`main.omg` entry, add one local `sample.json`:

```json
{
  "role": "intended-invalid",
  "entry": "rejected.omg",
  "rejection_contains": ["the authored callee contract", "the authored reason"]
}
```

`entry` names one local Omega source. `rejection_contains` is a list of literal
stderr diagnostic fragments, all required. Optional `rejection_any_contains`
requires at least one listed literal as well; `rejection_casefold: true`
preserves authored case-insensitive matching. These fields restate
the control's authored contract, not whatever error happens to occur on a machine. Without it, an intended-invalid
project uses every nonempty line of its existing diagnostic expectation file.
Missing expectations are reported as failures, never skipped or guessed from an
unrelated refusal. Source rejection must exit 1 and match the identity; timeout,
crash, unexpected acceptance, package/custody/provider refusals and wrong reasons
cannot pass the control. Negative controls enter `--check`, never package
acceptance or runtime execution. Runtime-invalid input cases remain cases of the
positive program and its own validator.

`sample_gate.py` checks the positive entries and validates the negative controls.
`sample_native_gate.py` shares the same selection and reports negative coverage
separately from runtime scores. Failed positive check-only probes still make it
fail. No-oracle and freestanding projects retain their explicit unscored reporting,
as does a project whose `build.omg` binds `ProgramEntry` only for other targets.
Library-only packages appear in the catalog; their runnable consumers enter the
gates. This changes repository discovery, not the shipped `omega refresh-samples`
command, package authority, review policy or compiler semantics.

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
The same outcome owner carries those compiler repairs and the complete sample
through acceptance and authorized landing. A compiler stage or overlapping file
does not create a worker dependency. Group related samples only by a demonstrated
shared capability or root cause, preserving each sample's frozen oracle and controls.

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
- `cli/audio/`: audio synthesis written as WAV files through the portable `WavOutput` boundary (`samples/cli/audio/wav_output`), which reaches the raw per-OS filesystem family per target, then played on the host speaker through the portable `AudioPlayback` boundary (`samples/cli/audio/audio_playback`) — winmm `PlaySoundW` on Windows, ALSA on Linux; macOS writes the WAV but has no playback provider yet.
- `cli/systems/`: host-ish, protocol, logging, task, ledger, and atomic samples.
- `cli/proofs/`: proof/domain-surface samples.
- `cli/probes/`: compact compiler/runtime behavior probes.
- `cli/interpreters/`: calculators, stack/token interpreters, and VM-ish samples.
- `cli/independent/`: complete bounded domain applications, with private algorithms,
  input variants, independent oracles and separate source-invalid controls.
- `cli/scientific/`: scientific models and reusable numerical-contract consumers;
  groups such as W100 contain multiple independently discoverable projects.
- `cli/device/`: device and extent-access programs exercising ordinary providers.
