# Where compiler tests go

A compiler test is a Rust test in this directory, declared as a `[[test]]` in
the crate's `Cargo.toml`. Prove a change with a test on the changed code, then
a very small area around it (for the lowered-form checker, the
`lowered_route_*` tests here).

A program a test reads is kept in the repository's `tests/fixtures/`, in the
layout the former corpus used, so a fixture keeps the name it always had:

- `tests/fixtures/pass/<group>/<name>/main.omg` compiles.
- `tests/fixtures/fail/<group>/<name>/main.omg` rejects, and its `expected.txt`
  holds a fragment of the expected diagnostic.
- `tests/fixtures/run/<group>/<name>/` compiles and runs; its `main.omg` header
  or expectation files state the exit.
- `tests/fixtures/packages/` holds the package projects fixtures depend on.

The directory sits at the depth `tests/omega/` had, because a fixture's
`build.omg` reaches the bundled library by a relative path
(`../../../../../source/library/std`); moving a fixture deeper breaks it.
Tests in other crates read the same directory. Keep only the programs a test
names, never a matrix: `cargo xtask check-rules` allows at most six new `.omg`
files per pull request. Name a fixture for the compiler behavior it pins. A
fixture that imports `omega_language_std` needs a `build.omg` that depends on
`source/library/std` and binds the program entry roots.

Nothing runs every fixture and nothing records outcomes. The owner's ruling
of 2026-11-13 removed the fixture corpus in `tests/omega/`, its runner
(`corpus_runner.rs`) and its ledger;
[tests/README.md](../../../../../tests/README.md) says how to read a deleted
fixture from history. A file in a kept fixture that only the runner read (for
example `expected_terminal_inspections.txt`) is now a note about the program,
not a checked expected result.

## Running a fixture through the CLI

Tests build their package inputs explicitly, so their native results do not
show the ordinary install/update workflow works. An unlocked CLI run supplies
no accepted semantic bindings, so a fixture using package-owned std can stop
at entry-schema admission before the behavior under test. Do not bypass that
check or treat a dependency as accepted because of its name or location.

For example, prepare the reference-pressure fixture on macOS ARM64 from the
repository root:

```text
omega packages update --project tests/fixtures/pass/borrows/runtime_reference_array_pressure_exit --target macos_arm64
```

If review asks for decisions, read the reported file and accept or reject
each finding. Then resume without repeating `--target`, and run the source:

```text
omega packages update --resume --project tests/fixtures/pass/borrows/runtime_reference_array_pressure_exit
omega run tests/fixtures/pass/borrows/runtime_reference_array_pressure_exit/main.omg
```

The program succeeds with process exit 70, not 0; a compilation failure is
CLI exit 200. `run --both` also runs the Psi interpreter and compares; give it
finite stdin as the [CLI guide](../../../application/README.md) describes.
The local lock accepts this exact source and compiler, not a portable
result. A source or compiler change needs fresh review, not copied bindings
or a deleted stale lock.

The source-based check of target and provider records runs in the SDK suite:
`mbx nextest run -p compiler --test package_review historical_constant_closure_requires_exact_target_and_provider`.
It checks canonical transport, exact target and provider inputs, and Windows
Terminal execution, without running a foreign native image.

## Shared multi-target pipeline observations

The `terminal_sharing` example runs in its own process so parallel work
cannot skew its counts. It uses the ordinary reviewed fixture inputs
and source-to-native entry point. For the
byte-conversion fixture's named target implementation:

```bash
OMEGA_PHASE_PROFILE=1 mbx run -p compiler --example terminal_sharing -- \
  --assert-native-stage-counts --compare-single-targets --run \
  tests/fixtures/pass/facts/unsigned_byte_to_signed_case_domain_through_target_body_exit/main.omg \
  linux_x86_64 windows_x86_64
```

Each target must have one completed application run that enters Omega
00–09 once each and attempts fragment/image emission once. Each image must
equal the image from compiling that target alone. The host product runs
natively and from a reloaded Terminal artifact. The shared
request must also complete source, run Build and produce Terminal Psi once
each, using one Terminal allocation for both targets. Its `checks` field reports
the legacy request counter. It does not count the lowered-form judgments. Checking every numbered
source and Psi stage count is still open.

The command prints `native_pipeline_evidence` and keeps the canonical
`program.oterm`, invocation records, each `<target>-shared.image` and each
`<target>-single.image` in its process directory. The counters are optional
observations, never an input to checking, admission or reuse. A failed run keeps its entered stages. Separate callable-body pipelines show as separate
units; the example refuses to count them as one application pipeline.

The control case for a refusal on one target uses the same entry point:

```bash
OMEGA_PHASE_PROFILE=1 mbx run -p compiler --example terminal_sharing -- \
  --assert-native-stage-counts --compare-single-targets --run \
  --reject macos_arm64 --reason 'behavior exclusion violated: crash cause Trap' \
  tests/fixtures/fail/targets/distinct_roots_exclusion_selected_body/main.omg \
  linux_x86_64 macos_arm64
```

The Linux product still completes its full pipeline, matches its separate
compilation and runs on both engines. macOS is rejected as intended; this
control rejects before native production starts, so its native count is zero.
