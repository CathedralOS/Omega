# Where compiler tests go

Exercise the public compiler API here. Cargo discovers the Rust test files at this directory's root. Tests for individual stages live with their stage. Project planning and Build execution tests live in [build/tests](../build/tests/). The archived orchestration tests still list cases to port to these owners.

Keep compiler-owned programs in [fixtures/](fixtures/), grouped by feature, then `pass`, `fail` or `run`, then case name. A failing case pins its diagnostic in `expected.txt`. A runtime case pins the exit or output read by its test. Keep a program used by several crates in the [shared fixtures](../../../../tests/fixtures/). Resolve package locations from the authored `build.omg`.

Nothing runs every fixture and nothing records outcomes. The owner's ruling
of 2026-11-13 removed the fixture corpus in `tests/omega/`, its runner
(`corpus_runner.rs`) and its ledger;
[tests/README.md](../../../../_recycling/tests/README.md) says how to read a deleted
fixture from history. A file in a kept fixture that only the runner read (for
example `expected_terminal_inspections.txt`) is now a note about the program,
not a checked expected result.

## Running a fixture

From the repository root, run the Rust test which names the fixture, for example `mbx test -p compiler --test final_stage_validators`. This harness opens a saved Psi artifact and checks target-specific validation through the compiler API. A direct CLI invocation follows the [CLI package review workflow](../../../application/README.md).

Run the provider selection checks in the package whose behavior changed.

- `mbx test -p source-compiler --lib provider_choices::tests` checks which provider authored declarations select and which declarations produce a diagnostic.
- `mbx test -p admitted-native-inputs-to-target-bound-program --test source_provider_choices` checks the native plans for those selections.

For each published program, the source tests check that its ID matches the one the native planner reads from the saved Psi file. [Shared provider programs](../../../../tests/fixtures/providers/pass/source_provider_choices/)

To update the saved programs after an intentional change, set `OMEGA_PROVIDER_TEST_OUTPUT` to an existing temporary directory and run the source tests. They write fresh publications with a `.new.psi` suffix and check the committed `.psi` files. Copy each reviewed update over its corresponding fixture, then run both checks.
