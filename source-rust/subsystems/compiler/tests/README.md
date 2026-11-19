# Where compiler tests go

A compiler test is a Rust test in this directory, declared as a `[[test]]` in
the crate's `Cargo.toml`. Prove a change with a test on the changed code, then
a very small area around it (for the lowered-form checker, the
`lowered_route_*` tests here).

Keep compiler-owned programs in [fixtures/](fixtures/), grouped by feature, then `pass`, `fail` or `run`, then case name. A failing case pins its diagnostic in `expected.txt`. A runtime case pins the exit or output read by its test. Keep a program used by several crates in the [shared fixtures](../../../../tests/fixtures/). Resolve package locations from the authored `build.omg`.

Nothing runs every fixture and nothing records outcomes. The owner's ruling
of 2026-11-13 removed the fixture corpus in `tests/omega/`, its runner
(`corpus_runner.rs`) and its ledger;
[tests/README.md](../../../../_recycling/tests/README.md) says how to read a deleted
fixture from history. A file in a kept fixture that only the runner read (for
example `expected_terminal_inspections.txt`) is now a note about the program,
not a checked expected result.

## Running a fixture

From the repository root, run the Rust test which names the fixture, for example `mbx test -p compiler --test byte_view_arrivals`. Its harness prepares the package inputs, selects the target and checks the observation. A direct CLI invocation follows the [CLI package review workflow](../../../application/README.md).
