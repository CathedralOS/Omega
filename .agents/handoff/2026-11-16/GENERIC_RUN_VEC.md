# Ordinary ownership and the remaining Run/Vec work

Root owns PR #3764 and the full Run/Vec outcome under claim `18e89b4f6d3691480000000018e98a76`, in `worker-2`. The current landing covers ordinary linear records and calls. Run/Vec acceptance remains open and stays with this session after that milestone lands.

The fixed integrated base is `d3deb345653b9812607155ff4b51e17db35ca520`. Checkpoint `a9a15989799b2c8083f9966dae946c1a97c84de6` preserves all source before extraction, including the original Run fixture, unfinished native Run paths and the pending address-predicate repair. It is an ancestor of this work, not a completed implementation.

## Current landing

Record metadata distinguishes copyable, affine and linear values. Separate claims identify the obligations owned by a value's fields. Construction, moves, ordinary calls and returns transport those claims; receiving reconstructs the transfer instead of trusting supplied rows. A linear leaf still owes its claim when a producer erases every claim flag and row. Declaration lookup allocates scratch by actual rows, not the largest supplied ID.

Bodyless requirements have no activation to walk, but their parameter claims still require publication and checking. They now use the same claim validator without inventing a body. A transition whose declared result is proved empty also fits a Unit destination through the shared type check; the actual terminal call remains, and an inhabited result still refuses even when its private body crashes.

The unfinished Run split/merge paths were removed from this landing, including their native operations and unwired source-identity validator. Existing empty-run release remains. General bodyless-header fixes and ordinary ownership code remain. No new Split/Merge capability is claimed.

## Current failure and next checks

The unchanged ownership program still needs package setup; neither native execution nor Psi execution has run. The indexed-case repair passes four focused controls: receiving validates its exact selected type without granting cell facts, native preparation preserves its selector, the source interpreter executes both distinct cases and rejects an invalid index, and fixed-case behavior remains intact. All 34 affected crates pass Clippy with all targets and warnings denied.

The latest original package attempt was stopped after 360.27 seconds, before a verdict or review. A short stack sample placed 220 of 231 active samples in proof-value identity lookup. `Known::new` queries each row by position, while that lookup restarts an iterator each time, giving quadratic work. A later sample reached broader proof search, so this is a demonstrated cost, not the sole proven cause of the delay. The next repair provides direct bounded row lookup while retaining actual identity offsets. The byte-observation worker is testing the exact shared patch with its original table program; its two related proof-search repairs are also available for reuse.

Retry the original package setup after those repairs. If it produces a review, inspect the actual decisions and resume before running the original program. Keep its accepted lock as evidence. The duplicate-transfer twin must reject for using the same linear value twice. Finish admission, squash-merge the reviewed head and verify publication on main. Full Run/Vec remains this session's next work.

## Retained evidence

Results below belong to their recorded source revisions. They are not fresh results on the untested indexed-case patch.

- Twenty focused ownership/header controls pass. They cover sibling transfers, altered or erased claim records, sparse IDs, active cases, constructors, returns, borrowed successors and bodyless signatures. The six records are `/tmp/omega-3764-{erasure-bounded,address-and-claims-retry,header-native,header-native-retry,extracted-native,extracted-native-retry}.json`.
- Three empty-result controls pass, including Unit compatibility and refusal of an inhabited result. Four storage-lifetime controls pass: internal edges retain obligations, normal returns still owe them, incompatible joins refuse, and only empty unclaimed homes retire. See `/tmp/omega-3764-never-unit.json` and `/tmp/omega-3764-hosted-identity.json`.
- Both Core identity controls pass, including independent receiving and counterfeit rejection. They resolved the actual boundary-entry identity mismatch. The next failure required normal package setup, not a custody exception. See `/tmp/omega-3764-boundary-entry.json`.
- Selected layout calls now retain authenticated source and application context. The controls execute the selected implementation, reject changed or ownerless selections, enforce its public requirement and distinguish changed source bytes. The arithmetic control retains actual operand types despite an unrelated overload. See `/tmp/omega-3764-layout-arithmetic.json`; its producer is `af74af59a59a2a34ddd9dc8d2fea859c5f04d25318d60d3a4e2d274d84d03b79`.
- The earlier diagnostic chain passed four diagnostic/format controls and identified the indexed-case failure. Package checking took 130.78 seconds and 2.05 GB peak RSS. Clippy reported a test module placed before a helper; the unchanged module has been moved, but lint has not been rerun. No package review was created. See `/tmp/omega-3764-terminator-detail.json`; its producer is `837f74d331ba6204a2afe90709a4e3b603443e3f03a4cff6cd6051a0e6b27701`.

- The indexed-case chain passed four controls and affected-crate Clippy on producer `0bf6da5beb4f86c9f6941bd788cb34ea67a87b753ccf6c4d06acf99cef25b564`. `/tmp/omega-3764-indexed-case-retry.json` records the frozen patch, 183-second test build, 189-second CLI build, 67-second lint run and interrupted package attempt (2.48 GB peak RSS). The first attempt stopped at test compilation because its setup tried to clone a non-cloneable graph; the retry constructs that graph directly. No package or runtime success is claimed.

The executable fixtures are `tests/fixtures/{run,fail}/ownership/linear_sibling_call`. Successful process exit explicitly abandons remaining obligations; this witness does not prove resource release. Source review found no blocker in the ordinary claims and empty-result repairs. Formatting, repository rules and dependency boundaries passed at their recorded checkpoints. Runtime acceptance, the duplicate-transfer refusal and admission remain open. The current ownership/indexed-case source has passed scoped lint; later source changes need their affected checks.

## Resume Run/Vec after the ownership milestone

Reapply only the removed Run and address-predicate changes from `a9a15989799b2c8083f9966dae946c1a97c84de6`, keeping the landed ownership fixes, and continue the original Run source and contracts. The original fixture is `tests/fixtures/pass/collections/run_split_merge/main.omg` at that revision. Its missing-cut twin must reach and fail the actual call requirement.

Both currently stop before that point. The address repair gets past the missing binding-model refusal, but a mathematical domain predicate remains unsupported. Its smaller transparent-predicate positive also reaches a proof operand-type mismatch. These are engineering gaps, not owner decisions. Do not weaken `Granted`, `no_wrap`, the caller contracts or the test oracle.

Descriptor geometry does not establish custody of occupied cells. Split/merge still need the actual ordered values, their obligations, contained loans and carry permissions. Keep native and receiving paths from claiming that support until it exists. The interpreter still refused those operations in the preserved source.

The broader WIP remains at `c53b684a5caf7e11b040fc26bd5f604a6d63194a`, on `work/generic-run-vec-unvalidated-20261116`. It contains the policy-value port, retained sequence declarations, lifecycle and content publication, generic Vec and executable controls. Its interpreter index is unwired and has an unresolved metadata-reclamation issue; retained declarations alone do not implement ordered custody.

Full acceptance remains the ordinary generic library Vec in `wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing`: actual linear and zero-sized elements, contained loans, fallible growth and initialization, element order, exactly-once disposition and backing return through native and Psi execution. No task-board acceptance is closed by this note or by the ownership checkpoint.
