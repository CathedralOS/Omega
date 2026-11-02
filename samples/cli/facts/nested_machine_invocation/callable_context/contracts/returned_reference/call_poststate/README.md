# Returned-view call post-state

Owner: [NESTED-MACHINE-UNIFICATION](../../../../../../../../TASKS.md).

An exact returned mutable view observes its current post-call value while earlier scalar observations stay unchanged.

This is another producer in the existing returned-reference fact: an ordinary call publishes the updated contents rather than a direct store. The [frozen original](main.omg) saves five, calls a writer promising nine, subtracts nine from the live alias without a caller equality guard, and traps unless the saved value is still five and both the subtraction and physical storage are correct.

The Rust reference at `ab9b41fc728b` (optimized CLI `77d820a96c305cbcf0b1ff0709f6537530fba21052dfc438cd716d3af4a3e174`) wrongly rejects that subtraction. The native Omega-written checker from `1572c6217fd7` accepts the same frozen source. These identify different producers; the product observation does not claim fresh generation with the newer SDK.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [post-state eight](local/invalid/main.omg) |
| field | [source](field/main.omg) | [post-state eight](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [post-state eight](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [post-state eight](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [post-state eight](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [post-state eight](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [post-state eight](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [post-state eight](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [post-state eight](target_body/invalid/main.omg) |

The match helper explicitly ties its returned view to the value input lifetime; the trace input grants no return origin. An earlier draft omitted that required annotation and its formation refusal is retained separately. The ambiguous-backing control gives both candidates an explicit common lifetime so its only failures are the intended arithmetic obligations.

Every twin changes only the writer's own value and matching public postcondition from nine to eight (the const-name twin changes its named value). It keeps the subtract-nine consumer. The match join executes both alternatives and checks distinct trace values; the target-selected implementations return distinct tags checked by their bound entries. All positive places bind the four hosted targets and declare exit zero.

Establishment comes from each writer's public postcondition under its completed actuals. Exact backing and storage versions own identity/lifetime; the saved scalar is a detached observation, and the changed live cell supplies the new subtraction fact. Calls and joins must preserve that distinction, without granting ownership, access authority, or equality to multiple possible backing cells. Mathematical erasure, borrowed call completion, Terminal operations, calling plans, register/frame homes, encoding and image emission must carry the same program to native/Terminal agreement. These witnesses exercise existing hosted scalar/reference lanes; they do not establish other-host runtime behavior, every ABI boundary, or the parent's full acceptance.

The [indexed-reference producer](controls/indexed_reference/main.omg) and [its twin](controls/indexed_reference/invalid/main.omg) exercise the same live read after projecting an array element. The earlier candidate still refused that positive, as it did the field place. Scalar completion must retain an authenticated read through later descriptor projection.

The [indexed-candidate control](controls/indexed_candidates/main.omg) rejects a claim that writing one runtime-selected pointer establishes nine on both possible backing cells. Its subtraction-free diagnostic counterpart reaches a captured-loan lowering refusal on both the retained pre-repair and earlier repaired Rust producers. That supplies no dynamic-selection runtime acceptance; it remains parent work.

On macOS ARM64, optimized Rust producer `cbd59be7a60836531b9b9ebf5d03ffa39a153436b29508e089ec8eaefc997fa0` from source `8d84fadb84ce` executes the frozen root, all nine places and the indexed-reference producer with native and Terminal exit zero. Their below-nine twins and the snapshot, intervening-write, ambiguous-backing, signed-recast and indexed-candidate controls reject their intended arithmetic obligations. The retained compatibility cohort also passes: the complete run records 20 positive BOTH executions and 26 intended rejects, plus the frozen root. All 92 program/Build input hashes stayed unchanged. Independent source and raw-runtime review accepted this bounded repair.

All nine places and twins, the indexed producer/twin and five soundness controls are pinned as sibling leaf fixtures under `tests/omega/{pass,fail}/facts/nested_machine_invocation/returned_reference_poststate/`; the focused corpus recorded all 25 with their expected outcomes. Exact commands, raw logs, source/binary/profile identities and the retained source bundle are published with PR #1472.

The earlier product checker observation remains source `1572c6217fd7`/image `422e4f58…`: seven positive places accept, while the match helper stops with `CheckCapacityExceeded` and selected-target input stops in typing. Those producer gaps, dynamic captured-loan lowering and the full parent acceptance remain open. No other-host execution is claimed.
