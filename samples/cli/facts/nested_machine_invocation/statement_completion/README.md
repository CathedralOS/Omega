# Ordinary statement completion

Parent: [NESTED-MACHINE-UNIFICATION](../../../../../TASKS.md).
Requirement: an executed nested helper preserves its normal-return facts across
an ordinary namespaced Unit call; theorem citation eligibility cannot erase its
completion frontier. Selection retains the exact public callable contract.

Every place copies this fact:
“Every completed helper retains result != -1 across an ordinary Unit call, so accept can consume the result.”

The helper emits one Console byte, then returns the candidate only when it is
neither zero nor -1; otherwise it returns zero. `accept` consumes the guarantee
directly. Four invocations must print `ABCD` and return 7, 0, 0, and -2.
Omitting the Unit effects loses the output; selecting a wrong arm traps.
The negative handle -2 checks that opaque handle representations are not
mistaken for errors solely because their sign bit is set.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| selected hosted body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Every twin changes only the helper's exclusion from -1 to -2. Its unchanged
result contract must reject because -1 can now escape. The ninth place uses an
ordinary named implementation selected by Build, with entries for all four
hosted targets; it adds no retired target-prefixed declaration syntax.

Additional controls retain [a load-bearing namespaced theorem](controls/namespaced_theorem/main.omg),
[its missing premise](controls/missing_theorem_premise/main.omg),
[its missing citation](controls/missing_theorem_citation/main.omg),
[a namespaced tag contract](controls/namespaced_tag/main.omg),
[a wrong tag](controls/wrong_tag/main.omg),
[an invalidated storage guard](controls/overwritten_guard/main.omg), and
[a stronger private guarantee unavailable through the public requirement](controls/private_guarantee/main.omg).
These are pinned beside the row under `tests/omega/{pass,fail}/facts/nested_machine_statement_completion_*`.

The accepted meaning comes from [invocation contracts](../../../../../wiki/spec/language/state_contracts.md)
and [proof contracts](../../../../../wiki/spec/proofs/contracts.md).

| Obligation | Evidence supplied by this fact |
| --- | --- |
| Establishment | The helper's own guard establishes the exact returned scalar's exclusion; no caller equality guard supplies the proof. |
| Identity and lifetime | Explicit argument snapshots survive the Unit call; overwritten-storage and private-contract controls reject. No new borrowing or cleanup authority is introduced. |
| Composition | Nested/ordinary invocations retain all normal arms, explicit actuals and their own contracts. The theorem control separately requires its exact premises. |
| Soundness boundaries | Runtime calls preserve effects and only eligible tag facts enter citation intake; erased theorem equations retain classification, formation and induction checks. |
| Psi 05–07 | `run --both` compares Console effects and the authored exit through Terminal Psi and native execution. Proof-only theorem values remain erased. |
| Omega 00–02 | Build selects the named hosted implementation, entry contract and Console provider; ordinary scalar call/return placements carry the same values. No new layout or ABI lane is claimed. |
| Omega 03–06 | Distinct returned values and observable ordered calls exercise live values across calls and activation storage on macOS ARM64. No new stack bound or ISA operation is claimed. |
| Omega 07–09 | The emitted macOS image must print the same bytes and agree with Terminal execution. Windows/Linux native execution remains unverified on this host. |

Baseline: fixed fetched main `f79ca742d1c024ccc65ef6359bbb81a77484604b`,
optimized pinned-nightly producer. All nine correctly formed positives refused
their `result != -1` guarantee before the repair. The namespaced theorem also
refused its structural claim. Baseline source/oracle hashes and raw observations
are retained in the implementation qualification record.

Acceptance command: `omega run --both <place>/main.omg`, after normal project
acceptance. Runtime and scoped corpus observations are recorded with the source
producer and published PR; source checking of the real Windows helper bodies is
separate from unavailable Windows native execution.

The parent outcome remains open for its other normal-return/cycle obligations,
product frontend migration, and broader custody/WCSU acceptance. The separate
borrowed representation-view composition refusal is not claimed by this row.
