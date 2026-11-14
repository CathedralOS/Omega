# Match computed root

A match subject is one evaluation of a computed root, the selected exclusive
loan records that root, and the consumer writes 41 through that recorded loan.

The counter starts at 0. `compute` increments it and returns index 0; the match
selects `&mut chosen[0]` when that element is 17. The consumer writes 41 through
the selected loan. The runtime oracle checks the written value, `chosen[1] == 5`,
`other[0] == 0` and `visits == 1`. The reject twin overwrites `chosen` while
`selected` is live and must report ``while local borrow `selected` is still active``.

The original programs assigned `counter = 1`, so running the computation twice
could still leave the count at 1. Their passing runtime results do not prove single
evaluation. The revised programs use `counter = counter + 1`; acceptance also
requires inspecting the emitted Terminal and native effects to confirm that the
counter update remains observable rather than folding to a constant assignment.

## Current evidence

The unchanged-source [baseline run 42156413196](https://github.com/CathedralOS/Omega/actions/runs/42156413196)
used optimized Linux ARM64 producer `47307200e73621ba104defb0300cc6c2447dc6ce`
with run configuration `2d38b4a7cf42118d17d1be10e32b8985f2fa57ee`. Eight ordinary places
checked and exited 0 on both engines. All nine twins reached the expected active-borrow diagnostic.
The target place refused in each old target-prefixed body because it did not
establish `visits == 0` for `compute`; ordinary package admission also refused.
That is an incomplete input contract in the retired source form, not evidence of
a new compiler defect. The earlier four private-call checking regressions are
already repaired by the retained producer.

The unchanged revised programs were also checked against published main
`0b53c8a4e109e8d6ef3a9bbabc29c52529317a48` in
[run 42166308906](https://github.com/CathedralOS/Omega/actions/runs/42166308906).
Eight positives checked and all nine twins reached their original expected
active-borrow diagnostics. Only the migrated target place refused: the checker
could not carry the provider's required `visits == 0` condition into the ordinary
`apply` helper. This checking gap needs the general private-call arrival repair;
the public contracts and authored programs remain unchanged. That run checked
Source only and supplies no runtime result.

The revised programs pass on all four hosts using compiler `d32f1cbab409`,
integrated with fixed main `c417f61c95b`:
[Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/42220565700) and
[Linux ARM64, macOS ARM64 and Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/42227161874).
Each host checks all 17 positive fixtures and 30 invalid controls, runs 17
native/Terminal exit-zero comparisons, and passes all nine sample places and
their nine invalid twins. Both ordinary CLI projects also pass. Every rejection
matches its original diagnostic. Saved Terminal instructions and native code
retain the counter read, increment, write and single call to `compute` in all
nine places. The callback control retains its original snapshot assertions and
an observable counter change from zero to one.

Compiler `edd05427fd02` maintains invocation lookup in the callable-contract
table. It preserves invocation generations,
row order, stored bytes and recovery limits. Its
[focused Linux run](https://github.com/CathedralOS/Omega/actions/runs/42239810895)
passes the same 47 source checks, the target-place and callback execution
comparisons, both ordinary CLI projects, seven table tests and the existing
contract-record test. All 17 generated Terminal programs have byte-identical
semantic sections compared with `d32f1cbab409`. The unchanged programs, library,
interpreter and native backend make the earlier runtime evidence applicable;
those observations retain their original compiler and host identities. Fixture
limits remain 120 seconds, with one worker and profiling disabled.

Execution and counter inspection cover this row. The broader Build-selection
migration stays open.

## Places

There is no separate root program. Every positive binds `ProgramEntry` for all
four hosted targets and declares `Expected exit: 0`.

| Place | Positive | Reject twin | How the root arrives |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [invalid](invalid_local/main.omg) | the match directly evaluates the computed index |
| field | [main](field/main.omg) | [invalid](invalid_field/main.omg) | `compute` returns the copied record's index |
| parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) | `apply` receives both arrays and the counter |
| call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) | `select` returns the selected exclusive loan |
| match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) | an earlier match returns the computed index |
| const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) | `compute` returns the named index constant |
| nested machine | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) | a nested `write` receives the explicit references |
| another machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) | `forward` passes the references to `apply` |
| target body | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) | Build selects an ordinary provider that calls `apply` |

The existing corpus mirrors are `tests/omega/{pass,fail}/facts/match_computed_root_through_*`.
Their main files match the corresponding sample. Every twin retains the same
loan diagnostic. The target fixture also asks the CLI to show the compiled `main` program and
read its saved form again, so the emitted counter effect can be checked.

## Requirements and remaining acceptance

This row advances `BUILD-OWNED-TARGET-SELECTION` in [TASKS.md](../../../../TASKS.md).
The four target providers share one behavior and forward explicit arguments to
one ordinary `apply`; they do not establish distinct operating-system behavior.
The public `ComputedRootWrite::write` requirement and every provider member
require `visits == 0`, which supplies `compute`'s required input condition. Build
selects a named implementation for each target, as required by
[provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

- **Establishment:** `compute` requires `counter == 0`, so adding 1 fits in `u64`.
  Every actual caller must establish that condition. No post-hoc caller guard supplies it. See
  [invocation contracts](../../../../wiki/spec/language/state_contracts.md) and
  [numeric operations](../../../../wiki/spec/language/numeric_values.md).
- **Identity and lifetime:** the evaluated subject and selected reference retain
  their exact storage identities. The counter update cannot retarget a captured
  loan. Writes through `selected` reach the original array element; overlapping
  writes while that loan is live reject. See
  [loans](../../../../wiki/spec/terminal-psi/loans.md) and
  [structural access](../../../../wiki/spec/terminal-psi/structural_access.md).
- **Composition and authority:** ordinary and nested calls substitute explicit
  arguments, while public provider contracts admit independent inputs. Matching
  neither creates exclusive authority nor discards possible loan origins. The
  selected descriptor must survive the call-result and forwarding places.
- **Lowering and execution:** each subject and its effects execute once, followed
  by only the selected arm. Terminal and native code must retain the increment,
  exact selected address, write 41 and loan restoration. The final oracles must
  agree on both engines; inspection must confirm an observable counter update.
  See [patterns](../../../../wiki/spec/language/patterns.md),
  [evaluation order](../../../../wiki/spec/language/expressions.md#evaluation-schedule)
  and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).

A filled row establishes this computed-subject and exclusive-loan behavior. It
does not close the parent task's other target migrations or borrowed-join work.
