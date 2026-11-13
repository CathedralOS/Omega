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

The directory sits at the depth `tests/omega/` had because a fixture's
`build.omg` reaches the bundled library by a relative path
(`../../../../../source/library/std`); moving a fixture deeper breaks it.
Tests in other crates read the same directory. Keep the few programs a test
names, never a matrix: `cargo xtask check-rules` caps new `.omg` files at six
per pull request. Name a fixture for the compiler behavior it pins. A fixture
that imports `omega_language_std` needs a `build.omg`: it depends on
`source/library/std` and binds the program entry roots.

Nothing runs every fixture and nothing records outcomes. The fixture corpus
that used to live in `tests/omega/`, its runner (`corpus_runner.rs`) and its
ledger were removed on the owner's ruling of 2026-11-13;
[tests/README.md](../../../../../tests/README.md) says how to read a deleted
fixture from history. A file in a kept fixture that only the runner read (for
example `expected_terminal_inspections.txt`) is now a note about the program,
not a checked oracle.

## Running a fixture through the CLI

The corpus constructs its package inputs explicitly. Its native result does
not establish the ordinary install/update workflow. An unlocked CLI invocation
supplies no accepted semantic bindings; a fixture depending on package-owned
std can therefore stop at entry-schema admission before reaching the compiler
behavior under test. Do not bypass that boundary or infer acceptance from the
dependency's name or location.

For example, prepare the reference-pressure witness on macOS ARM64 from the
repository root:

```text
omega update --project tests/fixtures/pass/borrows/runtime_reference_array_pressure_exit --target macos_arm64
```

If review requests decisions, inspect the reported file and accept or reject
the actual findings. Only after resolving them, resume without repeating
`--target`, then run the original source:

```text
omega update --resume --project tests/fixtures/pass/borrows/runtime_reference_array_pressure_exit
omega run tests/fixtures/pass/borrows/runtime_reference_array_pressure_exit/main.omg
```

The authored success is process exit 70, not 0; compilation failure is CLI
exit 200. `run --both` also compares checked interpretation; supply finite
stdin as described in the [CLI guide](../../../application/cli/README.md).
The local lock retains this project's exact source/compiler acceptance, not a
portable fixture oracle. Source or compiler changes require fresh review, not
copying bindings or deleting a stale lock to turn it into an unlocked request.

The source-backed target/provider custody check runs with the existing SDK suite:
`mbx nextest run -p compiler --test package_review historical_constant_closure_requires_exact_target_and_provider`.
It checks canonical transport, exact target/provider inputs and Windows Terminal execution without running a foreign native image.

## Shared multi-target pipeline observations

The standalone measurement process isolates invocation counts from parallel
corpus traffic. It uses the ordinary reviewed fixture inputs and source-to-native
compilation entrance. For the byte-conversion fact's named target implementation:

```bash
OMEGA_PHASE_PROFILE=1 mbx run -p compiler --example terminal_sharing -- \
  --assert-native-stage-counts --compare-single-targets --run \
  tests/fixtures/pass/facts/unsigned_byte_to_signed_case_domain_through_target_body_exit/main.omg \
  linux_x86_64 windows_x86_64
```

Each target must have one completed application invocation, with every Omega
00–09 entry count equal to one and one fragment/image emission attempt. Each
image must equal its independently compiled single-target image. The host
product executes natively and through a serialized/reloaded Terminal artifact.
The shared request also requires one source completion, one Build execution,
one Terminal production, and the same Terminal allocation for both targets.
Its `checks=2` is the existing request counter, not a numbered Psi-stage count.
Full numbered source/Psi invocation acceptance remains open.

The command prints `native_pipeline_evidence` and retains the canonical
`program.oterm`, invocation records, each `<target>-shared.image`, and each
`<target>-single.image` in that process's directory. Counter collection is
optional observation data and never an input to checking, admission or reuse.
Failed invocations retain their entered prefix. Independent callable body
pipelines are visible as separate units; the fixture refuses to silently count
those as one application pipeline.

The existing target-local refusal control uses the same entrance:

```bash
OMEGA_PHASE_PROFILE=1 mbx run -p compiler --example terminal_sharing -- \
  --assert-native-stage-counts --compare-single-targets --run \
  --reject macos_arm64 --reason 'behavior exclusion violated: crash cause Trap' \
  tests/fixtures/fail/targets/distinct_roots_exclusion_selected_body/main.omg \
  linux_x86_64 macos_arm64
```

The Linux product still completes its full pipeline, agrees with independent
compilation, and runs in both engines. macOS retains its intended rejection;
this particular control rejects before entering native production, so its native
invocation count is zero.
