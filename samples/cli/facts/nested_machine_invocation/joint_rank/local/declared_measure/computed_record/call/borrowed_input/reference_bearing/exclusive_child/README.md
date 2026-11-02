# Exclusive child custody in declared rank calls

Owner: **NESTED-MACHINE-UNIFICATION** in [TASKS.md](../../../../../../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

This extends the [existing shared-descriptor production](../README.md) and the [published affine-input production](../../affine_input/README.md) inside the same fact. The initialized-home lending support is already published by [PR #1360](https://github.com/CathedralOS/Omega/pull/1360); this production exposes the additional call-loan join for conditional operands. `Progress` now owns an exclusive `&mut u64` child and is not Copy. Its declared measure lends the complete initialized record to the pure, total `distance(&value)` call. That short shared loan neither copies the exclusive descriptor nor grants access to its referent. The ordinary state body still owns and transfers the original record.

Every place runs seven increments (counter 3→10) and the empty path (counter remains 10). The completion checks endpoint equality, record canary 641 and guard 827 through the exclusive descriptor. The ordinary caller then reads its original guard after the loan ends. A wrong branch, omitted increment, clobbered home or lost backing violates the exit-0 oracle. Each twin changes only `pending.lower + 1` to `pending.lower + 0`.

| Place | Positive | Nonprogress twin |
| --- | --- | --- |
| local | [source](main.omg) | [source](invalid/main.omg) |
| field | [source](../../../../../../../field/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../field/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| parameter | [source](../../../../../../../parameter/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../parameter/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| call_result | [source](../../../../../../../call_result/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../call_result/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| match_join | [source](../../../../../../../match_join/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../match_join/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| const_name | [source](../../../../../../../const_name/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../const_name/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| other_state | [source](../../../../../../../other_state/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../other_state/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| other_machine | [source](../../../../../../../other_machine/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../other_machine/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |
| target_body | [source](../../../../../../../target_body/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/main.omg) | [source](../../../../../../../target_body/declared_measure/computed_record/call/borrowed_input/reference_bearing/exclusive_child/invalid/main.omg) |

[Two simultaneous shared arguments](controls/two_shared_arguments/main.omg) lend the same noncopy input twice without duplicating its exclusive child. [An owned Copy child](controls/owned_copy_child/main.omg) supplies only its plain endpoint record to an owned formal while retaining the containing record's exclusive descriptor. Copy permission belongs to that selected operand.

[Mutation through the shared input](controls/shared_mutation/main.omg) rejects its assignment in `distance`. The [false-Copy control](controls/false_copy/main.omg) rejects marking the exclusive descriptor carrier Copy. The [overlapping sibling](controls/overlapping_sibling/main.omg) keeps a real exclusive conflict between a selected arm and a separate argument. [Independent match operands](controls/independent_match_siblings/main.omg) also reject an overlap: opposite arm numbers of different decisions do not establish exclusion. The nonprogress twins retain the independent every-cycle descent obligation.

## Requirements

- Establishment: the declaration application retains a complete initialized owned input independently of its scalar read roster. The selected call independently establishes ordered endpoints before using its result equation.
- Identity and lifetime: the short shared call borrows the original entry home. The exclusive descriptor stays in the original carrier, and the caller checks its backing after completion. Scalar proof reads cannot manufacture storage or loan authority.
- Composition: every private participant transports the same exact distance and counter fact. Two shared operands may overlap. An owned Copy child remains reusable even when another child is exclusive. The simultaneous-call judgment consumes the existing evaluated occurrence routes: opposite arms of one decision cannot coexist, while every compatible sibling pair keeps its ordinary loan check. Missing route evidence grants no exclusion.
- Soundness: call purity, totality, exact input identity, shared access and owned-copy permission remain separate obligations. Shared access cannot write; initialized storage does not make an exclusive descriptor Copy or authorize an affine ownership transfer.
- Psi 06–08: checked declaration calls retain the whole-input storage receipt and ordinary SharedBorrow plan. The published lending implementation already separates storage from Copy permission. Owned operands retain that independent Copy and unrestricted-multiplicity obligation. No statement occurrence or special interpreter is manufactured.
- Omega 00–02: ordinary layout and calling plans retain the exclusive child and both scalar endpoints. Every positive binds all four hosted ProgramEntry targets; target_body uses the existing ordinary named implementation selected by Build.
- Omega 03–06: caller guard reads and canary 641 challenge lost backing and clobbered storage. This adds no portable WCSU certification.
- Omega 07–09: acceptance requires actual native/Terminal exit-0 agreement on Linux x86-64 and fact-sensitive static refusals. A corrupted guard expectation must trap in both engines. Other hosted native engines remain unmeasured.

## Evidence

Published prerequisite baseline: `ff8fd23a7ba06793f24df7c05bef304de76eafdd`. Its optimized opt-level1 producer has SHA-256 `6209696452b7694143061af613286c70030eede103bed56f3331de9e53485d99`. The frozen shared-lending probe agrees at native and Terminal exit 0 on that baseline, while the unchanged inline `parameter` positive refuses with ``state `invoke` receives `guard` as mutable more than once`` (package-update exit 1). That is a source-checking refusal before execution: the source schedule selects one arm, but the simultaneous-call judgment compares both arm loans together. The source, producer and command observations remain under the publication workbench's ignored `build/advance-evidence/`.

The final optimized producer (`2b74ff9e383973917e102271eae399824b818220919455e4da8ca71cbf9e76da`) runs all nine places and both composition probes at native/Terminal exit 0 on Linux x86-64. Thirteen controls reject with their single intended diagnostic, including every nonprogress twin, the clean false-Copy twin, shared mutation and both sibling-overlap controls. Four existing lending/copy regressions and the original frozen probe also agree at exit 0. Changing only the expected guard from 827 to 828 produces native signal 4 and Terminal `Trap`.

The independent corpus instrument observes 11 checked positives, 13 intended rejections, and 11 actual native/Terminal exit-0 agreements, with no wrong reasons or missing execution oracles. Every positive corpus mirror carries `expected_native_exit.txt`; a built-only observation is not execution evidence. Other-host native qualification remains unmeasured. Scoped corpus mirrors are `facts/nested_machine_joint_rank_exclusive_child_*` under `tests/omega/{pass,fail}`.

The parent remains open for declaration calls requiring exclusive argument loans or owned affine transfers, declaration-local selectors, other ranking forms, remaining invocation obligations, source migration and the Omega-written producer.
