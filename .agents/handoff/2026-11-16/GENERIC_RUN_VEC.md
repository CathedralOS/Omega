# Ordinary ownership and the remaining Run/Vec work

Root owns PR #3764 and the full Run/Vec outcome under claim `18e89b4f6d3691480000000018e98a76`, in `worker-2`. The current landing covers ordinary linear records and calls. Run/Vec acceptance remains open and stays with this session after that milestone lands.

Checkpoint `41867a04b2c` includes the tested source-context and proof-lookup fixes after merge `fbfa28c5048df86750d31039c2067a44921b9fb6` integrated main `a3c0c319802`. The earlier integrated base was `d3deb345653b9812607155ff4b51e17db35ca520`. Checkpoint `a9a15989799b2c8083f9966dae946c1a97c84de6` preserves all source before extraction, including the original Run fixture, unfinished native Run paths and the pending address-predicate repair. It is an ancestor of this work, not a completed implementation.

## Current landing

Record metadata distinguishes copyable, affine and linear values. Separate claims identify the obligations owned by a value's fields. Construction, moves, ordinary calls and returns transport those claims; receiving reconstructs the transfer instead of trusting supplied rows. A linear leaf still owes its claim when a producer erases every claim flag and row. Declaration lookup allocates scratch by actual rows, not the largest supplied ID.

Bodyless requirements have no activation to walk, but their parameter claims still require publication and checking. They now use the same claim validator without inventing a body. A transition whose declared result is proved empty also fits a Unit destination through the shared type check; the actual terminal call remains, and an inhabited result still refuses even when its private body crashes.

The unfinished Run split/merge paths were removed from this landing, including their native operations and unwired source-identity validator. Existing empty-run release remains. General bodyless-header fixes and ordinary ownership code remain. No new Split/Merge capability is claimed.

## Current acceptance and remaining landing checks

The unchanged ownership program now runs on macOS arm64: native exit 0 and Psi exit 0. The run took 52.96 seconds and peaked at 1.60 GB RSS. Its duplicate-transfer twin rejects the second use of `input` with ``claim transport observes unavailable owned storage in machine `main` `` at line 17. That check took 31.29 seconds. The negative previously stopped for an unrelated linear value left at a normal return; it now duplicates the call in `main`, where successful process exit permits the remaining values to be abandoned. This isolates the forbidden second transfer.

The actual run was `omega run --both --keep tests/fixtures/run/ownership/linear_sibling_call/main.omg`, without `--target`. Supplying `--target` makes this command compile only, even for the host target. Earlier compile-only success is not execution evidence. The kept program is `/var/folders/_v/t3hcj52529g70h1rjsvtkws80000gn/T/omega-probe-65086-0`.

Ordinary address predicates now use the ordinary source address representation. Installed callable bindings still require their valid address model. Named domains can use mathematical expressions, while their embedded executable operations must be proved valid before their conditions are assumed. Tests exercise scalar and record domains, writes to qualified fields, unchanged neighboring fields and refusal of a partial operation that tries to justify itself. The domain-write test passes; its tightened assertion naming the failed domain obligation awaits one focused rerun.

The portable source-identity test now compares the supplied digest with the digest of retained toolchain source. Both match, and source with neither package nor toolchain ownership still refuses. The transparent address-predicate control also passes; its negative retains an actual unproved `no_wrap(base, length)` call requirement. The temporary proof tracing is removed. Fixed-view artifact version 116 distinguishes the new three-way multiplicity layout from main's version 115 copy-bit layout; both old-header refusal controls pass.

Evidence is in `build/ownership/mathematical-domains-host-run/`. The optimized CLI digest is `1062f10d76d55b7606200d8f2048c747adac260591b93cf34e4220a922b0b0fa`. It was built from `41867a04b2ca08bf82813884efee8a4b843bab91` plus frozen patch `a4c7fcba39935efc54ed23576fd7f3138a7dfbcf2c9316047fcf22213d4c378f`; Cargo profile and fingerprint receipts are in `build/ownership/mathematical-domains-current-main/producer.json`. The later changes affect only test code and the negative input. `producer-applicability.json` records that comparison and the checked executable hashes.

GitHub reported conflicts with main `e1234a187cc`. The integration uses main’s address-predicate implementation from `f12f92a6666`, which independently repairs the same ordinary-versus-binding distinction, and retains the ownership and arithmetic-operand fixes. The main implementation includes nested-predicate and malformed-binding controls; it replaces this branch’s duplicate address-context helper and its unit test. Both arithmetic and fixed-array contract tests remain. Rebuild the optimized producer and rerun the original pair plus the affected address, domain and contract tests because these inputs changed. Then finish affected-crate Clippy, repository checks, admission, squash merge and remote verification. Earlier lint passed on `3710611a7d3`; integrated-candidate lint remains pending. Full Run/Vec remains this session's next work.

## Retained evidence

Results below belong to their recorded source revisions. They do not establish acceptance on the latest integrated source.

- Twenty focused ownership/header controls pass. They cover sibling transfers, altered or erased claim records, sparse IDs, active cases, constructors, returns, borrowed successors and bodyless signatures. The six records are `/tmp/omega-3764-{erasure-bounded,address-and-claims-retry,header-native,header-native-retry,extracted-native,extracted-native-retry}.json`.
- Three empty-result controls pass, including Unit compatibility and refusal of an inhabited result. Four storage-lifetime controls pass: internal edges retain obligations, normal returns still owe them, incompatible joins refuse, and only empty unclaimed homes retire. See `/tmp/omega-3764-never-unit.json` and `/tmp/omega-3764-hosted-identity.json`.
- Both Core identity controls pass, including independent receiving and counterfeit rejection. They resolved the actual boundary-entry identity mismatch. The next failure required normal package setup, not a custody exception. See `/tmp/omega-3764-boundary-entry.json`.
- Selected layout calls now retain authenticated source and application context. The controls execute the selected implementation, reject changed or ownerless selections, enforce its public requirement and distinguish changed source bytes. The arithmetic control retains actual operand types despite an unrelated overload. See `/tmp/omega-3764-layout-arithmetic.json`; its producer is `af74af59a59a2a34ddd9dc8d2fea859c5f04d25318d60d3a4e2d274d84d03b79`.
- The earlier diagnostic chain passed four diagnostic/format controls and identified the indexed-case failure. Package checking took 130.78 seconds and 2.05 GB peak RSS. Clippy reported a test module placed before a helper; the unchanged module has been moved, but lint has not been rerun. No package review was created. See `/tmp/omega-3764-terminator-detail.json`; its producer is `837f74d331ba6204a2afe90709a4e3b603443e3f03a4cff6cd6051a0e6b27701`.

- The indexed-case chain passed four controls and affected-crate Clippy on producer `0bf6da5beb4f86c9f6941bd788cb34ea67a87b753ccf6c4d06acf99cef25b564`. `/tmp/omega-3764-indexed-case-retry.json` records the frozen patch, 183-second test build, 189-second CLI build, 67-second lint run and interrupted package attempt (2.48 GB peak RSS). The first attempt stopped at test compilation because its setup tried to clone a non-cloneable graph; the retry constructs that graph directly. No package or runtime success is claimed.

The executable fixtures are `tests/fixtures/{run,fail}/ownership/linear_sibling_call`. Successful process exit explicitly abandons remaining obligations; this witness does not prove resource release. Source review found no blocker in the ordinary claims and empty-result repairs. Formatting, repository rules and dependency boundaries passed at their recorded checkpoints. Runtime acceptance and the intended duplicate-transfer refusal are now recorded above. Admission and lint on the integrated candidate remain open.

## Resume Run/Vec after the ownership milestone

Reapply only the still-missing Run changes from `a9a15989799b2c8083f9966dae946c1a97c84de6`, keeping the landed ownership fixes, and continue the original Run source and contracts. The original fixture is `tests/fixtures/pass/collections/run_split_merge/main.omg` at that revision. Its missing-cut twin must reach and fail the actual call requirement.

The address and mathematical-domain prerequisites now pass their focused controls. The transparent predicate preserves its actual call requirement; the earlier reported operand-type failure came from the test misclassifying an incomplete negative proof. Retry the preserved Run fixture with the current producer before choosing its next repair. Do not weaken `Granted`, `no_wrap`, the caller contracts or the test oracle.

Descriptor geometry does not establish custody of occupied cells. Split/merge still need the actual ordered values, their obligations, contained loans and carry permissions. Keep native and receiving paths from claiming that support until it exists. The interpreter still refused those operations in the preserved source.

The broader WIP remains at `c53b684a5caf7e11b040fc26bd5f604a6d63194a`, on `work/generic-run-vec-unvalidated-20261116`. It contains the policy-value port, retained sequence declarations, lifecycle and content publication, generic Vec and executable controls. Its interpreter index is unwired and has an unresolved metadata-reclamation issue; retained declarations alone do not implement ordered custody.

Full acceptance remains the ordinary generic library Vec in `wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing`: actual linear and zero-sized elements, contained loans, fallible growth and initialization, element order, exactly-once disposition and backing return through native and Psi execution. No task-board acceptance is closed by this note or by the ownership checkpoint.
