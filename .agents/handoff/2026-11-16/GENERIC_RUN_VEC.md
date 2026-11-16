# Ordinary ownership and the remaining Run/Vec work

Root owns PR #3764 and the full Run/Vec outcome under claim `18e89b4f6d3691480000000018e98a76`, in `worker-2`. The current landing covers ordinary linear records and calls. Run/Vec acceptance remains open and stays with this session after that milestone lands.

The fixed integrated base is `d3deb345653b9812607155ff4b51e17db35ca520`. Checkpoint `a9a15989799b2c8083f9966dae946c1a97c84de6` preserves all source before extraction, including the original Run fixture, unfinished native Run paths and the pending address-predicate repair. It is an ancestor of this work, not a completed implementation.

## Current landing

Record metadata distinguishes copyable, affine and linear values. Separate claims identify the obligations owned by a value's fields. Construction, moves, ordinary calls and returns transport those claims; receiving reconstructs the transfer instead of trusting supplied rows. A linear leaf still owes its claim when a producer erases every claim flag and row. Declaration lookup allocates scratch by actual rows, not the largest supplied ID.

Bodyless requirements have no activation to walk, but their parameter claims still require publication and checking. They now use the same claim validator without inventing a body. A transition whose declared result is proved empty also fits a Unit destination through the shared type check; the actual terminal call remains, and an inhabited result still refuses even when its private body crashes.

The unfinished Run split/merge paths were removed from this landing, including their native operations and unwired source-identity validator. Existing empty-run release remains. General bodyless-header fixes and ordinary ownership code remain. No new Split/Merge capability is claimed.

## Evidence and next checks

- Twenty focused ownership/header controls pass at their recorded revisions. They cover actual sibling transfers, five altered transfer records, complete claim-metadata erasure, sparse IDs, active cases, constructor bindings, normal returns, borrowed successors and bodyless signatures. Records: `/tmp/omega-3764-erasure-bounded.json`, `/tmp/omega-3764-address-and-claims-retry.json`, `/tmp/omega-3764-header-native.json`, `/tmp/omega-3764-header-native-retry.json`, `/tmp/omega-3764-extracted-native.json` and `/tmp/omega-3764-extracted-native-retry.json`.
- Three empty-result compatibility controls pass, including the new Unit transition and its inhabited twin. The original executable gets past both the missing bodyless claim plan and the Never-to-Unit typing refusal. Its next failure is `normal edge cannot implicitly dispose storage without an authorized disposition`; neither engine ran. The optimized macOS arm64 producer is `caafd5e0b93fe6a7b9ff44fea97f17952006ed673547294a81a712b561400969`; source and observations are in `/tmp/omega-3764-never-unit.json` and its adjacent patch/logs.
- The next repair must keep a live value that cannot be implicitly disposed across ordinary control-flow edges. Read liveness alone cannot authorize forgetting its obligations before a later exit or crash. A real normal return must still account for that value.
- Source review found no blocker in the claim change or the shared empty-result compatibility repair. Formatting, repository rules and dependency boundaries pass at their recorded checkpoints. Native/Psi execution, the duplicate-transfer twin, scoped lint and admission remain required. Historical contributor checks are not fresh results on this candidate.

The executable claim fixtures are under `tests/fixtures/{run,fail}/ownership/linear_sibling_call`. Successful process exit explicitly abandons remaining obligations; this witness does not prove resource release.

## Resume Run/Vec after the ownership milestone

Reapply only the removed Run and address-predicate changes from `a9a15989799b2c8083f9966dae946c1a97c84de6`, keeping the landed ownership fixes, and continue the original Run source and contracts. The original fixture is `tests/fixtures/pass/collections/run_split_merge/main.omg` at that revision. Its missing-cut twin must reach and fail the actual call requirement.

Both currently stop before that point. The address repair gets past the missing binding-model refusal, but a mathematical domain predicate remains unsupported. Its smaller transparent-predicate positive also reaches a proof operand-type mismatch. These are engineering gaps, not owner decisions. Do not weaken `Granted`, `no_wrap`, the caller contracts or the test oracle.

Descriptor geometry does not establish custody of occupied cells. Split/merge still need the actual ordered values, their obligations, contained loans and carry permissions. Keep native and receiving paths from claiming that support until it exists. The interpreter still refused those operations in the preserved source.

The broader WIP remains at `c53b684a5caf7e11b040fc26bd5f604a6d63194a`, on `work/generic-run-vec-unvalidated-20261116`. It contains the policy-value port, retained sequence declarations, lifecycle and content publication, generic Vec and executable controls. Its interpreter index is unwired and has an unresolved metadata-reclamation issue; retained declarations alone do not implement ordered custody.

Full acceptance remains the ordinary generic library Vec in `wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing`: actual linear and zero-sized elements, contained loans, fallible growth and initialization, element order, exactly-once disposition and backing return through native and Psi execution. No task-board acceptance is closed by this note or by the ownership checkpoint.
