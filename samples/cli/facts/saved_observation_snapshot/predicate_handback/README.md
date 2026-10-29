# Current record and owned-array predicate handback

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, current record-field and owned-array predicate handback. This is a production of the existing saved-observation fact. The original **STATE-LOCAL-VALUE-FRONTIER** parent and broader scalar-range migration remain open.

## Acceptance

Each holder starts at `[7, 1]` with neighbors 11 and 13. Root `preserve` requires and ensures `kept(&holder.items) == true`. It copies the array independently, stores `copied[0] = 4`, then stores the copied value to original element one. The `other_state` and `other_machine` variants terminal-forward `&mut holder.items` to an exact nested or top-level helper taking `original: &mut [u64; 2]`. Those helpers have no Requires, Ensures or rank; the root postcondition must follow from their actual writes through the field reference.

`agree(saved, current)` retains its own static requirement `saved == current && saved == 7` and declares its actual output guarantee `result == observed`. Returning wrappers consume that declared guarantee; they do not replay the helper body. After that call, or its returning wrapper, the runtime oracle explicitly checks current original element zero equals 7, element one equals 4, and neighbors remain 11/13. The nested check uses its current-element argument; the match probe binds the agree result before checking storage. These canaries cannot supply the earlier agree call's proof. The match project executes both arms and observes distinct marker events 11 and 13.

Every twin changes only the final original-array store destination from index 1 to 0. The unchanged root postcondition must reject with `cannot prove ensures contract for exit from preserve`; an unrelated refusal does not qualify. Every positive must run with `omega run --both` and native/Terminal exits 0/0. Four hosted Build entries are authored, not cross-host execution evidence.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [source](../local/predicate_handback/main.omg) | [twin](../local/predicate_handback/invalid/main.omg) | A local saved scalar crosses preserve and is consumed by agree. |
| field | [source](../field/predicate_handback/main.omg) | [twin](../field/predicate_handback/invalid/main.omg) | Note.observed transports the saved scalar. The historical 6a6c6ffa173d source gains an explicit current-element runtime canary and the helper's declared output guarantee. |
| parameter | [source](../parameter/predicate_handback/main.omg) | [twin](../parameter/predicate_handback/invalid/main.omg) | hold receives the saved scalar and original holder as explicit parameters, calls preserve and consumes the unchanged fact. |
| call_result | [source](../call_result/predicate_handback/main.omg) | [twin](../call_result/predicate_handback/invalid/main.omg) | read_kept returns the saved value with its checked result contract before preserve. |
| match_join | [source](../match_join/predicate_handback/main.omg) | [twin](../match_join/predicate_handback/invalid/main.omg) | Both arms capture the saved value, writing distinct marker events 11 and 13. main runs both and checks each marker. |
| const_name | [source](../const_name/predicate_handback/main.omg) | [twin](../const_name/predicate_handback/invalid/main.omg) | MARK names exactly 7 in the original [7, 1] initialization and consumer oracle; kept and preserve retain their literal contracts. |
| other_state | [source](../other_state/predicate_handback/main.omg) | [twin](../other_state/predicate_handback/invalid/main.omg) | preserve terminal-forwards the actual field reference to its nested store_original helper, whose body performs the owned copy and disjoint final store without Ensures. A separate nested check receives the saved/current scalars and consumes agree. |
| other_machine | [source](../other_machine/predicate_handback/main.omg) | [twin](../other_machine/predicate_handback/invalid/main.omg) | preserve terminal-forwards the actual field reference to a separate top-level store_original helper with no Ensures. read_kept still captures, invokes preserve and consumes agree before returning its checked result. |
| target_body | [source](../target_body/predicate_handback/main.omg) | [twin](../target_body/predicate_handback/invalid/main.omg) | A normal named implementation selected by Build captures, preserves and consumes the fact; all four hosted targets select it. |

Positive pins: `tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_<place>_exit/`. Reject pins: `tests/omega/fail/facts/saved_observation_snapshot_predicate_handback_<place>_rejected/`. Source and Build inputs are copied exactly. Positive pins expect native/interpreter exit 0; reject expectations and invalid metadata identify preserve's own Ensures.

## Requirement mapping

- **Establishment:** `[7, 1]` establishes kept. Its checked predicate body, preserve entry/postcondition, and agree admission each owe their ordinary proof; the runtime canary supplies none of them. See [collection invariants](../../../../../wiki/spec/language/dependent_values.md#collection-invariants).
- **Identity and lifetime:** captured scalars and Requires versions stay immutable. Owned copies receive independent homes. Writes update current enclosing-record contents while preserving disjoint cells and neighbors. See [fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md).
- **Composition:** the nine actual transport shapes retain the same fact and operation. Two forwarded variants additionally require exact field-reference binding and actual body handback without assuming a helper Ensures.
- **Soundness:** index-only twins break preservation. Retain all normal-exit obligations, declaration paths, copy/move custody, access authority, and the existing `symbolic_slice_predicate_store_record_snapshot` / `symbolic_slice_predicate_owned_copy_snapshot` reject controls. No pending goal or selected target body supplies portable proof.
- **Psi 05–07:** ordinary copies, record/index accesses, calls and transitions must lower to Terminal Psi and execute with the stated canaries; predicate evidence is erased.
- **Omega 00–02:** ordinary record/array layout and call placement preserve both cells and neighbors. All four hosted entries are authored; target_body selects an ordinary named implementation for each target.
- **Omega 03–06:** native homes must keep copied and original arrays independent and saved scalars stable. The explicit current-zero check detects copy aliasing or a clobbered original zero even when agree returns the old saved 7; element one and neighbor checks detect omitted or collateral writes.
- **Omega 07–09 and emission:** emitted native execution must agree with Terminal execution. Match markers discriminate both selected arms. Only observed producer/target results establish execution acceptance.

## Baseline chronology and remaining qualification

Fixed published base: `5134dc49b62ecfc216a0458094b6c8f6d6dd473a`. Historical field commit: `6a6c6ffa173d`. Its original positive/twin bytes are archived in `build/scalar-storage-versions-20261029/historical-field-6a6c6ffa173d/`, with hashes and links to the unchanged original baseline records in `snapshot.json`.

The original field positive and twin rejected preserve's own Ensures on optimized R10 producer `ff7376f75f466e8b605149a58ede7e793bac852b`; the positive had one diagnostic and neither source had formation/access failures. The owner subsequently reported original-field check 0 and native/Terminal 0/0 on optimized R1 b140, with record_bad/copy_bad still rejecting their own Ensures.

All nine sources and pins now add the explicit current-element runtime canary and agree's accurate output guarantee. The field pair is therefore no longer byte-identical to 6a6c6ffa173d. Its fact, bounds, copy/store operation, preserve contract and agree requirement are unchanged. A fresh optimized R10 replay of all nine final positive sources still rejects preserve's own postcondition; the source hashes and raw diagnostics are retained in `build/scalar-storage-versions-20261029/final-source-baseline-observations.json`. The helper contract correction is fixture authoring, not a compiler repair. Candidate qualification of the strengthened positives, twins and pins remains pending. Historical observations do not qualify these new bytes.

Final producer/target results belong in the owning qualification evidence. Neither this sidecar nor a filled source table closes the full row or either parent.
