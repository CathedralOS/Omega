# Record: unit_026 (area 12-native-representations)

Crate: `optimization-unit-semantics`, path prefix
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/` (written `S/` below).
Checkout `4f3c255d`. All claims are "read" unless marked grepped or inferred. Line counts come from the unit file.

## S/lib.rs (267 lines)

1. Purpose. Crate root. Declares modules and re-exports the validators; defines a handful of thin public
   queries that forward to `unit_validation::operation_contracts::structural_source_contract`.
2. Shape. Public re-exports at `lib.rs:68-90`, `:232-247`. Free functions: `operation_place_inputs` (`:92`),
   `structural_place_access` (`:98`), `descriptor_argument_accessible` (`:106`),
   `descriptor_metadata_argument_accessible` (`:129`), `structural_place_backing_access` (`:152`),
   `structural_place_supports_window_restoration` (`:214`). Macro `structural_write_preserves!` (`:169`)
   expands to `structural_write_preserves_memberships` (`:204`) and `structural_write_preserves_content` (`:209`).
   Trait `StructuralTypes` (`:252`) with impl for `[StructuralTypeDeclaration]` (`:257`, binary search by id).
3. I/O. Reads a `PsiOptimizationFunction`, a place, and structural type declarations; returns `Option`/`bool`.
   No mutation.
4. How it decides. Every query keys on a `PlaceId` looked up through `structural_source_contract(function, place)`
   (`:102,113,136,165,180,218`). Window restoration (`:214-229`) accepts when access is `Owned` or `MutableBorrow`,
   multiplicity is not `Linear`, and the operation result carries no claims.
5. Cases by shape. `structural_place_supports_window_restoration` is a fixed access list (`Owned | MutableBorrow`)
   (`:220-224`).
6. Same job elsewhere. `StructuralTypes::declaration` (binary search `:259`) vs. linear `types.iter().find(..)`
   in `descriptor_results.rs:1564,2284,2789` and BTreeMap lookups in `current_ownership.rs`; three lookup styles
   for the same table.
7. Unused / one-caller. Grepped (Grep count, pattern list over `source-rust`): the group
   `captured_scalar_ordering | call_origins | structural_write_preserves_* | structural_place_supports_window_restoration |
   descriptor_argument_accessible | descriptor_metadata_argument_accessible | structural_place_backing_access | structural_place_access(`
   has 28 hits in 6 files; external files are stage-04 `legalization_reference_custody.rs` (2),
   `legalization_scalar_graph.rs` (12), `legalization_target.rs` (5). I did not split per function.
   `operation_place_inputs(` is 15 hits in 5 files (grep), 1 in stage 04.
   Lib.rs header comment at `:50-66` says downstream stages consume these; stage 04 is the only external consumer seen.
8. Stated limits. `lib.rs:5-8` "neither lower Terminal Psi nor sequence optimization passes"; `:11-12` "structural
   validation alone grants no execution or publication authority".
9. Fail-open risks. `descriptor_argument_accessible` / `_metadata_` return `false` on missing source (`:112-116,135-139`):
   fail-closed. `structural_place_backing_access` returns `None` on missing: fail-closed for the caller if it treats
   `None` as deny (not traced). `structural_place_supports_window_restoration` returns `false` on missing (`:218`).
   None seen.

## S/error.rs (391 lines)

1. Purpose. Failure vocabulary for whole-unit validation, ownership replay, range facts and rewrite candidates.
2. Shape. `OperationSite { machine, block, node: u32 }` (`:18`, `new` `:25`). Enum
   `OptimizationUnitValidationError` (`:35-383`), about 150 variants; `Display` prints `{self:?}` (`:385-389`).
3. I/O. Pure data.
4. How it decides. Nothing; keys are identities (`MachineId`, `BlockId`, `PlaceId`, `ClaimId`, `OperationId`).
   Several variants carry `&'static str` reasons (`CallableValueProvenance`, `InvalidRuntimeEvidence`,
   `CurrentReferenceCustodyViolation { reason }`, `CurrentOwnedValueUnavailable { reason }`), so those are
   discriminated by prose, not by variant.
5. Cases by shape. Candidate rejection is a fixed list of 20 `Candidate*Mismatch` variants (`:366-382`).
6. Same job elsewhere. `current_ownership.rs` uses `CurrentCleanupMismatch` for roughly 60 distinct failure causes
   (read; see 4 below) and `CurrentSharedJoinOriginDisturbed` for compatibility failures
   (`current_ownership.rs:734`), so variant names do not identify the cause.
7. Unused. Not checked per variant. `OperationSite` is re-exported `lib.rs:77`.
8. Stated limits. None.
9. Fail-open. None seen.

## S/current_value_ranges.rs (177 lines)

1. Purpose. Checks that a supplied integer range fact is exactly what the current constant operations produce, and
   that its value is available at the queried node.
2. Shape. Private modules `availability` (`:5`) and `reconstruction` (`:81`). Entry points:
   `validate_current_value_range_fact` (`:155`, `pub(crate)`), `validate_current_value_range_fact_at` (`:168`, pub),
   `independently_reconstruct_value_range_fact_at` (`:140`, re-exported `pub(super)` `:154`),
   `value_available_at` (`:46`), `constant_range` (`:92`).
3. I/O. Reads `PsiOptimizationUnit`, a `ValueRangeFact`. Calls `validate_psi_optimization_unit` (`:159`) first,
   then rebuilds the fact via `validator_scalar_constant_facts` and compares with `!=` (`:160-164`). Returns `Result<()>`.
4. How it decides. Range exists only if the value is an integer-typed definition with an `Integer` constant fact
   (`:98-109`); min = max = constant (`:126-127`). Availability keyed on definition site vs `(block,node)` position
   and dominator set (`:52-78`).
5. Cases by shape. Only constant-backed ranges: header says "Independently reconstructs constant-backed range claims"
   (`:1`). `ValueRangeSupport::ScalarConstant` is the only support built (`:109`). Any non-constant range fact fails
   with `CurrentValueRangeFactMismatch`.
6. Same job elsewhere. Dominance via `independent_reachable_dominators` also used in `current_ownership.rs:1994,2849` (grepped).
7. Unused. `validate_current_value_range_fact_at` has external users: not grepped separately (suspected: stage 03/04).
8. Stated limits. `:1` doc line above.
9. Fail-open. In `value_available_at` a `FunctionParameter` definition returns `true` at every position (`:56`);
   traced: parameters are defined everywhere so this is correct. Missing dominator row returns `false` (`:64,75`). None else.

## S/operation_metadata.rs (683 lines)

1. Purpose. Recomputes, from the `AbstractOperation` alone, the value definitions, value uses and ownership events a
   node should record, and compares to what the node stores.
2. Shape. `metadata_matches` (`:11`, `pub(crate)`), `scalar_definition` (`:21`), `scalar_uses` (`:197`),
   `direct_uses` (`:208`), `expected_ownership` (`:390`), `expected_definitions` (`:532`), `expected_uses` (`:546`).
   Three tests (`:587-682`).
3. I/O. Reads `AbstractOperation`, block, node index; compares to `OptimizationNode.definitions/uses/ownership`.
4. How it decides. By operation variant (exhaustive match, no wildcard). Definitions: one scalar result per variant with
   `(value, scalar_type)` (`:21-195`). Uses: operand values plus runtime indices (`:197-206`). Ownership events:
   `ClaimTransfer`, `ClaimCompletion`, `Cleanup`, `StructuralReturn`, `ExternalCompletionFrontier`, `CrashFrontier` (`:390-444`).
5. Cases by shape. Header `:2-4`: "Explicit no-result, no-input and no-ownership cases make new instruction kinds a
   compile error". All three functions list every variant, so adding an operation edits three match arms here.
6. Same job elsewhere. Duplicate of the producer: `optimization-unit/src/construction.rs:656 fn direct_uses` (grepped;
   header `:1-2` says it is deliberately independent). Test at `:614` compares against
   `optimization_unit::operation_uses`. Also `expected_definitions` is used by `current_ownership.rs:1706`.
7. Unused. `expected_ownership`, `expected_definitions`, `expected_uses` are `pub(crate)`; grep of
   `fn expected_...` shows one definition each; callers inside crate only (not counted).
8. Stated limits. `:4-5` "These checks establish metadata consistency, not an independent proof of the instruction semantics."
9. Fail-open. None seen. Exhaustive matches mean new variants refuse to compile rather than default.

## S/descriptor_results.rs (3609 lines)

1. Purpose. Adapts function bodies (abstract or optimized) into per-machine `DescriptorMachine` flow graphs that the
   shared kernel (`terminal_semantics::descriptor_results`) analyses. Records place definitions, derivations,
   scalar payload reads and writes, call invocations, escapes and cleanup calls.
2. Shape.
   - `pub mod access_results` (`:31`): `Source`, `ResultSlot`, `summarize` (`:54`), `resolve` (`:199`), `derive` (`:220`),
     `agree` (`:241`), `reconcile` (`:258`).
   - `pub(crate) mod scalar_expressions` (`:270`): `ScalarExpressions` (`:856`; maps for types, definitions, booleans,
     integers, owners, incoming, blocks), `new` (`:869`), `literal` (`:939`), `expression` (`:972`); submodule
     `case_conditions` (`:276`) with `retain_case_conditions` (`:685`), `case_test` (`:739`), `unchanged` (`:814`) and
     4 tests (`:282-674`).
   - Catalog entry points: `abstract_catalog_graph` (`:1033`, Views), `abstract_reference_catalog_graph` (`:1050`),
     `abstract_borrowed_storage_catalog_graph` (`:1067`), `optimized_borrowed_storage_catalog_graph` (`:1084`),
     `abstract_graph_with_slots` (`:1103`), `abstract_reference_catalog_graph_with_boundary_preservation` (`:1128`),
     `optimized_graph` (`:1156`), `optimized_catalog_graph` (`:1167`), `optimized_reference_catalog_graph` (`:1190`),
     `..._with_boundary_preservation` (`:1212`), `catalog_graph` (`:1236`), `call_origins` (`:1293`),
     `owned_aggregate_descriptor_returns_with_graph` (`:1306`), `record_escaped_invocation` (`:1334`),
     `adapt` (`:1364`, about 1700 lines), `element_coordinates_preserved` (`:3057`), `captured_scalar_ordering` (`:3089`),
     `scalar_call_guarantees` (`:3099`), `scalar_expression` (`:3157`), `selected_effects_machine` (`:3164`).
   - Test module `owned_cleanup_graph_tests` (`:3333-3609`), one test.
3. I/O. Inputs: function list, `BoundaryMachineDeclaration`s, structural types, `TerminalDynamicDispatchCatalog`,
   `TerminalCallableCatalog`, an operation-projection closure and a `preserves_boundary` closure. Output:
   `Option<Vec<DescriptorMachine>>`. `catalog_graph` runs `adapt` once, and a second time with the first graph as
   `selector_graph` only if some `CallStructural` has a `Dynamic` callee (`:1274-1290`).
4. How it decides. Keyed on place and path identity: `declaration(place)` requires exactly one `structural_places`
   row (`:1448-1455`); duplicate machine ids give `None` (`:1256`); duplicate producer for a place gives `None` (`:1697`).
   Calls: machine callee found by `Some(function.machine) == callee.machine()` and must be unique and arity-equal
   (`:2470-2478`); a callee outside the unit sets `may_escape` unless arguments are `argument_storage_is_closed` and
   the result is Unit/Scalar with no claim transfers (`:2516-2546`). Boundary calls keyed on the boundary declaration
   (found by id, `:2586-2613`), plus `placement_operation() == View` (`:2559-2562`). Case successors keyed on
   `(psi_edge, target)` equality (`:2746-2749`). Edge bindings keyed on `parameter` equality (`:2979-2983`).
   `case_test` follows a boolean value to a `StructuralCaseMembership` or a `BooleanNot` of one, across block
   parameters, only if nothing "changed" between (`:739-843`).
5. Cases by shape.
   - `access_results::summarize` handles `DescriptorStoredOrigin` kinds `AccessFormation | Parameter | IncomingSlot | Empty`
     and returns `None` for `_` (`:185-186`).
   - `case_test` accepts only `StructuralCaseMembership` and `BooleanNot` definitions (`:760-788`).
   - `ScalarExpressions::expression` denotation covers `IntegerArithmetic, BooleanNot, BooleanEqual, IntegerEqual,
     IntegerLessThan, IntegerLessOrEqual` and `_ => return None` (`:983-1013`).
   - Comment `:2243-2246`: "An owned element sequence keeps its roster in read-only image storage ... Target lowering
     refuses any construction whose roster has no constant pool bytes". Applies only when the type is
     `ElementView { access: None }` (`:2248-2253`).
   - Return value tracking only for `ReturnStructural`; the dynamic call branch handles `Machine`, `Callable`, `Dynamic`
     targets in three arms (`:2345, :2388, :2462`).
   - Cleanup calls: only `TerminalAffineCleanupAction::InvokeOwned`; requires a callee with `[formal]` single owned
     parameter at position 0, Unit result and no scalar parameters (`:2785-2838`).
   - `adapt` returns `None` (whole analysis fails) on many structural irregularities: parameter kind mismatch (`:1457`),
     duplicate place type (`:1464`), unknown target block (`:2902`), binding access mismatch (`:2984`).
6. Same job elsewhere. Scalar expression building `ScalarExpressions` (`:856`) vs `RangeRelations::new`
   (`current_ownership.rs:1618`) both walk blocks and build per-position steps; `captured_scalar_ordering` (`:3089`) wraps
   `RangeRelations`. `scalar_call_guarantees` (`:3099`) is reused by `current_ownership.rs:1841`. Dominators: separate
   `independent_reachable_dominators` (grepped, in `candidates`, not read). Three near-identical wrapper families
   (`abstract_*`, `optimized_*`) differ only in `slot_kind` and projection closure.
7. Unused / one-caller. Grep `abstract_catalog_graph\(` over `source-rust`: 0 hits (even in this file the definition
   line `:1033` has `abstract_catalog_graph<` so the pattern missed; grep for the 4 abstract_*/optimized_borrowed variants
   `abstract_reference_catalog_graph\(|abstract_borrowed_storage_catalog_graph\(|optimized_borrowed_storage_catalog_graph\(|abstract_reference_catalog_graph_with_boundary_preservation\(`
   gives 5 hits in 4 files: stage 04 `target_lowering/references.rs` (1), stage 15 `image-emission/src/function_fragments.rs` (2),
   this file (1), `unit_validation.rs` (1)). So `abstract_catalog_graph` and `optimized_borrowed_storage_catalog_graph`
   may have zero external callers; I did not run a name-only grep for each, so "suspected".
   `record_escaped_invocation` is private. `scalar_expression` (`:3157`) is `pub(crate)`: not counted.
8. Stated limits. `:2243-2246` (quoted above). `:2761-2762` "Capturing a descriptor's slot forms a child loan; it does not
   transport the permission already stored in that slot."
9. Fail-open risks.
   - `adapt`'s `payload_writes.push(None)` for `ByteSequenceWrite | ByteSequenceStore` (`:1781-1783`) and for an
     unbounded shared mutation (`:1818-1819`) records "unknown write"; downstream treats `None` as invalidating
     (inferred, kernel not read): fail-closed.
   - `record_escaped_invocation` returns `Some(())` after marking escape; errors only if `writable_descriptor_slots` is `None`.
   - `selected_effects_machine` returns `None` on any mismatch (fail-closed: no effects row).
   - `scalar_call_guarantees` for `CallStructural` uses `callee.machine()?` so dynamic/callable callees give `None` (no
     guarantee assumed): fail-closed.
   - `case_test` / `unchanged` return `None`/`false` on missing flow step (`:833`): fail-closed (no case fact kept).
   - `O::InvokeNoNormal` sets `may_escape` and clears inputs (`:2661-2676`): conservative.
   - `slot_kind == AccessOrigins` skips provider invocation insertion (`:2634`) with no stated reason: "open".
   None found that accepts a wrong program (traced only within this file).

## S/current_ownership.rs (11455 lines)

Overall purpose. Replays each function's CFG and, at every block entry, node and edge, recomputes the live
ownership frontier: live claims, owned places, partially moved paths, decomposed nominal wrappers, live reference
descriptors. Returns an error on the first violation. Also computes loan/pin conflicts for shared joins and descriptor
loans, and emits `OwnershipFrontierFact` snapshots.

Top-level shape (all in this one file; submodules inline):
- `address_joins` (`:27-2460`): `borrowed_regions` (`:28`; `BorrowedRegions` `:62`, `new` `:86`, `origins` `:152`,
  `origins_with_analysis` `:185`, `resolve_at` `:567`, `conflicted_operation` `:672`), `descriptor_loans` (`:694`;
  `DescriptorLoans` `:786`, `new` `:794`, `conflicted_operation` `:1062`, `conflicted_edge` `:1146`,
  `operation_observations` `:1266`, `descriptor_flow` `:1483`, `unknown_flow` `:1520`, `descriptor_inputs` `:1565`, nested
  `compatibility::validate_compatibility` `:714`), `range_relations` (`:1609`; `RangeRelations::new` `:1618`,
  `call_guarantees` `:1836`), `AddressJoinPins` (`:1892`, `new` `:1914`, `check_operation` `:2163`, `check_edge` `:2211`),
  `shared_join_live_blocks` (`:2385`), `entirely_descends_from` (`:2363`).
- `block_parameters` (`:2461-3121`): `consume_edge_sources` (`:2480`), `consume_projected_source` (`:2686`),
  `establish_edge_parameters` (`:2797`), `parameter_establishment_order` (`:2837`), test module `:2864`.
- `call_accesses` (`:3122-3421`): `CallAccesses` (`:3149`, `new` `:3160`, `reconstruct` `:3253`, `written_regions` `:3361`),
  `argument_storage_is_closed` (`:3398`).
- `cleanup` (`:3422-4219`): `validate_return_cleanup_actions` (`:3449`), `unrestricted_owned_cleanup_roots` (`:3520`),
  `cleanup_root_order` (`:3602`), `cleanup_subjects` (`:3655`), `CleanupTraversal` (`:3740`), `consume_owner_cleanup` (`:3870`),
  `consume_owned_cleanup` (`:4118`), `valid_owned_cleanup` (`:4179`).
- `continuation` (`:4221`): `apply_edge_cleanup_actions` (`:4229`). `mutations` (`:4265`): `insert_owned_result`, `reject_live_linear_claim`.
- `references` (`:4312-8031`): `CallRegion` (`:4335`, pub, re-exported `lib.rs:85`), `access_paths::access_reference_custody` (`:4362`),
  `descriptor_candidates` (`:4550`), `descriptor_storage_available` (`:4604`), `call_effects::ReferenceCallEffects` (`:4725`;
  `new` `:4731`, `remaining_at_return` `:4842`, `stage` `:4969`), `StoredReferences::commit` (`:5243`), `windows::transfer` (`:5322`),
  `copy_reference_carriers` (`:6054`), `normalized_source` (`:6146`), `normalized_direct_source` (`:6198`), `release` (`:6319`),
  `discard_subtree` (`:6385`), `establish_record` (`:6447`), `establish_constructor` (`:6501`), `apply_operation` (`:6564`),
  `transfer_return` (`:7726`), `return_origin_family` (`:7914`), `validate_return_coverage` (`:7970`), `validate_normal_return` (`:7998`).
- `replay` (`:8032-9474`): `validate_current_ownership_cfg` (`:8174`), the main worklist loop (`:8216-9453`).
- `residuals` (`:9475`), `storage_regions` (`:9732`), `structural` (`:9879`), `value_custody` (`:10062`; `ValueUse` `:10076`,
  `join_owned_frontier` `:10100`, `decompose` `:10166`, `receiving_store` `:10294`, `moving` `:10411`, `observing` `:10464`,
  `observations` `:10536`, `install_store` `:10661`, test module `store_tests` `:10743-11315`).
- Tail: `validate_current_ownership_frontier_collecting` (`:11325`), `LiveClaim` (`:11358`), `CurrentOwnership` (`:11365`: claims,
  owned_places, partial_custody_paths, nominal_partitions, live_references), `snapshot` (`:11374`),
  `reconstruct_entry_ownership` (`:11407`), re-exports `:11454-11455`.

Inputs/outputs. Reads `PsiOptimizationUnit`, function map, boundary and type tables, `CallAccesses` (which pre-builds
descriptor graphs for the whole unit, `:3176-3204`). Mutates `facts: &mut Vec<OwnershipFrontierFact>` (`:9469`) and returns
`Vec<DescriptorCompatibilityRequirement>` (`:9472`). The replay is a worklist (`ready.pop_first()`, `:8216`); joins require equal
`claims` (`:9427`) and `join_owned_frontier` (`:9433`).

How it decides (keyed on):
- Identity of place + path: claims (`claim.input == Some(place)`, `claim.path`), partial custody (`BTreeSet<Vec<StructuralPathSegment>>`
  per place, `structural_paths_may_overlap`), nominal partitions keyed on `(place, path, structural_type)` (`:10202-10207`).
- Reference custody keyed on `ReferenceIdentity { place, path }` and `carrier`/`carrier_path`, plus origin roots (`LiveReference`).
- Join admission: claims must be `==` (`:9427`); owned places and live references must be `==`; only the partial-custody path set
  can differ, and only for a no-code, non-linear, non-copyable descriptor-cleanup root (`value_custody.rs` fn at `:10100-10162`).
- Cleanup actions are matched against a derived roster by position: `subjects.len() != actions.len()` fails (`:4038`), then
  pairwise `action.place()`, and action kind vs `subject.nominal` (`:4045-4101`).
- Case membership by predecessor path analysis `membership_on_every_path` (external to this file, `unit_validation::function_structure`; not read).
- Reference-bearing owned calls: `contains_reference(types, source.structural_type)`; boundary calls rejected with
  "reference-bearing owned calls require a whole internal argument" (`:7073-7077`).
- Display-name/string dependence: error reasons are `&'static str` literals, tests and callers distinguish by text (e.g. `reason:`
  fields in `CurrentReferenceCustodyViolation`); no logic compares those strings in this file.

Cases by shape (fixed lists and bounds):
- `MAX_INCOMING_REFERENCE_LEAVES: usize = 4096` (`:4332`); `entry_references` fails beyond it (`:5811-5823`).
- `descriptor_candidates` expands runtime index only for `FixedArray` with `Literal` length, count capped by `live.len()`
  (`:4570-4579`); non-literal or non-array returns `None`.
- `partial_affine_residuals` handles only `Record`, `FixedArray` with literal nonzero length, `Sum | Mixed`; `_ => return None`
  (`:9548-9644`). Also `max_residuals` bound (`:9711`) and array length bound `length > remaining + moved.len()` (`:9567`).
- `projected_paths_are_fully_consumed` handles owned-affine roots, or a `Linear` parameter whose type is a `FixedArray` with literal
  length and all indexes moved; else `false` (`:10002-10037`).
- `consume_projected_source` requires source `Affine` owned and parameter `Affine|Linear` (`:2701-2719`); claim multiplicity must be `Linear` (`:2734-2739`).
- `consume_owner_cleanup` residual-discard path requires `multiplicity == Affine` and `no_code_affine_cleanup` (`:3966-3972`).
- `valid_owned_cleanup` requires target with exactly one parameter, Unit result, no scalar parameters, no entry claims (`:4179-4218`).
- `windows::transfer` requires one exact storage region for a reference-bearing window (`:5472-5476`: "reference-bearing window requires one exact storage region").
- `apply_operation` owned reference argument paths must be all `Field` segments (`:7083-7097`).
- `unrestricted_owned_cleanup_roots` eligibility: copyable, no linear claim, `no_code_affine_cleanup && descriptor_cleanup` (`:3539-3551`).
- `storage_cannot_retain_loan` lists shapes: `PrimitiveScalar | AtomicCell` true; `ByteSequence` true if not borrowed; `Record/FixedArray/Sum/Mixed` recursive;
  `TypeParameter | Opaque | Reference | ElementView` false (`:9830-9852`).
- `call_accesses.reconstruct` gives up (leaves summary `None`) when the function contains `Call | AtomicEvent | PortWrite`, a `CallStructural`
  with no machine callee or unsummarised callee, an owned/unclosed boundary-call argument, or any `InvokeOwned` cleanup (`:3272-3323`).
- `shared_join_live_blocks` returns `None` (meaning "every block") unless every use is one of: scalar storage observation, byte length/read,
  or a shared-to-shared jump/conditional (`:2412-2446`).

Same job elsewhere.
- `RangeRelations::new` (`:1618`) and `descriptor_results::adapt` both build a per-position step list over the whole function.
- `descriptor_results.rs:3057 element_coordinates_preserved` is called here at `:765,851,853` (same crate).
- Cleanup root derivation: `unrestricted_owned_cleanup_roots` (`:3520`) vs foundation validation (comment `:3517-3519` says "Foundation validation checks that each retirement follows its dominating producer"); suspected duplicate in `unit_validation.rs` (not read).
- Ownership frontier is also rebuilt in `unit_validation` (`reconstruct_ownership_frontier_facts_with_admitted_cycle_machines`, `lib.rs:234`); this file implements it.
- Place-to-type lookup: `place_structural_type` (`:9889`), `structural_source_contract` (external), `StructuralTypes::declaration`, and `types.get(&id)`: three lookups.
- Leaf and reference origin projections in `references` (`projected_origin`, `projected_origin_family`) repeat the same `Invocation | Primitive | IngressLeaf | Alternatives` four-arm match at `:5631-5681`, `:5685-5709`, `:5763-5770`, `:4670-4683`, `:7928-7933`.

Unused / one-caller (grepped).
- `partial_affine_root_is_drained | projected_paths_are_fully_consumed | partial_affine_residuals | argument_storage_is_closed | storage_cannot_retain_loan | unrestricted_owned_cleanup_roots | parameter_establishment_order | validate_descriptor_compatibility`: 15 hits in 3 files across `source-rust`: this file (10), `unit_validation.rs` (4), `psi/semantics/src/structural/partial_affine.rs` (1) (Grep count, pattern in tool call). So these are used almost entirely inside the crate.
- `partial_affine_residuals` (`:9483`) is wrapper-only here; it has at least one use in `structural.rs` (`:9977`); `psi/semantics/src/structural/partial_affine.rs` has another definition or use of the same name (suspected duplicate of `terminal_semantics::partial_affine_residuals_in_regions`, used at `:10135`; not read).
- `CallRegion` is `pub` (`:4335`) and re-exported (`lib.rs:85`); consumers not grepped (suspected stage 04).
- Comment `:3517-3519` and `:3737-3739` claim "One owner prepares the catalog" for `CleanupTraversal`; its single use is `cleanup_subjects` (`:3692`), 1 site.

Stated limits (quotes under 20 words).
- `:6248,6298` "projected reference custody is not yet supported" (two sites).
- `:5812-5821` "incoming reference leaf reconstruction capacity exceeded".
- `:7497` / `:5473` ("reference-bearing window requires one exact storage region").
- `:7075` "reference-bearing owned calls require a whole internal argument".
- `:5012` "ordinary call has no complete stored reference effects".
- `:3950-3956` comment on residual discards that "equalize a join".
- `:7175-7178` loans suspend only the captured projection; "extracting owned storage through a borrowed root still needs its explicit ownership-window operation."

Fail-open risks.
- `reference_sources.insert(parameter.place, None)` (`:197`) then `normalized_source` with `origins == None` errors via `.ok_or_else(invalid)` (`:6166-6169`): fail-closed (traced).
- `discard_subtree` returns `Ok(())` if `structural_source_contract` is `None` (`:6393-6395`). A discard of a place with no contract is accepted silently. Traced: callers `consume_owner_cleanup` (`:3900,4062,4091`) and `consume_edge_sources` (`:2556`) have already fetched a type or contract for the same place in the cleanup paths, but the `unrestricted` branch (`:3885-3906`) and `uninstalled_home` branch call it without an earlier contract lookup. Status: "open" (no comment states why accepting is right).
- `ValueUse::moving` returns `Ok(())` when the root is not `Owned` or the projection leaves inline storage (`:10429-10436`); `observing` returns `Ok(())` for non-owned root (`:10484`) and for paths leaving inline storage (`:10515-10528`, `else { return Ok(()) }`). Stated by structure: those are borrowed or heap-backed storage checked by references/loans instead (inferred, not commented). Status: "traced" partly (loans path in `references`), not fully.
- `AddressJoinPins::new` early-returns with empty pins when `address_joins.is_empty()` (`:1960-1966`); `exclusive_joins.is_empty()` same (`:2043-2053`). Correct if the join list is complete; depends on `is_address_join` (external, not read). "open".
- `shared_join_live_blocks` returning `None` means "all blocks dominated by owner are pinned" (`:2013`): fail-closed (pins more).
- `DescriptorLoans::new` returns `Self::default()` (no loans, no requirements) when `births.is_empty()` (`:921-923`). Accepts any later conflict check because there are no loans. Births come from enumerated operations (`:807-918`); a descriptor-forming operation not in that list would have no loan. Status: "open"; list covers `StructuralByteViewFieldStore`, stored call effects, `EstablishElementView` (shared or non-preserving), subslice patterns, structural call results with origins, `StructuralLeafCopy` of shared descriptors.
- `descriptor_loans::conflicted_operation` with `compatibility_requirement` returning a requirement instead of error (`:1126-1129`): defers the conflict to `validate_compatibility` which fails only when the residual reaches `unit.entry` or a provider candidate (`:750-761`). A residual on an internal machine passes. Stated by design? No comment. "open".
- `CallAccesses::written_regions`: for a machine callee with summary it replaces the written region with the summary's writes (`:3367-3392`); summary `None` falls back to `written_or_exclusive_regions` (`:3394`). A callee whose body was abandoned (`reconstruct` returned early) uses the conservative operation-level regions: fail-closed.
- `check_region_access_with` default `compatible_origin` is `|_| false`; fail-closed.
- `validate_atomic_access`: `location()` `None` returns `Ok(())` (`:8077-8079`): non-memory atomic events (fences) need no location; traced by structure.
- Join of `incoming` blocks: `ready.insert` only when `changed`; if `join_owned_frontier` leaves a stale snapshot, facts for blocks may be from the first visit: replaced per replay (`:9318-9322` comment) for returned references, but `snapshots.insert(BlockEntry)` is overwritten each visit (`:8222`). OK.
- Unreachable block check is by `blocks.keys().find(|b| !incoming.contains_key(b))` (`:9455`): rejects.

## Coverage

Read in full:
- `current_ownership.rs` 1-11455 (11455 lines, read in 8 chunks).
- `current_value_ranges.rs` 1-177 (177).
- `descriptor_results.rs` 1-3609 (3609).
- `error.rs` 1-391 (391).
- `lib.rs` 1-267 (267).
- `operation_metadata.rs` 1-683 (683).
Total 16582 lines read in full. Skimmed: none. Not read: none.
Files outside the unit that I only grepped, not read: `unit_validation.rs`, `candidates.rs`, `terminal_semantics` kernel, `structural_storage_queries`.

## Five things a maintainer most needs to know

1. `current_ownership.rs` is not a single concern: it is about 11,400 lines holding 11 inline modules (address-join pins, descriptor loans, scalar range relations, edge parameter transfer, cleanup, reference custody, residuals, storage regions, value custody, replay loop). The replay worklist is `replay::validate_current_ownership_cfg` (`:8174`); everything else is called from it.
2. Join admission is strict: two arrivals at a block must have equal live claims (`:9427`), equal owned places and live references; only the set of "moved paths" may differ, and only for no-code non-linear non-copyable descriptor-cleanup roots (`value_custody` `join_owned_frontier` `:10100`). A new ownership state added to `CurrentOwnership` (`:11365`) must be added to that join and to `snapshot` (`:11374`).
3. `descriptor_results::adapt` (`:1364`) returns `None` for the whole machine on many structural irregularities, and `CallAccesses::new` / `ReferenceCallEffects::new` / `AddressJoinPins::new` each rebuild descriptor graphs for the whole unit (`:3176`, `:4742`, `:165`); whole-unit graph builds happen at least 3 times per validation (read), with `unit_validation` possibly adding more (not read).
4. `operation_metadata.rs` matches every `AbstractOperation` variant with no wildcard in three functions, so adding an operation requires edits here (definitions, uses, ownership) in addition to the producer in `optimization-unit/src/construction.rs:656`.
5. Refusals are mostly `&'static str` reasons inside two catch-all variants (`CurrentCleanupMismatch` has no reason; `CurrentReferenceCustodyViolation { reason }`), and `Display` prints the Debug form (`error.rs:385-389`), so error identity for tests is textual.

## What I am unsure about

- Whether the early `Ok(())` returns in `discard_subtree` (`:6393`), `ValueUse::moving`/`observing` (`:10429,10484,10520`) and the empty `DescriptorLoans` (`:921`) can admit a wrong program; I traced only inside this file. The callers in `unit_validation.rs` and `is_address_join` (`structural_storage_queries`) were not read.
- Exact external call counts per exported function in `lib.rs`; I grepped groups only. `abstract_catalog_graph` and `optimized_borrowed_storage_catalog_graph` may be unused outside this file; a name-only grep was not run for each.
- Whether `partial_affine_residuals` in `psi/semantics/src/structural/partial_affine.rs` duplicates `residuals::selected_affine_residuals` (`current_ownership.rs:9498`); I grepped the name only.
- The "whole-unit graph built at least 3 times" claim is from reading `CallAccesses::new` (`:3176`), `ReferenceCallEffects::new` (`:4742`), `BorrowedRegions::origins` (`:165`, used only when `take_borrowed_origins` is empty); the third call may not run when `CallAccesses` already holds origins for the machine (`:92-103`).
