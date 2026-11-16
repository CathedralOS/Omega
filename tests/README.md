# Repository tests

This tree has one directory per language, plus shared test support.

- `alpha/` contains Alpha conformance and reference tests.
- `beta/` contains trusted Beta compiler reconstruction and differential tests.
- `gamma/` contains Gamma evaluator and compiler-customer gates.
- `delta/` contains candidate-language experiments, not the canonical Delta definition.
- `epsilon/` contains Epsilon checking, evaluation and runtime gates.
- `bootstrap/` contains tests spanning more than one bootstrap rung.
- `fixtures/` holds Omega programs explicitly read by Rust tests.
- `psi/` contains black-box gates for the Omega-written Psi source.
- `product/` holds `gate.sh`, shared by gates of the Omega-written compiler.

Single-crate Rust tests live in that crate's `tests/` directory or `#[cfg(test)]`
modules. Ordinary changes test the mechanism and a small surrounding area under
[the sample loop](../AGENTS.md#the-sample-loop). Release-wide commands belong in
[the completion plan](../wiki/drafts/reference/rust_compiler_completion.md), not
in every pull request.

## Fixture outcomes

A positive program must reach its specified check or execution result. A negative
program must reach the diagnostic for the obligation it tests. An unrelated
parser error or unsupported construct is not evidence that the intended safety
check works. Preserve the program's operation, contract and expected outcome
while repairing the implementation.

Fixtures are selected by their owning tests; no runner needs to walk the entire
fixture tree. Preserve relative Build inputs when moving one. Keep the focused
witnesses required by those tests and current tasks under the repository's
`.omg` file limits.

## Build-host fixtures

A standalone failing Build fixture may include `build_resource_limits.txt`
with four positive integers: evaluator work, staging entries, total logical
bytes and largest object size. The harness supplies those host budgets and
fresh private staging. Omega source cannot grant itself a budget.

With that policy, `build_output_alias.txt` may name two relative files. The
harness creates a file and a hard link in private output staging so the Build
exercises aliased-output rejection. Default Build has no link operation. These
files configure the test host; the shipped CLI does not read them.

## The former corpus

`tests/omega/`, its runner and outcome ledgers are retired. Do not recreate them
or add files there. Recover a useful program into `tests/fixtures/` only when an
owning test needs it. Coverage comes from those tests and maintained samples,
not old pass stamps.

Locate a removed file with `git log --all -- <path>`, then read it from the
removing commit's parent with `git show <parent>:<path>`. Keep the needed fixture
and its dependencies beside the owning tests. Historical inventories and
migration chronology belong in Git, not the current test instructions.
