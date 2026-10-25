# Correlated invariant store

amount <= 320 - counter.total makes exact counter.total + amount preserve counter.total <= 320.

The receiving record declares `where total <= 320`. Independent operand
intervals lose the coupling: each operand can reach 320, but their sum cannot
exceed 320 on the same execution. The store must consume that live relation,
not assume its own receiving invariant or insert a runtime guard.

## Places

Each folder contains an executable `main.omg`, a four-hosted-target
`build.omg`, an exit-0 oracle and an `invalid/main.omg` twin.

| Place | Transport into the addition |
| --- | --- |
| local | A scalar local retains the incoming amount's identity. |
| field | An ordinary record field carries the amount. |
| parameter | The receiving machine consumes its parameter directly. |
| call_result | A call's declared result equality carries the amount. |
| match_join | Either the offered amount or all remaining capacity reaches the store; both branches run. |
| const_name | A named capacity supplies the same bound. |
| other_state | A successor receives the coupled values without restating its own precondition. |
| other_machine | An ordinary call forwards the coupled values. |
| target_body | Four separately checked target bodies perform the same store. |

Run any positive with `omega run --both <place>/main.omg`.
The oracle checks an interior sum (30 + 40), the upper endpoint (300 + 20),
zero increment at capacity and a full-capacity increment from zero.
The match additionally checks its distinct alternative at total 30.
Each twin changes only the admitted increment's ceiling from 320 to 321:
the intermediate addition still fits u64, but the receiving invariant does not.
It must reject with “not provably within its data's where facts”.
The match and its twin retain an independent `amount <= 1000` formation
bound. It is redundant in the valid program and insufficient for the store;
it keeps the invalid join's arithmetic representable so the twin reaches
the receiving-invariant obligation rather than an earlier overflow refusal.

The row is pinned under
`tests/omega/{pass,fail}/facts/correlated_invariant_store_through_<place>_*`.
Additional `ranges/correlated_store_*` controls cover writes in destination
selectors and right operands, an insufficient predecessor at a state join,
and an unsafe intermediate. A positive saved-scalar witness checks that a
later storage mutation does not retroactively change captured values.

## Contract and evidence

This repairs a prerequisite of **FILESYSTEM-PLATFORM-SERVICES**: the unchanged
`runtime_posix_read_runtime_range_view_exit` fixture accumulates short reads
under the same remaining-capacity relation. The source refused at
`read_advance` on published base `705856dc542f`; all nine new places also
refused their invariant store with that exact baseline producer.

The governing contracts are
[dependent values](../../../../wiki/spec/language/dependent_values.md#mutable-arguments-and-declared-field-domains),
[numeric values](../../../../wiki/spec/language/numeric_values.md#arithmetic-and-comparisons)
and [invocation contracts](../../../../wiki/spec/language/state_contracts.md).

1. Establishment: the incoming contract proves the increment fits the remaining capacity.
2. Identity and lifetime: scalar snapshots survive; mutation retires overlapping storage relations.
3. Composition: the same 320 bound reaches the consuming store across all nine places.
4. Soundness: ceiling-321 twins, effect controls, joins and intermediate formation must refuse.
5. Psi lowering: the accepted store and exact addition execute in Terminal Psi; no runtime proof replaces static admission.
6. Omega planning: ordinary ProgramEntry settlement, record layout and calling plans carry the values.
7. Realization: scalar addition, record writeback, calls and state transfers exercise native register/frame realization.
8. Emission: native exit 0 must agree with Terminal exit 0 and every authored assertion.

Runtime observations identify their actual host and producer in qualification
records. Four target bindings and checked target bodies do not establish
execution on Windows or Linux. This row does not finish the full Windows
filesystem provider or its matching-host acceptance.
