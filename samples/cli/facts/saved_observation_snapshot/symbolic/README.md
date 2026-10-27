# Saved symbolic observations through selections

A saved reference observation remains equal to the supplied value across a disjoint element write.

Parent: `STATE-LOCAL-VALUE-FRONTIER` in [TASKS.md](../../../../../TASKS.md).
The remaining requirement discharged here is symbolic numeric predicate transport
through selected-value joins, including return guarantees. Each probe receives an
unknown `value`; a selected call writes it to an array element and guarantees
`source == value`. The owned receiving read saves the post-call value. After a
neighbor write, `same` requires the saved equality before any runtime guard.
Each reject changes only the false-arm supplied value to 3.

| Place | Program | Reject twin |
| --- | --- | --- |
| local | [positive](local/main.omg) | [reject](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [reject](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| call result | [positive](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| match join | [positive](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| const name | [positive](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| other state | [positive](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| other machine | [positive](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| target body | [positive](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

`main` invokes both arms with 7 and 9. A caller-visible toggle exposes an omitted
or doubled helper invocation; distinct event tags expose the wrong arm. Array
and record neighbor canaries expose displaced writes. Four hosted targets are
bound; the observations below establish execution on macOS ARM64 only.
Matching fixtures are `tests/omega/{pass,fail}/facts/saved_symbolic_observation_*`.

## Requirements and evidence

- **Establishment:** the completed helper contract establishes the relation for
  arbitrary inputs. No caller guard or replay of the callee establishes it.
- **Identity and lifetime:** the scalar keeps its detached read epoch. Descriptor
  identity is checked independently of referent contents. The disjoint write
  preserves the saved value; stale expected-value controls reject.
- **Composition:** each candidate retains its own selection condition and source
  predicates. A conclusion must cover every candidate. Calls, selected records,
  explicit nested-machine parameters and terminal returns carry the same fact.
- **Soundness boundaries:** [correlated pairs](controls/correlated_pair/main.omg)
  use different input values in the two arms; their
  [mismatched twin](controls/correlated_pair_invalid/invalid/main.omg) and
  [independent choices](controls/uncorrelated_choices/invalid/main.omg) reject.
  [Reversed equality](controls/reverse_guarantee/main.omg) works through a
  differently named helper. [Stale expected values](controls/stale_expected/invalid/main.omg),
  [write-only reads](controls/unreadable_delivery/invalid/main.omg),
  [wrong scalar types](controls/wrong_primitive_delivery/invalid/main.omg), and
  [wrong reference types](controls/wrong_reference_delivery/invalid/main.omg) reject.
- **Checked-to-Terminal Psi (`Psi 05–07`):** ordinary calls, primitive receiving
  reads, match joins, explicit loans and nested terminal transfers execute in
  Terminal Psi. This row does not qualify affine cleanup, proof tags, `dyn`
  dispatch or general crash-route completeness.
- **Provider settlement, layout and calling plans (`Omega 00–02`):** normal Build
  entries, array/record geometry and explicit scalar/reference arguments realize
  through the standard plans. Foreign boundaries and stack-argument thresholds
  are outside this fact.
- **Instructions, homes and frames (`Omega 03–06`):** native ARM64 execution
  checks calls, scalar loads, indexed writes, live saved values and canaries.
  This does not qualify other ISAs or all activation-bound calculations.
- **Encoding and images (`Omega 07–09`):** emitted native images agree with
  Terminal execution on this host. Foreign-host execution and encoding-boundary
  qualification remain separate.

## Recorded outcomes and remaining scope

On fixed base `040c210f8b482bdc14c0e2ebd597f101d9f56696`, all nine
positives refuse; the direct call-result place refuses earlier at primitive
terminal delivery, and the others refuse their consuming or return contract.
At implementation source `1b7ceabd8b7`, all nine run in both engines with exit 0,
and all nine reject twins reach their equality obligations. Both positive
controls run in both engines; all six additional invalid controls reject.
The optimized producer reports opt-level 1. Exact source hashes, binary digest,
commands and raw diagnostics are retained in the PR evidence.

The parent remains open for generic body-checking composition, byte predicates,
single-evaluation pattern destructuring and early exact-arithmetic formation.
This row proves the selected symbolic equality mechanism; it does not close all
numeric implication or ownership requirements.
