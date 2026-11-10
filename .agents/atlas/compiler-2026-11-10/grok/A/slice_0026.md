# slice_0026

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0192 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:1714-1716 | UNDECIDED | Missing `StructuralTypeDeclaration` for a formal’s identity yields `EndpointUse::Storage`; whether that identity can be absent from the lowering `types` table was not traced. |
| A0193 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:1855-1858 | CLOSED | The `entry == None` branch exists, but both production callers pass `Some(...)`, so universal dominators are not computed today. |
| A0194 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:4546 | UNDECIDED | Successor block ids missing from `identities` are dropped by `filter_map`; whether a lowered machine can still carry such a terminator target was not traced. |
| A0195 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:5248 | REACHABLE | A runtime byte index path checks disjointness once, then only probes fixed index 0. |
| A0196 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:8245-8253 | UNDECIDED | `BorrowedView` matches `readable_byte_source_bound` without comparing capacity; the digest’s separate obligation at `7922` was not traced through runtime proof. |
| A0197 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:9646 | UNDECIDED | `IntegerComparison` is a leaf in `contains_short_circuit`; whether checked lowering can still place short-circuiting direct subtrees in its operands was not traced. |
| A0198 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:1194-1196 | UNDECIDED | `restore_path` no-ops when the place is absent from the frontier; whether `StoreStructuralField` restoration can run in that state on accepted programs was not traced. |
| A0199 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:872-895,919 | UNDECIDED | `StructuralCase` case arms and `ExternalCompletion` skip `transfer` on structural arguments; whether those lists can be non-empty in production terminators was not traced. |

## A0192

**Cited site (read):** `emission.rs:1703-1729` in `completed_place::endpoint_use` (module `completed_place` inside `emission.rs`). Lines `1714-1716` match the atlas cite at this checkout.

When `types.iter().filter(|row| row.identity == identity)` has no first row, the function returns `Ok(EndpointUse::Storage)` instead of erroring (`1714-1716`). Duplicate identities error at `1717-1718`; missing checked identity errors at `1709-1711`.

**Callers (read):** `rg '\bendpoint_use\b' omega-rust` → 3 matches: definition at `emission.rs:1703`, call at `unit_operation_frame.rs:1808`, call at `scalar_computations.rs:988`.

**Upstream (read):** `unit_operation_frame.rs:1793-1831` pairs the result with `completed_place::plan(..., endpoint,)` for captured structural call operands. `scalar_computations.rs:981-994` does the same for computed structural arguments. `EndpointUse::Storage` steers `plan` toward storage-style capture (`emission.rs:1680-1688` uses `runtime_structural_path_tip` when copying with `EndpointUse::Storage`).

**Accept path (inferred):** A checked formal whose `type_identity` text is present but whose identity is not represented in the `types` slice passed into lowering would be classified as storage. Whether compilation still accepts such a formal without an earlier diagnostic was not traced through stage-05 type retention or structural type catalog construction.

**Verdict:** UNDECIDED — need evidence that every `CheckedUnitStructuralParameterPlan` identity always has a matching `StructuralTypeDeclaration` in the lowering catalog.

## A0193

**Cited site (read):** `emission.rs:1841-1891` in `control_flow::block_dominators`. When `entry` is `None`, `1855-1857` builds a universal set and returns `vec![universal; count]` so every block dominates every block.

**Callers (read):** `rg 'block_dominators' omega-rust` → 3 matches: definition `emission.rs:1841`, call `emission_cleanup.rs:1924`, call `unit_composed_control.rs:6165`.

**Caller arguments (read):** `emission_cleanup.rs:1923-1924` passes `Some(entry_position)`. `unit_composed_control.rs:6165-6167` passes `Some(entry)` where `entry` is the machine entry index computed in the same helper (`6142-6171`).

**Accept path (read):** Dominators feed parameter block ordering and cleanup specialization (`emission_cleanup.rs:1930-1939`). With real dominators from `Some(entry)`, behavior is the standard iterative dominance algorithm (`1859-1891`). The permissive branch does not run on either caller path today.

**Verdict:** CLOSED — production call sites always pass `Some(...)` (`emission_cleanup.rs:1924`, `unit_composed_control.rs:6165-6167`), so the universal-dominator fallback does not affect acceptance.

## A0194

**Cited site (read):** `emission.rs:4542-4548` inside `private_borrow_transport::forward_from` (`4456+`). Successor block indices are built with `.filter_map(|target| identities.iter().position(|identity| *identity == target))` (`4546-4547`), so unknown `BlockId` values are omitted rather than rejected.

**Enclosing function (read):** `forward_from` walks operations and terminators, observes borrow demand, builds `successors`, then propagates demand and injects forwarded block parameters (`4489-4628`). Unlisted targets simply omit an edge in `successors`.

**Callers (read):** `rg 'forward_private_state_borrows' omega-rust` → 2 matches: definition `emission.rs:4426`, call `unit_composed_control.rs:6634-6640`.

**Input (inferred):** A terminator whose `target` is a `BlockId` not equal to any `block.id` in the current `blocks` slice would lose that edge in borrow forwarding while lowering continues.

**Verdict:** UNDECIDED — whether composed-body emission can leave such a terminator on an accepted machine was not traced (all targets might always be in-machine ids).

## A0195

**Cited site (read):** `emission.rs:5238-5249` in `require_unrestricted_byte_region` (byte footprint helpers in `emission.rs`). For `ByteStoreIndex::Runtime(_)`, after a whole-path disjointness check (`5239-5246`), the code pushes only `FixedIndex(0)` and calls `require_unrestricted_receiving` once (`5248-5249`).

**Enclosing function (read):** `require_unrestricted_byte_region` (`5212-5253`) is used for byte representation stores and logical footprints (`4958`, `5276`).

**Callers (read):** `rg 'require_unrestricted_byte_region' omega-rust` → 3 matches, all in `emission.rs` (definition plus two call sites).

**Upstream (read):** Caller `emission.rs:4957-4958` is on the byte-representation store path inside structural scalar store lowering, reached from body emission (inferred from `prepare_byte_destination` / store pipeline in the same module).

**Input (read, plain words):** A program stores through a runtime byte index into a non-uniform array where qualifications overlap only at indices greater than zero, while index zero remains unrestricted, could pass the disjointness precheck and the single probe at zero, then be accepted for lowering.

**Verdict:** REACHABLE — only element zero is checked under a runtime index after the disjointness test.

## A0196

**Cited site (read):** `emission.rs:8227-8255` in `readable_byte_source_bound`. For `ByteSequenceCarrier::BorrowedView { access }` the function returns true on access alone and does not compare `maximum_length` to a view capacity (`8245-8253`). `BoundedView` and `BoundedOwned` compare capacity (`8229-8237`).

**Consumers (read):** `rg 'readable_byte_source_bound' omega-rust` → 3 matches in `emission.rs`: definition `8227`, uses at `8153` (whole-byte `ByteValue::Whole`) and `8178` (field carrier).

**Enclosing emitter (read):** `ValueEmitter::emit` (`8151-8155`) rejects when no declaration row satisfies `readable_byte_source_bound(&row.shape, *capacity)`; a borrowed-view shape can satisfy the predicate without capacity agreement.

**Related obligation (read, not traced):** `emit_concatenation` at `7919-7922` attaches `capacity_obligation` on `EstablishByteSequenceConcatenation`; whether whole-byte or runtime proof closes the capacity gap for borrowed views was not followed through obligations or stage-08 verification.

**Verdict:** UNDECIDED — digest matches the code shape; closing checks beyond `7922` were not traced.

## A0197

**Cited site (read):** `emission.rs:9631-9654` in `expression_validation::contains_short_circuit`. The `IntegerComparison` arm returns `false` without inspecting `left` or `right` (`9646`).

**Enclosing logic (read):** `validate_short_circuit_expression` (`9666-9705`) also treats `IntegerComparison` as unconditionally `Ok(())` at `9689`. `Equal`/`And`/`Or` recurse (`9647-9652`, `9693-9703`).

**Lowering shape (read):** `expression_preparation.rs:1725-1740` lowers `CheckedBooleanExpression::IntegerComparison` with `left`/`right` from `lower_scalar`, not `lower` on booleans.

**Stage-05 parallel (read):** `execution_terminal_unit.rs:3512` lists `IntegerComparison` as non-short-circuit in `checked_boolean_contains_short_circuit`.

**Downstream uses (read):** `contains_short_circuit` drives guard branching in `unit_arguments.rs:3438` and boolean decision lowering (`emission.rs:10094-10114`). `rg '\bcontains_short_circuit\b' omega-rust` → 12 symbol hits across `emission.rs` (8), `unit_arguments.rs` (3), `unit.rs` (1). `direct_expression_contains_short_circuit` has additional call sites (`unit_arguments.rs` 15 hits per count mode).

**Verdict:** UNDECIDED — the leaf treatment is real (read), but whether any accepted program can still embed short-circuiting direct subtrees inside `IntegerComparison` operands was not traced through scalar typing at stage 05 or `lower_scalar`.

## A0198

**Cited site (read):** `emission_cleanup.rs:1193-1212` in `affine_cleanup::projected_transfers::restore_path`. If `frontier.iter().position(|root| root.place == place)` is `None`, the function returns immediately (`1194-1196`) without error.

**Caller (read):** Sole call at `emission_cleanup.rs:832` inside `projected_transfers::complete` (`721+`), after `StoreStructuralField` computes a restoration projection (`807-831`).

**Enclosing driver (read):** `projected_transfers::complete` is invoked from `affine_cleanup::complete` at `emission_cleanup.rs:1393-1400`. `affine_cleanup::complete` is called from `unit_composed_control.rs:11206` and `attached_unit.rs:11530` (`rg 'affine_cleanup::complete' omega-rust` → 2 call sites plus wrapper).

**Effect (read):** When the destination place is not tracked in `frontier`, restoration after field store is skipped silently instead of updating paths or failing.

**Verdict:** UNDECIDED — whether accepted programs can reach `StoreStructuralField` restoration with the destination missing from `frontier` was not traced.

## A0199

**Cited site (read):** `emission_cleanup.rs:872-919` inside `projected_transfers::complete`’s terminator dispatch. `Terminator::Jump` and `Terminator::Conditional` call `transfer` on `structural_arguments` (`847-848`, `860-861`). `Terminator::StructuralCase` (`872-894`) runs overlap checks and `complete_actions` on case edges but never calls `transfer` on case `structural_arguments` (emission often builds `structural_arguments: Vec::new()` at `unit_composed_control.rs:8201`). `Terminator::Crash` and `Terminator::ExternalCompletion` are empty arms at `919` with no `transfer`.

**transfer (read):** `emission_cleanup.rs:1023-1033` removes moved paths from the frontier for owning transfers.

**Emission note (read):** Composed case terminators at `unit_composed_control.rs:8210-8214` use `cases` built with empty structural arguments on successors (`8201`).

**Verdict:** UNDECIDED — the skip is real (read); whether `StructuralCase` or `ExternalCompletion` terminators with non-empty `structural_arguments` can appear on accepted machines was not traced.

## What I did not read

- Stage-05 or retention paths that populate `StructuralTypeDeclaration` rows for every formal `type_identity` (A0192).
- Terminator target validation across composed graph sealing (A0194).
- Runtime or proof discharge for `capacity_obligation` and borrowed-view byte sources beyond `emission.rs:7922` (A0196).
- Full scalar type rules for `CheckedBooleanExpression::IntegerComparison` operands at stage 05 (A0197).
- Frontier construction invariants before `restore_path` on field stores (A0198).
- All emission sites that set non-empty `structural_arguments` on case or external-completion terminators (A0199).
- `git show 457adf4a11f:` line-by-line diff for each cite (lines matched at this checkout on read).

SLICE COMPLETE
