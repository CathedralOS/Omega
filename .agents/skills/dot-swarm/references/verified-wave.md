# Verified diagnostic wave

## Scope and outcome

On 2026-09-30, a coordinator dispatched two native collaboration workers against
Omega revision `18d68e27ec1620c945d36ae74fa8e6902ee207d5` on Linux x86-64. Both
read the draft dot-swarm skill, acknowledged their assignments, and returned
independent, source-backed findings and check evidence. This was an actual
read-only diagnostic wave, not a READY/capacity probe. No compiler implementation,
board changes, shared-claim mutations, remote publication, or production landing
were part of the wave.

The coordinator checked the shared claims registry before selection.
`SAMPLE-CORPUS` was already assigned, so the diagnostic slots used unclaimed
`CANARY-CORPUS` and `FLOAT-PROVIDERS` subjects. Workers shared a pinned read-only
checkout and wrote separate scratch outputs. The coordinator retained an
in-flight ledger, reconciled native handles, received partial findings, and drained
both results. The original checkout remained clean on its original main revision.

## Useful findings

### CANARY-CORPUS: execution oracles

The worker inspected three existing customers and the maintained corpus harness:

- `tests/omega/pass/host/runtime_console_bounded_line_exit` binds `Main::main`
  only. Without input/output oracle files, the historical native record covers
  the empty-input exit, not the authored LF, Full, or `Main::repeat` window
  cases. The program emits raw untouched bytes (`0xA5`), while the harness's
  text decoding cannot establish an exact raw-byte output oracle.
- `tests/omega/pass/filesystem/native_wrapper_write_all_result` has an entry
  binding but neither an `_exit` name nor an explicit native-exit file. The
  harness would build it without executing it even after its compiler/provider
  blockers were fixed.
- `tests/omega/pass/terminal_psi/content_custody_exit` has no `build.omg` entry
  binding, so it remains declaration checking. Separate constructed-input
  lowering/codec tests do not prove this fixture's source-to-native execution.

A focused synthetic probe also exercised the real Python corpus gate with a
mocked runner and a temporary golden. Ordinary comparison classified
`stdout:match` becoming `stdout:differs` as a regression, but `--record` accepted
`stdout:differs` when the explicit exit still matched. This is a recorded-output
safety gap, not a fresh native compiler failure. No repository golden was changed.

The next acceptance is explicit executable input/output/exit coverage for each
promised route, including byte-safe console output checking. The board item stays
open; no acceptance was declared satisfied by this wave.

### FLOAT-PROVIDERS: source-free proof obligations

The worker confirmed that the current source still restricts FloatMeaning
projection binding and independent Terminal replay to Meaning results and
Format/Meaning operands. Boolean/integer results and integer operands need their
proper proof carriers; they cannot be placed into the Meaning carrier.

Relevant source owners are:

- `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/proof/float_meaning.rs`
- `omega-rust/psi/compilation/checking/src/proof_contracts/float_projection_bindings/semantic_values.rs`
- `omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/float_meaning_projection.rs`

The existing customer
`tests/omega/pass/float/runtime_named_float_to_integer_conversion_exit` calls a
real I32 conversion slot and has authored native exit 70. Its historical success
is not proof that the slot's `to_i32` postcondition was retained and discharged in
a produced source-free artifact. The worker also distinguished roster-custody
codec tests with no obligations from actual contract-proof acceptance.

The next acceptance is this existing customer's actual slot contract carried
through producer, codec, and independent replay, with false-result and changed-
operand rejection controls and unchanged native exit 70. The board item stays
open. No Rust tests or compiler execution were claimed from source inspection.

## Checks actually performed

- Skill creator's bundled `quick_validate.py` against the final skill: valid
- Relative Markdown references: all 8 resolved in the Omega checkout
- `python3 tools/tests/test_swarm.py -v`: 39 tests passed, exit 0; existing
  helper tests use temporary repositories and mocked network calls
- `python3 -m unittest tools.tests.test_corpus_gate tools.tests.test_corpus_records -v`:
  29 tests passed, exit 0
- Isolated synthetic corpus-output recording probe: expected behavior observed,
  exit 0; mocked runner, temporary fixtures/golden, no Omega execution
- FLOAT-PROVIDERS source audit: 8 explicit source/evidence checks satisfied;
  these are source inspections, not compiler or Rust-test results
- `python3 tools/swarm/worktree_status.py --offline --base 18d68e27ec1620c945d36ae74fa8e6902ee207d5`:
  exit 0
- `python3 tools/corpus_scoreboard.py --base 18d68e27ec1620c945d36ae74fa8e6902ee207d5 --json`:
  exit 0; recorded authored-run delta 0, executed delta 0, no gained/lost rows.
  This reads existing golden evidence; it performs no new compiler execution
- Independent adaptation review checked native lifecycle, global capacity,
  local-only scope, claims, and landing policy. It prompted explicit receipt
  regeneration after rebase and a portable Python 3 command convention

Toolchain availability was verified as `mbx 1.21.0`, cargo-nextest `0.9.146`,
and pinned `nightly-2026-09-04`. No built Omega CLI or corpus runner was available
in this checkout, and this bounded diagnostic wave did not build one.

## What this verifies, and what it does not

Verified: two-worker native dispatch, assignment acknowledgments, concurrent
useful investigation, intermediate messages, handle reconciliation, final result
draining, scope preservation, and honest evidence reporting. The seven-agent
limit is the runtime-declared global budget; this two-worker wave is not itself
a saturation test.

Not exercised: compiler edits, isolated concurrent editing worktrees, claim
acquisition/renewal, landing reservations, publication, worker-failure recovery,
long-build coordination, or indefinite backfill. The skill documents those paths
from the repository's existing contracts; this record does not claim they passed
an end-to-end implementation wave. No newly running program or compiler fix was
produced. The diagnostic wave's success is limited to its stated scope.
