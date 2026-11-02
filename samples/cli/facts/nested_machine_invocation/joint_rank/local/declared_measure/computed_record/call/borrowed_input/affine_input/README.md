# Shared calls on affine rank inputs

Owner: **NESTED-MACHINE-UNIFICATION** in [TASKS.md](../../../../../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

This continues the existing [shared-reference production](../reference_bearing/README.md). Its input is an owned affine record with a nominal cleanup hook and an exclusive descriptor. The selected pure, total `distance(&value)` call lends the initialized record; it neither copies that record nor consumes the descriptor. The same seven-step distance and counter operation remain unchanged.

The retained application input records its exact initialized owned home and typed projection. Copy permission is a separate judgment on each selected owned operand. A plain Copy child may be copied from an affine parent; passing the affine parent itself as an owned observation gains no such permission. Existing root ownership, selector bounds, independent application formation, shared access, purity and totality checks remain in force.

Every `main` executes both the seven-step and empty paths. The caller checks counter 3→10→10 and cleanup count 0→1→2. The cleanup hook changes caller-owned storage and clears the record canary. Early cleanup corrupts the canary before completion reads it; duplicate or omitted cleanup changes the caller-side count. Each falsifies the runtime oracle. Prefix 419 and canary 641 surround the updated rank fields and remain checked at completion. Each nonprogress twin changes only `pending.lower + 1` to `pending.lower + 0`.

| Place | Positive | Nonprogress twin |
| --- | --- | --- |
| local | [source](main.omg) | [source](invalid/main.omg) |
| field | [source](../../../../../../field/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../field/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| parameter | [source](../../../../../../parameter/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../parameter/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| call_result | [source](../../../../../../call_result/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../call_result/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| match_join | [source](../../../../../../match_join/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../match_join/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| const_name | [source](../../../../../../const_name/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../const_name/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| other_state | [source](../../../../../../other_state/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../other_state/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| other_machine | [source](../../../../../../other_machine/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../other_machine/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |
| target_body | [source](../../../../../../target_body/declared_measure/computed_record/call/borrowed_input/affine_input/main.omg) | [source](../../../../../../target_body/declared_measure/computed_record/call/borrowed_input/affine_input/invalid/main.omg) |

## Requirements and evidence

- Establishment: the real owned entry parameter supplies initialized storage. The helper independently establishes ordered endpoints before its distance equation is consumed; the desired rank supplies no formation premise.
- Identity and lifetime: the original record owns its contained exclusive descriptor and nominal cleanup throughout every short shared call. Scalar proof reads cannot reconstruct that ownership or extend its lifetime.
- Composition: the same complete input and measure travel through all nine places, including an ordinary returning helper and a Build-selected implementation. Every full cycle strictly descends.
- Soundness: [an owned formal supplied a borrow](controls/borrow_into_owned/main.omg), [write-only observation](controls/write_only_input/main.omg), [mutation through shared access](controls/mutating_helper/main.omg) and [reusing the moved affine input](controls/reuse_owned_input/main.omg) challenge independent boundaries. [Copying a selected plain child](controls/owned_projection/main.omg) preserves the existing operand-level rule.
- Psi 06–08: the initialized-home receipt replaces the superseded root-copy receipt. The ordinary argument plan retains shared access; no synthetic statement occurrence, rebuilt aggregate or second interpreter is added. Existing cleanup plans execute the observable caller-side hook.
- Omega 00–02: every positive binds all four hosted ProgramEntry targets. Target-body uses an ordinary named implementation explicitly selected by Build. Existing layouts and calling plans carry the whole record and original descriptor.
- Omega 03–06: field canaries, the live caller counter and cleanup counter challenge wrong homes, overwritten neighbors and lost backing. This establishes no new portable WCSU or long-cycle physical-stack bound.
- Omega 07–09: acceptance is actual Linux x86-64 native/Terminal exit-0 agreement for the positives and intended static refusals for the controls. Other-host native execution is unmeasured here.

The exact integrated base is `bc2c778d57d90b75587d484a6a882ad2cecac9a4`. Producer revisions, digests, profiles, command exits and raw observations are retained in the publication worktree's ignored `build/advance-evidence/` and summarized in its PR. The earlier preintegration candidate's observations remain historical and are not relabelled as integrated-source runs.

Pinned fixtures are `facts/nested_machine_joint_rank_affine_call_*` under `tests/omega/{pass,fail}`. The parent remains open for exclusive declaration-call arguments, owned affine argument transfers, borrowed outer homes, declaration-local selectors, other ranking forms, remaining invocation obligations, source migration and the Omega-written producer.

The integrated optimized producer (`a3c872617dd75df21f05734fe815ea714e8d4185390f8b38e28f38fbee1b7f03`, opt-level 1) executes all nine sample places and the selected-child composition with Linux native/Terminal exit 0 agreement. The source corpus records all 23 new fixtures with their intended outcomes. The 22 existing shared-reference fixtures retain 11 checked and 11 rejected outcomes with every expected fragment present; measured timing changes occurred under concurrent build load.

The independent corpus runtime leg executes all ten new positives and preserves all eleven existing shared-reference positives, with native/Terminal exit 0 agreement and no structural moves. The exact-base runner (`bc2c778d57d90b75587d484a6a882ad2cecac9a4`) executes the supported Copy control at exit 0 but reports both affine probes incomplete at `state graph: rank observations`, before either engine runs. Changing only the expected final cleanup count from 2 to 3 produces native signal 4 and Terminal `Trap`.
