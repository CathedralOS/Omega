Second pass for unit 25

Ranges covered (gap_009):
- `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs` lines 1101-7100 (6000 lines)

All paths below are relative to `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/` unless stated. All line numbers are `current_ownership.rs`. I also read 1060-1100 and 7101-7105 for context; they do not count toward coverage.

The range begins inside `pub(crate) mod address_joins` (27) > `mod descriptor_loans` (691) > `impl DescriptorLoans` > `fn conflicted_operation` (1059, body starts at 1068). It ends inside `pub(crate) mod references` (3780) > free fn `apply_operation` is complete (5995-6989); the range ends in the middle of the module after `combine_alternative_origins` (7061-7105), inside `transfer_return` (7107, only its signature is cut off by 7100). Nothing in the earlier notes (unit_025.md) is contradicted for these lines; those notes only reach 1100.

Layout of what the range contains (module tree, all in current_ownership.rs):
- `address_joins::descriptor_loans` tail: 1101-1589
- `address_joins::range_relations`: 1591-1857
- `address_joins::AddressJoinPins` + helpers: 1858-2432
- `block_parameters`: 2433-2813
- `call_accesses`: 2814-3108
- `cleanup`: 3109-3689
- `continuation`: 3691-3732
- `mutations`: 3733-3779
- `references` (opens 3780): `access_paths` 3809-4163, `call_effects` 4164-4766, `windows` 4767-5037, free functions 5038-7105+.

---

## 1. `address_joins::descriptor_loans` (tail, 1101-1590)

### Purpose
Finish the loan-conflict checks for "descriptor loans" (shared-borrow descriptors that were born from byte views, element views and stored call effects, built earlier at 791-917). Decide, for one operation or one CFG edge, whether it touches storage that a live loan still protects. Also supplies the per-operation list of places an operation observes, and a per-position descriptor flow table.

### Shape
- `DescriptorLoans::conflicted_operation` (1059-1141): returns `Option<PlaceId>` of the disturbed place. Pushes `DescriptorCompatibilityRequirement`s into an out-vector instead of failing when `compatibility_requirement` says a requirement can satisfy the conflict.
- `DescriptorLoans::conflicted_edge` (1143-1229): same for an `OptimizationEdge` with the destination block's `StructuralParameterDeclaration`s.
- `DescriptorLoan::authorizes` (1233-1249).
- `operation_observations` (1252-1466): builds `Vec<Access>` (place, location, regions) per operation.
- `descriptor_flow` (1468-1503), `unknown_flow` (1505-1545), `descriptor_inputs` (1547-1588).
- Module re-export: `pub(crate) use descriptor_loans::validate_compatibility` (1590).

### Inputs and outputs
Reads `BorrowedRegions` (resolve, resolve_at, regions_overlap_at, writes_at, access_location, location_at, descriptor_flow), `PsiOptimizationFunction`, `StructuralTypeDeclaration` table, `storage_regions::{projected, canonical_field, written_or_exclusive_regions}`. Produces `Option<PlaceId>`, pushes into `requirements`, or returns `BTreeMap<Position, DescriptorFlowStep>`. No mutation of the unit.

### How it decides
- Operation conflict (1083-1139), keyed on position `(BlockId, node)`: for each loan live at the position, (a) if the operation's structural result place overlaps a backing region of the loan, return the result place (1088-1099); (b) for each observation (writes at the position, plus recorded observations when the loan is not `SharedBorrow`, 1101-1109) skip it if `loan.authorizes(...)` (1111-1117), else for each overlapping region pair call `compatibility_requirement`: `Some` pushes a requirement, `None` returns the place (1123-1126); (c) for each moved place whose resolved regions overlap a backing region return it (1130-1138).
- Edge conflict (1152-1228): loan must be live at both `(block,node)` and `(edge.target,0)` (1153); a binding is "disturbed" if shared-borrow argument against a non-shared loan (1162-1164), `Owned` argument into a non-`Unrestricted` parameter (1165-1168), or any exclusive borrow (1169); cleanup actions are added as forced `(place, empty projection, true)` (1178-1182). Forced entries conflict on any overlap (1194-1207); others go through `authorizes` then `compatibility_requirement` (1208-1224).
- `authorizes` (1233-1249): true only if the position has an authorized-place set that contains the access location, `regions` is non-empty, and every region has a root and is not `indirect`.
- `descriptor_inputs` (1547-1588): removes a place from the operation's inputs only for cleanup actions `DiscardRoot`/`RetireHome` on `Jump`, return and `ReturnStructural` operations, and only when the place is an operation-result, Affine, reading-borrow, shared-view descriptor (1554-1562).

### Cases by shape
- `operation_observations` early-returns an empty list for `ElementViewLength`, `ByteSequenceLength` and `jump_or_conditional_pattern!` (1261-1268).
- Explicit arms (1348-1457): byte-sequence concatenation/equal/predicate, `primitive_storage_observation_pattern!`, `StructuralScalarFieldRead`, `ElementViewRead`, `ByteSequenceRead`, `element_view_or_byte_subslice_pattern!`, `StructuralByteSequenceFieldLength/Read`, `call_structural_or_boundary_call_pattern!`, `EstablishReference/AccessReference/ElementView`; everything else `_ =>` uses `written_or_exclusive_regions` (1448-1456).
- `projected` (1284-1307) marks a projection `indirect` when any prefix of the path passes through a `Reference`, `ElementView` or borrowed byte view declaration (1296-1303), or the source has no contract (1289).
- `byte_field_projection` (1308-1333): a byte-sequence field counts as non-indirect only if its carrier is `ByteSequenceCarrier::BoundedOwned` (1325-1331).
- Call arguments: `indirect` unless `argument_storage_is_closed` (1428-1439).

### Same job elsewhere
- Per-operation "which places does this read/write" is also done by `operation_place_inputs` (imported 1859, defined in unit_validation) and `storage_regions::written_or_exclusive_regions` (used 1450). `operation_observations` re-lists the operation shapes again. Suspected overlap with `references::apply_operation` (5995-6989) which has a second hand-written list of the same observation operations (see section 8). Read both lists; they are not identical (e.g. `ElementViewRead` is in 1368 but not in the `apply_operation` match at 6020-6461).
- `unknown_flow` (1505) builds a conservative flow; the real one comes from `BorrowedRegions::descriptor_flow()` (1474).

### Unused or one-caller code
- `descriptor_flow` (1468) private; grep not run for it (only called inside its module, read).

### Stated limits
None quoted in this stretch.

### Fail-open risks
- 1069-1073: for `return_or_return_unit_pattern!()` operations the `moved` list is replaced by an empty slice, so a place moved by a return is not checked against live loans in this function. No comment at the site. What would get through: a return that moves a place still backing a live loan. Status: open (checking of normal returns is done elsewhere, `validate_normal_return`, not traced).
- 1476: `descriptor_flow().cloned().unwrap_or_else(|| unknown_flow(function))`: missing flow becomes a conservative `may_escape: true` flow with every structural place as input (1525-1530). Fail-closed, traced (read).
- 1160, 1276: `edge.structural_bindings.iter().zip(destination)` silently truncates if the binding list is longer than the destination parameter list; the extra bindings are never checked as disturbed. Status: open (length equality is presumably validated elsewhere; not traced).
- 1448-1456 default arm only counts written/exclusive regions; any operation that reads a place but is not in the explicit list falls to the "remaining" loop (1458-1464) which records a whole-place access, so reads are still treated as accesses. Traced (read), no wrong program seen.

---

## 2. `address_joins::range_relations` (1591-1857)

### Purpose
Turn a function's blocks and nodes into the step/edge graph that `terminal_semantics::scalar_ordering::ScalarOrdering` expects, so the ordering analysis can reason about `<`, `<=`, `not`, unsigned literals and storage reads/writes across the CFG, including guarantees from callee contracts.

### Shape
- `RangeRelations(ScalarOrdering)` (1598), `RangeRelations::new(function, functions)` (1600-1809), `into_ordering` (1810, `pub(crate)`), `ordering` (1814, `pub(super)`).
- `call_guarantees(operation, functions, scalar_types)` (1818-1856): asks `crate::descriptor_results::scalar_call_guarantees` for a callee's guaranteed propositions and instantiates them with `terminal_semantics::scalar_ordering::instantiate_contract_ordering`.

### Inputs and outputs
Reads the function blocks, `function.parameters`, block parameters, `function.verified_contract.requires`, and the map of all functions. Output is a `ScalarOrdering` built with `ScalarOrdering::with_contracts(steps, (function.entry,0), parameter types, requires, normal_returns)` (1795-1808).

### How it decides
- One `Step` per node position (1604-1766). Successors by operation: `Jump` one edge (1644-1646); `Conditional` two edges tagged with `(condition, true/false)` (1647-1665); `StructuralCase` one edge per case with no bindings and no condition (1666-1669); return patterns no successors (1670-1671); every other op falls through to `(block, index+1)` (1672-1683).
- `Effect` per operation (1694-1764): `IntegerLessThan/LessOrEqual` -> `Compare` (strict iff `LessThan`, 1710); `BooleanNot` -> `Not`; `IntegerConstant` with `IntegerValue::Unsigned` -> `UnsignedLiteral` (1718-1725); `EstablishPrimitiveLocal`, `PrimitiveLocalStore`, `WriteOnlyPrimitiveStore` with empty path -> store facts; `PrimitiveScalarRead` with empty path -> `ReadStorage`; reference establishment ops -> `None`; calls, `BoundaryCall`, `ExternalCompletion` -> `ForgetStorage`; any other op with non-empty `storage_mutations()` -> `ForgetStorage` (1762); everything else `None`.
- `call_guarantees` requires the value-to-type mapping to be unambiguous: it takes the first `(value, type)` row and requires all rows for the value to equal it (1826-1833).

### Cases by shape
- Only unsigned integer literals produce `UnsignedLiteral`; a signed `IntegerConstant` falls to `_ => Effect::None` unless it has storage mutations (1718-1725, 1762-1763).
- Stores/reads of storage are modelled only when the path is empty (1743, 1752).

### Fail-open risks
- 1620-1623 and 1623: the `edge` closure returns `None` when the target position `(target,0)` is missing or `function.block(target)?` fails; `.into_iter().collect()` / `.flatten()` then silently drops the edge (1646, 1651-1665, 1668). What input reaches it: a jump/conditional/case whose target block is absent or has no nodes. Dropping a successor can make the ordering analysis believe fewer paths exist. Status: open (unit is presumably validated before this runs; not traced).
- 1617: `.expect("position comes from block")` can only fail if the position list and block list diverge (they are built from the same function, 1604-1612). Traced.

### Unused or one-caller code
- Grep (count mode) `RangeRelations|into_ordering` over omega-rust: current_ownership.rs 6, descriptor_results.rs 2 (8 total). So `into_ordering` (pub(crate)) is used outside this file only via descriptor_results.rs (inferred from the file list; not read).

---

## 3. `AddressJoinPins` (1858-2432)

### Purpose
For functions that have "address join" block parameters (shared- or exclusive-borrow parameters that merge several borrowed origins at a block entry), compute which regions and places must stay undisturbed in which blocks, then check each operation and edge against those pins and against the descriptor loans.

### Shape
- `struct AddressJoinPins` (1873-1882, `#[derive(Default)]`): `borrowed_regions`, `descriptor_loans`, `shared: BTreeMap<BlockId, Vec<Region>>`, `exclusive: BTreeMap<BlockId, BTreeSet<PlaceId>>`, `origins`, `dominators`, `readers`.
- `new` (1896-2143), `reference_sources` (1885), `descriptor_requirements` (1889, delegates to `descriptor_loans.requirements()`), `check_operation` (2145-2181), `check_edge` (2183-2300), `read_after` (2302-2320).
- Free fns: `pinned_root_disturbed` (2323), `entirely_descends_from` (2335), `shared_join_live_blocks` (2357-2431).

### Inputs and outputs
`new` reads the function, the machine map, the structural type table, and a `&mut CallAccesses` (for borrowed origins, via `BorrowedRegions::new`, 1902-1903). It also calls `crate::candidates::global_value_numbering::independent_reachable_dominators(function)` (1975-1978) and `function_structure::current_mutable_borrows` (2084-2087). Output is a populated struct. `check_*` return `Result<(), OptimizationUnitValidationError>`; all failures are `CurrentSharedJoinOriginDisturbed(OperationSite, PlaceId)` via `pinned_root_disturbed` (2323-2333).

### How it decides
- Joins: shared-borrow block structural parameters for which `is_address_join` holds (1906-1921). Origins: for every edge binding into a join, the argument place joins the parameter's origin set (1922-1932), then a second pass for all address-join parameters (1949-1958), closed transitively by a fixpoint loop (1959-1974). Early return with only `borrowed_regions` and `descriptor_loans` filled when there are no address joins at all (1942-1948).
- Shared pins (1988-2002): for each shared join, live blocks from `shared_join_live_blocks`; every block that equals the owner block or is dominated by it, and (if live info exists) is live, receives the join's resolved regions as pins.
- Exclusive pins (2088-2133): for each exclusive join with owned `OperationResult` roots (2090-2105), suspended places = roots + descriptor places aliased to those roots that do not "entirely descend" from the join (2109-2124); applied to every block where `available.may_be_live(block, join)` (2125-2132).
- `check_operation` (2145-2181): (1) descriptor-loan conflict, (2) exclusive pins: any `operation_place_accesses` place in the pinned set, (3) shared pins: `borrowed_regions.conflicted_operation(pinned, (block,node), &moved)`.
- `check_edge` (2183-2300): (1) descriptor-loan conflict; (2) for each binding into an exclusive-borrow destination parameter whose source is an `Owned` structural source (2202-2211): the owner must be live (Affine: in `frontier.owned_places` as Affine; Unrestricted: true; Linear: false, 2212-2219), and must have no partial custody paths, no claims, no live references rooted/carried at it, exactly one binding for the place or its aliases, and no cleanup action naming it (2220-2248); (3) exclusive pins vs bindings and cleanup actions; `DiscardRoot`/`RetireHome` only count if `read_after` (2250-2269); (4) shared pins vs disturbing bindings/cleanups that overlap and are read after the edge (2270-2299).
- `read_after(place, target)` (2302-2320): names = place + every place whose origin set contains it; true if some reader block is `target` or is dominated by `target`... more exactly `dominators[reader].contains(target)`.
- `shared_join_live_blocks` (2357-2431): collects blocks that mention the join (or an alias) as a block parameter or operation input; any use whose operation might retain the alias returns `None` (2416-2418). "Cannot retain alias" is a fixed list: scalar storage observations, byte/element length, `ByteSequenceRead`, `byte_len_or_field_read_pattern!`, and jump/conditional where every use is a shared-borrow binding into a name in the set (2384-2415). Result is the live set closed backwards over predecessors (2422-2430).

### Cases by shape
- The "cannot retain alias" list at 2384-2415 is the fixed list of operations allowed to touch the join in the narrow-live-region path; anything else gives `None`, which `new` turns into "pin all dominated blocks" (1995 `is_none_or`).
- `check_edge` exclusive-owner checks apply only when the destination parameter is exclusive-borrow and the source place is `Owned` (2202-2211).

### Same job elsewhere
- The transitive-closure fixpoint over `BTreeMap<PlaceId, BTreeSet<PlaceId>>` is written twice in `new`: 1959-1974 (`origins`) and 2069-2083 (`aliases`), same shape.
- Dominators come from `candidates::global_value_numbering::independent_reachable_dominators` (1975-1978, 2799 as well). That is one outside user in this range beyond candidates.rs, plus `parameter_establishment_order` (2798-2799), plus two in current_value_ranges.rs and others per earlier notes.
- `entirely_descends_from` (2335) is a recursive all-parents check with an `active` cycle guard; suspected similar to the transitive loops in `BorrowedRegions` (not read in this range).

### Unused or one-caller code
- `AddressJoinPins::reference_sources` (1885) and `descriptor_requirements` (1889): grep not run; `pub(super)` and delegating only.

### Fail-open risks
- 2154-2161 and 2192-2199: `conflicted_operation`/`conflicted_edge` are called with `&mut Vec::new()`; any `DescriptorCompatibilityRequirement` they push is discarded here, and `None` is returned for that conflict (no error). Requirements are instead exposed through `descriptor_requirements()` (1889-1894). Status: traced for this file (read both sides of the hand-off); who consumes `descriptor_requirements` was not read in this range.
- 2171-2173 / 2270-2272: no shared pin for the block means `Ok(())`. Intended (no joins reach that block). Traced.
- 2207-2211: a binding into an exclusive-borrow parameter whose source has no contract or is not `Owned` is skipped (`continue`) from the owner-liveness checks. Source borrowed from elsewhere is checked by other pins. Status: open (not traced).
- 2160, 2198 as above; 2202 `zip(destination)` truncates silently when the lists differ in length (same remark as in section 1). Open.
- `shared_join_live_blocks` returning `None` is conservative (wider pinning). Traced.

---

## 4. `block_parameters` (2433-2813)

### Purpose
Track how owned structural values, claims and live reference carriers move across a CFG edge into the target block's structural parameters, so the ownership frontier after the edge is exact.

### Shape
- `EdgeTransfer { claims: Vec<(ClaimId, LiveClaim)> }` (2448).
- `consume_edge_sources` (2452-2646): removes sources from the frontier and returns the claims to re-key onto parameters.
- `consume_projected_source` (2648-2745): the same for an argument with a non-empty path (partial move out of an Affine owned place).
- `establish_edge_parameters` (2747-2785): inserts the target's owned parameters and the transferred claims.
- `parameter_establishment_order` (2787-2812, `pub(crate)`): function parameters, then block parameters ordered by dominator-set size.

### Inputs and outputs
Mutates `&mut CurrentOwnership` (fields used: `owned_places`, `partial_custody_paths`, `claims` (`LiveClaim{input, path, multiplicity}`), `live_references`). Reads `function.block_entry_claims`, `structural_source_contract`, `references::{copy_reference_carriers, discard_owned, discard_subtree, check_region_access, borrows_owned_storage, owned_resident}`. Errors are all `CurrentCleanupMismatch { machine, block }` (2461, 2656, 2754).

### How it decides
- Per binding, keyed on the destination parameter's `access`/`multiplicity` and the argument's path:
  - non-`Owned` parameter: skipped (2468-2470);
  - `Unrestricted`: source must be an Unrestricted owned source not in `owned_places`, no partial paths, no claims (2473-2484); references are copied with `copy_reference_carriers` at the block's last node (2485-2499); `retains_source` is `source == parameter.place && path.is_empty()` (2500);
  - non-empty path: `consume_projected_source` (2508-2512);
  - whole value: discard owned references (2517-2526), remove from `owned_places` requiring equal multiplicity (2527-2532), and move claims whose `input == source` (2533-2551): Linear with no claims or Unrestricted with claims is an error (2538-2543).
- Claim re-keying (2588-2645): if the parameter has catalogued entry claims (`function.block_entry_claims` for that parameter place), each transferred claim must match exactly one active catalogue row by path (2618-2622), multiplicity must equal parameter multiplicity (empty path) or `Linear` (non-empty) (2623-2630), cases excluded by `case_is_excluded` are removed from the active catalogue (2598-2609); any active row left over, or a count mismatch, is an error (2638-2644).
- `consume_projected_source` (2648-2745): argument must be `Owned`, parameter Affine or Linear, source in `owned_places` as Affine, not `content_entry_claim_declared`, and `resolve_structural_path(source type, path) == parameter.structural_type` (2661-2679). Claims with a prefix match move with a suffix path; a claim whose path is a prefix of the argument path is an error (2685-2692). Linear parameter needs exactly one empty-path Linear claim; Affine needs none at empty path; Unrestricted is rejected (2694-2704). Moved paths must not overlap an existing partial path (2732-2737).
- `parameter_establishment_order` (2787-2812): if no block has structural parameters, returns function parameters only (2791-2797); otherwise appends block parameters sorted by `dominators.get(&block.id).map_or(0, len)` (2805), i.e. by number of dominators.

### Cases by shape
- Edge handling is split into exactly three shapes: Unrestricted copy, projected move, whole move (2472-2551).
- Claim transfer assumes at most one claim set per `(parameter, path)`; the `bound` list rejects duplicates (2633-2635).

### Same job elsewhere
- The terminal-node ordinal "last node of block" is derived at 2485-2488 and 2715-2718 with `checked_sub(1)` (error on empty block) and at 3488-3491 and 3580-3583 with `saturating_sub(1)` (ordinal 0 on an empty block). Grep (`nodes\.len\(\)\.(checked|saturating)_sub`) in current_ownership.rs: exactly those 4 sites.
- Error constructor `CurrentCleanupMismatch { machine, block }`: grep over current_ownership.rs finds 14 sites (7 in this range: 2461, 2656, 2754, 3178, 3384, 3532, 3708; the rest at 7984-8737 outside this range). Each is built inline with its own closure.

### Unused or one-caller code
- Grep (count) `parameter_establishment_order` over omega-rust: 4 occurrences, all in current_ownership.rs: definition 2787, import 3114, call 3223 (in `cleanup_root_order`), and one more occurrence I did not locate (outside my range or unread line). So the `pub(crate)` function has no user outside this file.

### Stated limits
None quoted.

### Fail-open risks
- 2554-2569: reference carriers that move with an Owned binding are re-carried onto the parameter place; a reference with no matching binding is left unchanged (`continue`, 2564-2566). Status: traced (read); an unmatched reference stays carried at the old place, which later checks treat as live custody.
- 2467 / 2554 / 2589: `bindings.iter().zip(&target.structural_parameters)` silently truncates; final `bound.len() != transferred.len()` check (2642) covers only claims. Open.
- 3 `expect`: 2548 `.expect("captured live claim")` after collecting identities from the same map; cannot fail unless the map changed in between (it is not). Traced.

---

## 5. `call_accesses` (2814-3108)

### Purpose
Summarise, for every function, which of its structural parameters it may write to and with what projection, so a `CallStructural` call can be treated as writing exactly those regions in the caller. Also carries the descriptor view graph and the reference-call-effects table for the whole unit.

### Shape
- `ParameterMutation { position, projection }` (2830), `FunctionMutations { parameters, writes }` (2836).
- `CallAccesses<'unit>` (2841-2848, `pub(crate)`): `functions: BTreeMap<MachineId, Option<FunctionMutations>>`, `boundaries`, `dynamic_dispatch`, `references: ReferenceCallEffects` (`pub(super)`), `view_graph: Option<Vec<DescriptorMachine>>`, `borrowed_origins`.
- `new` (2851-2913), `boundaries` (2915), `view_graph` (2919), `has_descriptor_call_origins` (2923), `take_borrowed_origins` (2933), `reconstruct` (2940-3046), `written_regions` (3048-3082), free fn `argument_storage_is_closed` (3085-3107, `pub(crate)`).

### Inputs and outputs
`new` builds `ReferenceCallEffects::new` (2858-2863), the descriptor catalog graph via `crate::descriptor_results::optimized_catalog_graph` (2865-2871), a `DescriptorAnalysis` (2872-2877), then `BorrowedRegions::origins_with_analysis` per function (2878-2892), then runs `reconstruct` for every machine (2901-2910). Output: the populated struct; `take_borrowed_origins` removes one machine's `BorrowedRegions` (consumed by `BorrowedRegions::new`, per earlier notes).

### How it decides
- `reconstruct` (2940-3046) memoises with `self.functions.insert(machine, None)` before recursing (2951), so recursion yields `None` summaries. It returns early (leaving `None`, meaning "no summary") when: the function or its origins are missing (2952-2956); a `CallStructural` callee has no `machine()` or its own summary is `None` (2959-2967); the function contains `Call`, `AtomicEvent` or `PortWrite` (2968-2973); a `BoundaryCall` has an Owned argument, an argument whose storage is not closed, completion claim sources or receipts (2974-2997); any cleanup roster contains `InvokeNominal` (2998-3010); a written region resolves to a root that is not one of the function's structural parameters (3015-3021), or is an `Owned` parameter (3022-3026).
- `written_regions` (3048-3082): if the operation is a `CallStructural` with empty `claim_transfers` and the callee has `Some` summary with matching parameter count (`summary.parameters == structural_arguments.len()`), return the summary's writes mapped through the actual arguments (`Projection::new(&actual.path).then(&write.projection)`); otherwise fall back to `written_or_exclusive_regions` (3081).
- `argument_storage_is_closed` (3085-3107): resolve the path, then for a non-Owned whole-argument borrowed byte sequence return `true` (3097-3098), for an `ElementView` ask `storage_cannot_retain_loan` on the element (3100-3102), else `storage_cannot_retain_loan(selected)`.

### Cases by shape
Summary exists only for functions made of: no ordinary `Call`, no `AtomicEvent`, no `PortWrite`, boundary calls with only borrowed closed-storage arguments, no nominal cleanup, and writes only through borrowed (non-Owned) structural parameters. Everything else gets `None`.

### Same job elsewhere
- `ReferenceCallEffects::new` (4187) and `CallAccesses::new` both build a catalog graph with different entry functions (`optimized_reference_catalog_graph` at 4197 vs `optimized_catalog_graph` at 2865) and both construct `DescriptorAnalysis` objects (2873 vs 4206).
- `argument_storage_is_closed`: used at 1431 (descriptor_loans), 2984, and elsewhere in this file. Grep (count) `argument_storage_is_closed` over omega-rust: current_ownership.rs 5, descriptor_results.rs 1 (6 total).

### Unused or one-caller code
- `has_descriptor_call_origins` (2923): grep over omega-rust: one caller, unit_validation.rs:12837. Definition at 2923.
- `view_graph()` / `boundaries()` accessors (2915, 2919): grep of `\.view_graph\(\)|\.boundaries\(\)` (combined with `has_descriptor_call_origins`) gives unit_validation.rs 3, current_ownership.rs 2, `psi/.../06_checked-trees-to-lowered-psi/src/attached_unit.rs` 1. The attached_unit.rs hit is a different object with similar method names (suspected, not read).

### Fail-open risks
- 3081: functions with no summary get the generic `written_or_exclusive_regions` instead of a rejection. That is the pre-summary model (arguments passed with exclusive access count as written), so it is the conservative route. Traced (read).
- 2964-2966: recursion and mutual recursion produce `None` summaries for the whole cycle, as analysed above. Traced.
- 3062-3063: summary used only when argument counts match; mismatch falls back (conservative).

---

## 6. `cleanup` (3109-3689)

### Purpose
Check that the affine cleanup actions on a return or edge (`DiscardRoot`, `DiscardResidual`, `RetireHome`, `InvokeNominal`) exactly match what the current frontier owes, and apply them to the frontier.

### Shape
- `CaseMembership<'a> = dyn Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId, bool) -> bool` (3132-3133).
- `validate_return_cleanup_actions` (3136-3191, `pub(super)`).
- `cleanup_root_order` (3193-3235): root order for cleanup: operation-result places by producer descending, then trivial affine locals by declaration ordinal descending, then parameters in reverse establishment order.
- `CleanupSubject { path, structural_type, nominal }` (3237), `cleanup_subjects` (3243-3329), `append_cleanup_subjects` (3331-3372).
- `consume_owner_cleanup` (3374-3522), `consume_nominal_cleanup` (3524-3596), `valid_nominal_cleanup` (3598-3660), `bounded_nominal_cleanup_receiver_shape` (3662-3688).

### Inputs and outputs
Reads `CurrentOwnership`, `functions`, the type table, `value_custody::uninstalled_home`, `references::{discard_owned, discard_subtree, check_region_access, validate_normal_return, owned_resident}`, `terminal_semantics::owned_structure::*` and `selected_affine_residuals` (from `residuals`). Mutates a cloned or `&mut` frontier. Errors: `CurrentCleanupMismatch` throughout.

### How it decides
- `validate_return_cleanup_actions` (3146-3190): clone the frontier, apply cleanup through `continuation::apply_edge_cleanup_actions` (3147-3155), then drop from `owned_places` every `Linear` place that has no partial paths, no claims and whose root type validates with an empty claim frontier (`validate_owned_claim_frontier(..., &|_, _| true)` returns `Ok`, 3156-3173). The place stays if `place_structural_type` is `None` (3163). Then `owned_places`, `partial_custody_paths` and `claims` must be empty (3174-3182), then `validate_normal_return` on the remaining live references (3183-3189).
- `consume_owner_cleanup` (3374-3522): three situations. (a) uninstalled home: actions must be exactly `[RetireHome(place)]` with no claims or owned-resident carrier references (3388-3404). (b) place not in `owned_places` (already fully moved): actions must be `[RetireHome(place)]`, `projected_root_is_fully_consumed`, no carried references (3405-3419). (c) owned: strip a trailing `RetireHome(place)` as `explicit_retirement` (3424-3431); otherwise the whole action list must be a single `DiscardRoot(place)` or a single whole-place `InvokeNominal` with `!retains_descendants` (`intrinsic_retirement`, 3432-3433); neither -> mismatch (3434-3436). Then `cleanup_subjects` must produce exactly as many subjects as actions (3437-3449) and each action must match its subject by variant, path and type (3450-3510). After, no claims may remain (3512-3518); place removed from `owned_places` and `partial_custody_paths` (3519-3520).
- `cleanup_subjects` (3243-3329): subjects are the path/type pairs that need separate cleanup. When some path moved out (`moved`), any `Case` membership failing returns `None` (3259-3262), and any ancestor that is a nominal type returns `None` (3265-3268). If no moved paths and no selected nominal cleanup inside the type, a single whole-root subject is returned (3273-3289; `limit != 0` required). Otherwise boundaries = nominal types plus Sum/Mixed types that have a non-erased structural field (3290-3308), and subjects come from `selected_affine_residuals` (3310) then `append_cleanup_subjects` with limit `actions.len()` (3313-3327).
- `consume_nominal_cleanup` (3524-3596): requires empty `requirement_obligations`, `selected == cleanup.structural_type`, `valid_nominal_cleanup`, no overlapping partial path, all overlapped claims on the exact path and `Linear`, empty path => Affine has no claims / Linear exactly one / Unrestricted rejects, non-empty path => at most one claim, `consumed_claims` equal the claims in order (3550-3579).
- `valid_nominal_cleanup` (3598-3660): the cleanup machine must differ from the caller, the receiver type must be a bounded-record shape, `!retains_descendants`, target `attachment` and `nominal_cleanup_subject` equal the structural type, exactly one function in the unit has that `nominal_cleanup_subject` (3616-3622), consumed claims sorted strictly ascending, the target returns Unit with no scalar parameters, structural parameters are `[]` (no receiver, no structural places) or exactly one position-0 `MutableBorrow`/`WriteOnlyBorrow` parameter whose place is the `cleanup_receiver` and declared as a `Parameter` place, no entry claims, published service ceiling subset of the caller's, and no crash routes in `verified_contract` (3650-3659).

### Cases by shape
- Fixed receiver shape (3662-3688): record only; every live field must be `Boolean`, or an integer of 8/16/32/64 bits where an address integer must be 64 bits (3669-3675); floats, atomics, byte sequences, structural fields, erased, refined and bounded integers all return `false` (3676-3685). Cleanup receivers of any other shape cannot be validated.
- Return cleanup action forms: `[RetireHome]`, `[DiscardRoot]`, `[InvokeNominal]` (whole place), or a partition plus `RetireHome` (3424-3436).
- Cleanup group order: `apply_edge_cleanup_actions` (see 7) walks groups of equal `place()` against the order from `cleanup_root_order`.

### Same job elsewhere
- `cleanup_subjects`/`append_cleanup_subjects` depend on `terminal_semantics::owned_structure` (3125-3129); a similar "which sub-values need separate disposal" job exists in the terminal semantics crate (suspected, not read; names imported at 3125-3129).
- Return-path frontier clearing here and edge-path transfer in `block_parameters` both hand-code "nothing owned may remain" rules.

### Unused or one-caller code
- `valid_nominal_cleanup` (3598): grep over omega-rust (content mode): 2 hits, definition 3598 and one call at 3552.
- `bounded_nominal_cleanup_receiver_shape` (3662): 2 hits, definition 3662 and one call at 3611.
- Combined grep `transferred_ingress_leaf|window_hole|entry_references|valid_nominal_cleanup|bounded_nominal_cleanup_receiver_shape`: current_ownership.rs 16, `psi/interpreter/terminal-interpreter/src/engine_structural.rs` 3, `omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow.rs` 2. The two outside files are not read; the match could be a same-named function there (suspected, not verified).

### Stated limits
- 3662: function name `bounded_nominal_cleanup_receiver_shape` plus the shape list above is the limit; no comment.

### Fail-open risks
- 3156-3173: a `Linear` owned place at return that has no claims or partial paths is removed from the "still owed" set when the empty-claim frontier of its type validates, and its closure `&|_, _| true` treats every case as selected. What gets through: a function that returns while holding a Linear place whose type owes nothing under an empty claim frontier. No comment at the site. Status: open; direction of the `true` membership (more cases selected = more obligations) suggests it errs toward keeping the place, inferred, not traced into `validate_owned_claim_frontier`.
- 3388-3404 / 3405-3419: a place with no owned entry is accepted if the actions are exactly `[RetireHome(place)]` and `projected_root_is_fully_consumed`. Traced for the first part (read); `projected_root_is_fully_consumed` (in `structural`) was not read.
- 3490, 3582: `saturating_sub(1)` makes an empty block yield ordinal 0 instead of an error (versus `checked_sub` at 2487/2717). Input: a block with no nodes reaching cleanup validation. Status: open.

---

## 7. `continuation` (3691-3732) and `mutations` (3733-3779)

### Purpose
`continuation::apply_edge_cleanup_actions`: apply a cleanup action list to the frontier in the canonical order. `mutations`: two small frontier mutators.

### Shape
- `apply_edge_cleanup_actions` (3699-3731): `cleanup_root_order(function, frontier)` gives an iterator; for each `chunk_by` run of equal `action.place()`, the place must be found by `eligible.any(...)` in the remaining order (so groups must appear in order, 3714-3718), then `consume_owner_cleanup` (3719-3728).
- `mutations::insert_owned_result` (3740-3759): `Unrestricted` results are not tracked (`return Ok(())`, 3748-3750); a duplicate owned insert is `CurrentOwnedPlaceNotLive` (3751-3756); clears partial paths (3757).
- `mutations::reject_live_linear_claim` (3761-3778): any live claim with `multiplicity == Some(Linear)` gives `CurrentLinearClaimAtReturn`.

### How it decides
Keyed on place identity and ordering: `eligible.any(|candidate| candidate == place)` consumes the iterator, so a later group must come after the earlier one in `cleanup_root_order` (3716).

### Fail-open risks
- 3748-3750: `Unrestricted` results are not inserted into `owned_places`, so they are never tracked. Intended (unrestricted values need no disposal). Traced (read).

### Unused or one-caller code
Not grepped for these (all `pub(super)`).

---

## 8. `references` module (3780-7105)

### Purpose
Model reference custody: every live reference (a descriptor stored in a place at a path) with its root origin and parent, and update that set operation by operation (`apply_operation`), across calls (stored reference effects), windows (field restoration), cleanup (`release`/`discard_*`) and returns (`transfer_return`, begins 7107). Reject any operation that touches a suspended referent or treats a carrier as owned data.

### Shape
- Constant `MAX_INCOMING_REFERENCE_LEAVES = 4096` (3800); `enum CallRegion { OrdinaryStorage, CarrierStorage, Referent }` (3803).
- `access_paths` (3809-4163): `AccessReferenceCustody { origins, dependencies }` (3821), `access_reference_custody` (3826-4004), `descriptor_candidates` (4006-4058), `descriptor_storage_available` (4060-4141), `call_dependency_conflict` (4143-4162).
- `call_effects` (4164-4766): `SlotEffect`, `SlotSource::{Incoming, BorrowedStorage}` (4171-4179), `ReferenceCallEffects { functions: BTreeMap<MachineId, Option<Vec<SlotEffect>>>, declarations }` (4181), `new` (4187-4292), `remaining_at_return` (4294-4418), `stage` (4421-4615), `slot_type` (4618), `resolve_slot` (4632-4686), `StoredReferences` (4688) with `commit` (4695-4764).
- `windows::transfer` (4774-5036).
- Free: `origin_place` (5051), `projected_origin`/`projected_origin_family` (5060-5135), `origins_overlap` (5137), `invalid` (5164), `owned_resident` (5178), `borrows_owned_storage` (5193), `storage_cursor` (5225), `entry_references` (5254), `operation_structural_arguments` (5295), `window_hole` (5305), `check_root_access` (5336), `check_region_access` (5346), `check_region_access_with` (5357), `check_record_field_access` (5399), `check_observation_access` (5429), `shared_source_compatible` (5473), `shared_borrow_compatibility` (5492), `copy_reference_carriers` (5506), `normalized_source` (5598), `normalized_direct_source` (5650), `release` (5755), `discard_owned` (5806), `discard_subtree` (5816), `establish_record` (5878), `establish_constructor` (5932), `apply_operation` (5995-6989), `transferred_ingress_leaf` (6991-7028), `combine_alternative_references` (7030-7059), `combine_alternative_origins` (7061-7105).

### Inputs and outputs
`LiveReference { identity, carrier, carrier_path, root, parent }` and `ReferenceOrigin::{Invocation, Primitive, IngressLeaf, Alternatives}` come from `unit_validation::references` (imports 3785-3791). `apply_operation` mutates `live: &mut Vec<LiveReference>` and returns `Result<(), OptimizationUnitValidationError>`; every refusal is `CurrentReferenceCustodyViolation { machine, block, node, reason: &'static str }` built by `invalid` (5164-5176).

### How it decides
- Identity and keying: carriers are `(place, path)`; roots are `ReferenceOrigin` values compared structurally; overlap between origins is `origins_overlap` (5137-5162): any `Invocation` origin overlaps everything (5138-5142), different places never overlap (5143), `Primitive` and `Invocation` overlap any same-place origin, `IngressLeaf`/`Alternatives` compare projection paths with `reference_origin_paths_may_overlap` (5146-5161).
- `access_reference_custody` (3826-4004): walks an argument path across each `Referent` segment; at each crossing finds candidate descriptor carrier paths (`descriptor_candidates`, expanding `RuntimeIndex` into every index of a literal-length `FixedArray`, bounded by `maximum = live.len()` 4013-4035), requires the carrier type to equal the expected reference type (3915), requires `descriptor_storage_available` (3918), requires exactly one live carrier at that `(place, path)` (3929-3933), and rejects if any child of that carrier is incompatible with the requested access (3934-3946). Uses `cursor_budget = live.len() * path.len()` to bound further cursors (3870-3873, 3981-3983).
- `descriptor_storage_available` (4060-4141): a loan on the same place conflicts unless it is an ancestor, or compatible shared use (4077-4082), or its referent type differs from the type at the storage (4106-4116); `Invocation` roots never conflict here (4127).
- `call_effects::new` (4187-4292): per function, the stored effects from `DescriptorAnalysis::stored_effects`; any effect whose parameter, path or origin cannot be expressed makes the entire function's effects `None` (the `Option<Vec<_>>` collect, 4283, and `?` inside). Origins kept: `IncomingSlot` -> `SlotSource::Incoming`; `Parameter` with exact non-indirect projection and matching referent type -> `BorrowedStorage` (4253-4270). `Invocation`, `IncomingContents`, `Provider`, `Empty`, `Literal`, `AccessFormation` -> `None` (4271-4274), which drops all effects of that function.
- `remaining_at_return` (4294-4418): for each effect, builds candidate `(root, parent, identity)` tuples and requires exactly one live reference at the destination whose root(s) match a candidate; matched destination references are removed from `expected` and `remaining`. No effect summary: returns all of `live` as remaining (4302-4308).
- `stage` (4421-4615): for a `CallStructural` with no effect summary: error `"ordinary call has no complete stored reference effects"` when any argument could have writable reference slots, or its path type is unknown (4446-4466); otherwise `Ok(default)`. With a summary: destination must be exclusive (`MutableBorrow`/`WriteOnlyBorrow`, 4484-4491) and have at most one resident (4494-4507); sources become alternatives; the replacement must not duplicate another replacement's slot (4592-4599) or reuse a historical parent identity (4600-4604). `commit` (4695-4764) rejects replacements whose root overlaps a publication unless both are `SharedBorrow` (4734-4745) or reuses an identity (4747-4757).
- `windows::transfer` (4774-5036): resolves the window hole, finds origins through `access_reference_custody` when the hole crosses a `Referent`, and for whole-referent restoration additionally requires `structural_window_origins` support (4816-4834). If the moved value has no reference leaves it only checks for overlap with live descriptors (4911-4923); otherwise it requires exactly one exact storage region (4924-4928), relocates or (for Unrestricted owned copies, `copies_storage`) copies each leaf (4938-5033).
- `apply_operation` (5995-6989) is the per-operation dispatch, described in "Cases by shape" below. After the match it normalises call arguments into `(origins, access, CallRegion, dependencies, argument_position)` rows (6463-6632) and rejects any pair that overlaps through origins/dependencies unless both are shared (6633-6670), stages stored effects (6671-6681), builds `sources` for reference-establishing operations (6682-6740), and either replaces moved references with the established ones (6741-6983) or just drops the moved ones (6984-6986), then commits stored references (6987).

### Cases by shape
- `apply_operation` match (6020-6462), one arm per fixed operation list: `AtomicEvent` (location with a `Referent` goes through custody, else `check_observation_access`, 6021-6064); `ReleaseReference` (6065-6075); `EstablishCase` (root access for each structural field, 6076-6082); `EstablishStructuralArray` (6083-6100); `MoveStructuralValue`/`TransformContent` (6101-6124); `EstablishRecord` (6125-6135); `StorageAddress`/`PrimitiveScalarRead` with a `Referent` in the path (6136-6152); `primitive_storage_observation_pattern!` (6153-6171); `StructuralScalarFieldRead` (6172-6195); `StructuralCase` (6196-6207); `PrimitiveLocalStore` (6208-6210); `byte_len_or_field_read_pattern!` (6211-6229); byte-sequence concat/equal/predicate (6230-6273); `WriteOnlyPrimitiveStore` (6274-6307); `StructuralScalarFieldStore` (6308-6325); `StructuralByteSequenceFieldStore`/`ByteStore` (6326-6363); `MoveStructuralField` and `StoreStructuralField` via `windows::transfer` (6364-6413); `StructuralLeafCopy` only when the result type contains references (6414-6460); every other operation: `_ => {}` (6461).
- Arguments considered for the overlap and stored-effect logic: `operation_structural_arguments` (5295-5303) returns arguments only for `call_structural_or_boundary_call_pattern!` and `StoreStructuralField`'s `value`; every other operation gives `&[]`.
- Reference-bearing owned arguments: the path must be all `Field` segments and a declared record edge (6506-6520); `BoundaryCall` with a reference-bearing owned argument is rejected (6496-6500).
- Return-value reference results: only `EstablishReference`, `EstablishAccessReference` and `CallStructural` returning a structural result whose type contains a reference (6683-6740).
- Fixed capacity: at most 4096 incoming reference leaves, `leaf_paths(... , MAX_INCOMING_REFERENCE_LEAVES - live.len())` (5263-5275); `leaf_paths` capacity is `live.len()` in most other calls (3-5 sites: 4885, 5533, 5837, 5957, 6521).

### Same job elsewhere
- Two parallel models of "what does this reference/descriptor point at": `address_joins::borrowed_regions::{Region, Projection}` with `regions_overlap_at` (used 1120, 1133, 1197, 1213) and `references::{ReferenceOrigin, ReferenceOriginProjection}` with `origins_overlap` (5137). Both answer "do these two storage regions overlap". Read both; they are separate tables and separate functions. Suspected duplicate job.
- Two operation-by-operation lists of what each operation reads or writes through descriptors: `operation_observations` (1252-1466) and `apply_operation` (5995-6461). Lists differ as noted in section 1.
- `windows::transfer` (relocate leaves) and `establish_constructor` (5932-5992, relocate leaves into a constructed value) and `copy_reference_carriers` (5506-5596, copy leaves): three hand-written "move or copy the set of live reference leaves from one carrier path to another" routines, each with its own leaf-roster check (`leaf_paths`, then "roster differs from its type" error at 4996, 5982-5984, 5593).
- `descriptor_candidates` (4006) does path expansion that `leaf_paths` (imported, unit_validation) also does for types; not identical (index expansion vs leaf enumeration).
- Alternatives merging: `combine_alternative_references` (7030) and `combine_alternative_origins` (7061) both required to agree on place and parent shape.

### Unused or one-caller code
- `transferred_ingress_leaf` (6991, `pub(super)`): grep content mode: definition 6991, calls at 6685 and 7688 (2 call sites, both in current_ownership.rs).
- `entry_references` (5254, `pub(super)`): included in the combined grep above; not separated.
- `window_hole` (5305, `pub(super)`): same, not separated.
- `check_root_access` (5336): not grepped.

### Stated limits (quoted strings, each under 20 words)
- 5701 and 5751: "projected reference custody is not yet supported".
- 5712: "reference projection has no live supplying descriptor".
- 5708: "reference carrier is no longer live".
- 4464: "ordinary call has no complete stored reference effects".
- 6498: "reference-bearing owned calls require a whole internal argument".
- 6519: "projected reference argument path is not a declared record edge".
- 6767: "reference call result leaf is unsupported".
- 5273: "incoming reference leaf reconstruction capacity exceeded".
- 6672 wording no limit.
No `TODO`/`unsupported` comments appear in this range (read, no grep run for comments).

### Fail-open risks
- 6461 `_ => {}` in `apply_operation`: operations not named in the match perform no access check in this function, and since `operation_structural_arguments` (5295) returns `&[]` for them, they also skip the argument overlap logic and stored-effect staging (`stage` returns `Ok(default)` for any non-`CallStructural`, 4433-4440). Operations that fall here include, by reading the lists: `ElementViewRead`, `ByteSequenceRead`, `EstablishElementView`, `EstablishPrimitiveLocal`, `ElementViewLength` and ordinary scalar operations (those with no structural places). What wrong program could get through: a read through a suspended referent by one of the unlisted structural operations. Status: open (the same operations are listed in `operation_observations` 1368-1406, so descriptor-loan checks do cover them; other checks outside this file were not traced).
- 5824-5826: `discard_subtree` returns `Ok(())` and does nothing when `structural_source_contract(function, source)` is `None`. Input: a discard of a place that has no structural declaration. Callers in this range: 2518 (`discard_owned`), 2707, 3471, 3500; at 2518 no prior contract check on `source` exists on that path. Status: open.
- 4302-4308: `remaining_at_return` with no effects summary returns every live reference as remaining; the effect is to keep references, so the caller that compares against the declared result sources decides. Fail-closed if the caller rejects leftovers; caller not read. Open.
- 4469-4471: effects summary present but empty: `Ok(StoredReferences::default())`, ordinary call stores nothing. Traced: an empty list means the analysis found no stored effects.
- 4447-4466: when the callee has no summary the call is allowed only when no argument could have writable reference slots; `is_none_or` on both the path type and `writable_slots` turns "unknown" into an error. Fail-closed. Traced (read).
- 5080-5082: `projected_origin_family` with an empty suffix and `Projected` extent returns the origin unchanged. Intended. Traced.
- 5138-5142: `Invocation` origins overlap everything. Fail-closed. Traced.
- 5245-5250 `storage_cursor`: when the selected type differs from the expected one it only repairs a single case (reference whose referent is the expected type, on a parameter with only Field/Case/FixedIndex segments); otherwise `None`. Fail-closed.
- 6008-6019: "operation result remains pinned by live reference custody": uses `operation.psi_operation()`; if `None` (operations with no psi id) the check is skipped. Status: open, not traced.
- 6153-6171 and 6172-6195: observation checks use `shared_borrow_compatibility`, which lets a shared-borrow observation pass through a live reference whose carrier leaf is itself a shared-borrow descriptor (5473-5490). Intended per name. Traced (read).
- 6636-6669: overlap rejection is skipped when both arguments are `OrdinaryStorage` and both rows come from different argument positions with only ordinary storage (6644-6646: the pair is only rejected if at least one is non-ordinary). So two ordinary (non-reference-bearing) arguments to the same call are not checked against each other here. Presumably covered by plain borrow rules elsewhere. Status: open.

---

## Coverage

- Read in full: `current_ownership.rs` 1101-7100 = 6000 lines.
- Skimmed: none.
- Not read: none.
- Extra context read (not counted): 1060-1100 and 7101-7105.

## Five things a maintainer most needs to know about this range

1. All ownership checking in this range is by place identity plus structural path (and `(BlockId, node)` positions for loans); no source spelling is used. Refusals are almost all single error variants (`CurrentCleanupMismatch` at 2461, 2656, 2754, 3178, 3384, 3532, 3708; `CurrentReferenceCustodyViolation` via `invalid` 5164; `CurrentSharedJoinOriginDisturbed` via `pinned_root_disturbed` 2323), with static reason strings for the reference ones.
2. Two parallel region models exist: `borrowed_regions::Region/Projection` (descriptor loans, address-join pins, call write summaries) and `ReferenceOrigin/ReferenceOriginProjection` (reference custody). `check_operation`/`check_edge` (2145, 2183) use the first; `apply_operation` (5995) uses the second; each has its own overlap function (1120 `regions_overlap_at`, 5137 `origins_overlap`).
3. Two hand-written operation lists decide what each operation touches: `operation_observations` (1252-1466) and the `apply_operation` match (6020-6461, ending in `_ => {}`). They cover different operations (e.g. `ElementViewRead` only in the first).
4. A lot of cleanup/return validation is exact-shape matching on action lists: single `[RetireHome]`, `[DiscardRoot]`, whole-place `[InvokeNominal]`, or a partition plus `RetireHome` (3424-3436); nominal cleanup receivers must be records of Boolean or 8/16/32/64-bit integer fields (3662-3688) and there must be exactly one cleanup function per nominal type (3616-3622).
5. Call summaries are all-or-nothing: `CallAccesses::reconstruct` gives `None` for any function with ordinary `Call`, `AtomicEvent`, `PortWrite`, owned boundary arguments, `InvokeNominal`, or recursion (2968-3010, 2951-2966), and `ReferenceCallEffects::new` gives `None` if any stored origin is `Invocation`, `Provider`, `Empty`, `Literal`, `AccessFormation` or `IncomingContents` (4271-4274). Callers then fall back to the conservative path (3081, 4446-4466).

## What I am unsure about

- Whether `_ => {}` at 6461 is covered by checks outside this file; I did not read `unit_validation.rs` call sites.
- What `validate_owned_claim_frontier(..., &|_, _| true)` (3164-3170) accepts exactly; I did not read `terminal_semantics::owned_structure`.
- What `projected_root_is_fully_consumed`, `value_custody::uninstalled_home`, `storage_regions::*`, `residuals::selected_affine_residuals` do; only their call sites were read.
- Who consumes `AddressJoinPins::descriptor_requirements` (1889) and `ReferenceCallEffects::remaining_at_return` (4294), so the hand-offs flagged "open" were not traced to their end.
- Whether `psi/interpreter/terminal-interpreter/src/engine_structural.rs` and `pipeline/02_abstract-operations-to-target-operations/src/control_flow.rs` contain same-named copies of `window_hole`/`entry_references` (the combined grep matched them; I did not read them).
- The 4096 reference-leaf bound (3800) and the `live.len()` capacity used in most `leaf_paths` calls: I did not check that all callers' real data fit.
