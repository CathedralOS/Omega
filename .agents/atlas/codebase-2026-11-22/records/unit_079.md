# Record for unit_079 (area 14-interpreter)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths are relative to it. Paths under `source-rust/subsystems/interpreter/memory/` are shortened to `memory/`. "read" means I read the lines; "grepped" means a Grep count or content search; "inferred" means not read directly.

Unit files (10): `memory/bytes_and_views.rs`, `memory/custody.rs`, `memory/extents/src/{extent,lib,loans_and_claims,mapping}.rs`, `memory/materialization/src/{contract_tests,lib,materialization,symbolic_materialization}.rs`.

---

## 1. memory/bytes_and_views.rs (5952 lines, read in full)

**Purpose.** Runtime storage for two kinds of "view" values in the Terminal Psi interpreter: byte sequences (`ByteSequenceView`, `ByteSequenceBinding`) and typed element views (`ElementView`). It holds the operation handlers that build, read, write, narrow and compare them (`impl TerminalExecution`), and the accounting (storage lease) for byte allocations. The file header says it reserves "live byte capacity before allocation or copy-on-write detachment" (`bytes_and_views.rs:1-8`).

**Shape.** Two top-level inline modules.

`pub(crate) mod byte_sequences` (10-3230):
- `binding` (11-1214). `enum ByteSequenceBinding` (28-52) has five variants: `Immutable(ByteSequenceView)`, `Representation(Arc<ElementView>)`, `MutableArray{array, array_type, offset, length, referent, descriptor_home}`, `MutableField{field, capacity, parent_type, referent, descriptor_home}`, `MutableOwner{home: ByteOwnerHome, capacity, referent, descriptor_home}`. `enum ByteOwnerHome` (60-67) is `Frame{depth, machine, place}` or `Storage(StructuralRuntimePlace)`. Key methods: `validate_mutable_referent` (219-356, one branch per mutable variant), `reborrow_byte_sequence_binding` (1039-1101), `byte_descriptor_binding` (522-558), `byte_descriptor_source` (588-740), `byte_descriptor_length` (742-784), `prepare_owned_byte_descriptor_argument` (821-917), `prepare_byte_descriptor_argument` (919-1037), `byte_owner_bytes(_mut)` (1131-1212).
- `concatenation` (1215): `execute_byte_sequence_concatenation` (1223).
- `equality` (1259): `execute_byte_sequence_equal` (1266).
- `field_index_store` (1279): field length, byte store, byte read (1288, 1317, 1363).
- `field_store` (1406): `execute_structural_byte_sequence_field_store` (1441), `execute_structural_byte_view_field_store` (1536), `resolve_byte_field_carrier` (1646).
- `operands` (1750): `fixed_byte_array_observation` (1762), `byte_sequence_operand_view` (1791).
- `predicate` (1849), `read` (1868, `TerminalByteRead` with `Contiguous`/`Cells`/`Representation` storage), `storage` (2090, `ByteSequenceStorage` with `EvaluationStorageLease`), `store` (2327, `execute_byte_sequence_store` 2337), `subslice` (2399, `execute_byte_sequence_subslice` 2409, `narrow_byte_sequence_binding` 2514), `write` (2647, `mutable_byte_sequence_storage` 2673, `execute_byte_sequence_write` 2933).
- `struct ByteSequenceView` (3009-3014): `Arc<ByteSequenceBacking>`, `window: Range<usize>`, `descriptor_home`, `original_array`. Copy-on-write in `replace_byte` (3179).

`pub(crate) mod element_views` (3232-5952): `immutable_sequence::ImmutableStructuralSequence` (3238), `struct ElementView` (3351) with a private `enum ElementViewBacking` (3361: `Structural{collection}`, `ImmutableSequence`, `Typed`, `Representation`), `CollectionStorage` (3328), `ElementViewHome` (3323). Operation handlers: `execute_establish_element_view` (4079-4572), `execute_element_view_write/length/read/subslice` (4817, 4877, 4955, 4978). Sub-modules `representation` (5070; `RepresentationView`, byte-level reinterpretation) and `typed` (5588; `TypedElementView`, element-to-element reinterpretation).

**Inputs and outputs.** Reads the execution state `TerminalExecution` fields: `byte_sequence_values`, `element_view_values`, `structural_values`, `structural_byte_storage`, `structural_byte_sequence_fields`, `structural_element_view_fields`, `live_claims`, `live_affine_frontier`, `call_stack`, `values`, `account` (used throughout, for example 1241, 3993-4050). Reads type declarations through `self.structural_types()` and `terminal_semantics::*` geometry helpers. Writes: inserts into `byte_sequence_values`/`element_view_values`/`structural_values` (for example 1250-1254, 5063-5064), replaces field bytes (2386), mints identities through `local_structural_identities` (1245, 2494-2508), and calls `invalidate_stored_provenance` after field writes (1359, 1487, 2388, 2987).

**How it decides.** Almost every check keys on three things together: the value's `opaque_identity`, its `path` and its `structural_type`. See `matches_bound_descriptor_home` (3126-3137), `ElementView::same_descriptor_home` (3697-3704), and the `matches` closure in `byte_descriptor_source` (616-629). Rejections are `Err(malformed())` with no reason text in most places. Other keys:
- Access kind: `StructuralAccess` match arms (`SharedBorrow`, `MutableBorrow`, `WriteOnlyBorrow`, `Owned`), for example 593, 1045, 2424-2430, 3556.
- Declared type shape: `row.shape.is_bounded_owned_byte_sequence()`, `is_borrowed_byte_carrier()`, `is_shared_readable_byte_view()` (for example 518, 597-600, 797).
- Length operands: handlers recompute the real length and compare it with a length value supplied by the operation (1804, 1839-1841, 2381, 4851, 4970).
- Source spelling: none seen. Error detail strings are given to `in_argument_preparation`/`in_reference_formation` (for example 396, 975, 4126) but decisions do not branch on them.
- `byte_descriptor_source` (588-740) searches, in order: activation claim descriptors (630-637), a whole-place binding in the current frame (641-652), `structural_byte_descriptors` (653-666), `captured_byte_owner_binding` (667-681), a field-path source (682-708), stored bytes, then a scan of the current frame's `byte_sequence_values` and every `call_stack` frame (715-739). The scan takes the first binding whose home matches (`find_map`).
- `element_view_at_resolved_home` (4012-4050) also falls back to `returned_element_view`, then scans the current frame and all call-stack frames by home match.

**Cases by shape.**
- `ByteSequenceBinding::immutable`/`immutable_mut` accept only `Immutable` (100-107, 147-155).
- `narrow_byte_sequence_binding` (2514-2644): shared narrowing handles `Representation`, `Immutable`, `MutableArray`; `MutableOwner` is narrowed only when the window is the whole value (`window.start == 0 && end == length`, 2622-2629); `MutableField` and other mutable combinations are refused (2630-2635).
- `execute_byte_sequence_store` (2337-2396) accepts destination bindings `MutableField`, `MutableOwner`, `Immutable`; `Representation` and `MutableArray` are refused (2368-2371), and a resulting `MutableByteSequenceStorage::Array` is refused (2393).
- `execute_element_view_subslice` (4987-4991): `Unrestricted` becomes shared, `Affine` becomes mutable, `Linear` is refused.
- `scalar_width` (5412-5423): integer widths 8/16/32/64 that are not address-sized, plus binary32/binary64. Everything else is refused.
- `RepresentationView` byte storage (5097-5107) is one of three shapes: `Array`, `Immutable`, `Typed`. `write_representation_element` (5351-5389) writes only into `ByteStorage::Array` (5371).
- `TypedElementView::selected` (5616-5645) requires exactly one source/target leaf pair matching the path and scalar type (5641).
- `execute_establish_element_view` (4079-4572) handles: whole-representation (4210-4306), representation-bytes (4350-4418), typed (different source and target element, 4419-4453), and the default structural/byte path (4454-4571). It requires an element type that is an 8-bit unsigned integer for the byte-view case (4150-4155).
- `ImmutableStructuralSequence::from_values` accepts only elements with empty path and no qualifications (3269-3272).

**Same job elsewhere (in this unit).**
- Descriptor-home equality: `ByteSequenceView::matches_bound_descriptor_home` (3126), `ElementView::matches_bound_descriptor_home` (3706) and the closure at 616-629 implement the same test (identity + path + type).
- `relocated_descriptor`: `ByteSequenceBinding` (129-146) and `ElementView` (3592-3604) do the same relocation.
- Unsigned count extraction: `count` (3969-3988), `byte_sequence_unsigned_operand` (2992-3001) and inline copies (1296-1298, 1329-1332, 1373-1375, 1801-1803, 1837-1838, 4883-4884) all build `IntegerType::new(Unsigned, 64)` and check it.
- Scalar view conversion: `representation::decode_scalar`/`encode_scalar` (5431, 5453) and `typed::reinterpret` (5653) both go through `terminal_semantics::scalar_reference_view_value` (5436, 5462, 5661).
- Aggregate/element offset and stride validation appears in `materialization.rs` (see section 7) and not here.
- Same job suspected outside the unit: `read_primitive_home`, `store_primitive_home`, `primitive_view_access` (called at 3921, 5742, 5814) live in `memory/primitives.rs` (not read).

**Unused or one-caller code (grep).**
- `ByteSequenceView::retained_payload_bytes`, `shares_backing`, `owned_snapshot` and `ElementView::retained_bytes` (combined grep `\.retained_bytes\(\)|\.owned_snapshot\(\)|\.shares_backing\(|\.retained_payload_bytes\(` over `source-rust/`): 14 matches in 7 files, so these are in use (three of the matches are inside this file).
- I did not grep every `pub(crate)` function in this file. Not checked: `byte_field_snapshot`, `shared_byte_descriptor_binding`, `retained_byte_descriptor_binding`, `original_byte_allocation`, `storage_address_coordinate`, `captured_structural_index`, `lends_same_array_storage`.
- No comment in this file calls anything the single/shared/canonical way.

**Stated limits.** Comments in this file are few. Quoted:
- "A whole-place reborrow can present the caller's backing with the callee's formal type." (638-640, explains a lookup order.)
- "The resolved receiver still owns its physical field path." (1599.)
- "An unavailable descriptor is an error. A retained sibling whose codec is unsupported here is not the requested leaf." (5399-5400.)
- "The current layout producer excludes wider codecs." (test comment, 5563.)
- Runtime refusals saying a representation is unsupported: `UnsupportedSemanticVariant("bounded byte owner requires its byte-sequence storage representation")` (1146-1148) and the mutable version (1179).

**Fail-open risks.**
1. `prepare_byte_descriptor_argument` 927-946 returns `Ok(None)` (meaning "no byte descriptor to bind") when the parameter is borrowed-storage without a ceiling (927-935), when the access is `Owned` (936-938), when the place has no structural value (939-941), and when `resolve_live_projection` fails (`let Ok(value) = ... else { return Ok(None) }`, 942-946, which swallows the error). Which wrong program gets through depends on what the caller does with `None`; I did not read the caller. Classification: open.
2. `prepare_owned_byte_descriptor_argument` 831-847 returns `Ok(None)` for any parameter that is not a borrowed byte sequence carrier. Same note. Classification: open.
3. `execute_element_view_subslice` 5019-5023: the check that the result's projected qualifications are a subset of the source's is skipped when `view.len() == 0` (`view.len() != 0 && ...`). An empty view could be given projected qualifications that the source value does not carry. No comment gives a reason. Classification: open.
4. `representation_scalar_leaf` 5397-5408 skips (`continue`) a leaf whose offset or width cannot be computed. The comment at 5399-5400 says why. Classification: stated. The test at 5558-5584 pins that a missing descriptor still errors.
5. `byte_field_read` 432-437 returns `Immutable` and `Representation` bindings without calling `validate_presented_referent` (438). `Immutable` has no mutable referent to check; `Representation` is checked through `read_view` only. Classification: open (not traced).
6. `PartialEq for TerminalByteRead` (1926-1942) treats a read error as "not equal", so it errs toward refusal. Not a fail-open.
7. `impl PartialEq for ByteSequenceView` (3029-3033) compares backing pointer and window, not content. Anything that relies on it for content equality would be wrong; I did not trace uses. Classification: open.
8. `retire_unrestricted_views` (4053-4077) retires every view place with no affine discard and no claim, without asking why. No bad input found; it is a cleanup step.

Tests in the file: `byte_read_*` (2034, 2064), storage accounting tests (2228-2324), `closed_geometry_tests` (5474-5586), element-view home tests (5842-5950). Read in full.

---

## 2. memory/custody.rs (3028 lines, read in full)

**Purpose.** Ownership bookkeeping for structural values in the interpreter: which parts of an owned value are still live (the "affine frontier"), which linear claims are live (`LiveClaim`), how those move across calls (`transfer_claims`), and argument binding with alias checks. Header: "single-move affine custody, split and merge claim frontiers across projections, and verify loan provenance at call and block boundaries" (`custody.rs:1-7`).

**Shape.** Inline modules `complete_values` (9, `validate_complete_owned_value_claims` 16), `field_stores` (73, `restore_store_frontier` 85), `windows` (206: `loan_completions_for_operation` 220, `complete_loan_custody` 232, `owned_window_projection` 319, `stage_window_move_custody` 350, `stage_window_store_claims` 422). Free functions at file level:
- Binding: `bind_arguments` (815), `bind_boundary_arguments` (847), `bind_active_structural_arguments` (862), `bind_active_retained_arguments` (923), `bind_structural_arguments` (977), `bind_existing_structural_places` (1098), `retains_source_lender` (985), `bind_structural_primitive_values` (1191), `bind_affine_frontier(_types)` (1285, 1298), `bind_entry_claims` (2043).
- Frontier: `whole_value_frontier` (1345), `exclude_linear_regions` (1373), `consume_projected_ownership` (1431), `consume_affine_discard` (1510), `consume_affine_projection(_if_present)` (1547, 1713), `validate_complete_affine_frontier` (1575), `decompose_owned_subject` (1638), `split_affine_frontier_at_projection` (1790), `commit_cleanup_action` (690), `retire_cleanup_home` (786).
- Claims: `bind_result_claim` (516), `place_is_claimed` (534), `require_owned_value_claims` (538, also a method at 585), `transfer_claims` (2209), `rebind_structural_result_claims` (2370), `complete_claims` (2443), `validate_boundary_requirements` (2418).
- Type paths: `literal_fixed_array` (1918), `resolve_structural_path_type(_shape/_with_leaf)` (1947, 1964, 1988), `project_structural_value(_with_leaf)` (2126, 2160), `inline_structural_field` (2032).

**Inputs and outputs.** Takes `ProgramDeclarations<StructuralTypeDeclaration>`, `RuntimeRows<PlaceId, TerminalStructuralValue>`, `structural_cases` (a `BTreeMap<StructuralRuntimePlace, StructuralCaseContents>`), `BTreeSet<StructuralAffineDiscard>` for the frontier, `BTreeMap<ClaimId, LiveClaim>` for claims. Mutates the frontier and claim maps passed in. `commit_cleanup_action` (690) mutates seven runtime tables at once. Calls `terminal_semantics::owned_structure::*` for claim-frontier validation (27, 48, 571-580).

**How it decides.** Keys are places (`PlaceId`), structural paths (`Vec<StructuralPathSegment>`) with `starts_with` prefix tests, claim ids, and `StructuralMultiplicity` (Unrestricted/Affine/Linear). Examples: `place_is_claimed` is "any claim has this place" (534-536); `validate_complete_affine_frontier` compares the live frontier with a recomputed expected one using iterator equality (1624-1631); `bind_existing_structural_places` rejects aliasing when two arguments share `opaque_identity` and their paths may overlap and either is custody-bearing or exclusive, unless `retains_source_lender` says one lends to the other (1145-1183). Case-sensitive choices use the stored case at a projection (`stored_case_at_projection`, for example 61-66, 270-276, 1747-1754). No decisions on source spelling; the `reason: &'static str` fields in `OwnedValueCustodyMismatch` (552-558, 593-599, 639-646) are diagnostic only.

**Cases by shape.**
- `split_affine_frontier_at_projection` (1790-1916): handles `Record`, `FixedArray` with literal length, `Sum`/`Mixed`; everything else (`Opaque`, `TypeParameter`, `PrimitiveScalar`, `AtomicCell`, `Reference`, `ByteSequence`, `ElementView`, non-literal `FixedArray`) yields `None` and then `AffineProjectionNotRepresentable` (1892-1908).
- `decompose_owned_subject` (1638-1711): requires the declaration to be a `Record` (1682-1684, else `malformed`); an erased field gives `UnsupportedSemanticVariant("erased component decomposition needs its retained custody")` (1686-1691).
- `restore_store_frontier` (85-203): for a `FixedArray` parent only when `length <= frontier.len()` (175); other shapes `break` (188).
- `commit_cleanup_action` (720-723): `InvokeOwned` is refused with "owned cleanup requires its ordinary call transaction".
- `retains_source_lender` (985-1096): lists specific (lender access, lender multiplicity, source access) combinations at 1004-1067 and `continue`s on any other.
- `cleanup_field_type` (1765-1788): structural field, or an inline view carrier; a bounded owned byte sequence yields `None`.

**Same job elsewhere.**
- Frontier splitting appears three times in this file: `restore_store_frontier` merges children upward (144-201), `split_affine_frontier_at_projection` splits downward (1790), `complete_loan_custody` splits then retains (277-297). They share `split_affine_frontier_at_projection` only in the last two.
- Aggregate path typing: `resolve_structural_path_type` here versus `terminal_semantics::runtime_structural_path_tip` (called inside it, 2020), and `resolve_structural_path_shape` (1964) repeats the inline-carrier check from `resolve_structural_path_type_with_leaf` (1999-2019).
- `bind_arguments` (815) vs `bind_boundary_arguments` (847) both compare parameter scalar types to argument scalar types; only the first also range-checks integers (835-841).
- Claim matching by place+path prefix appears in `projected_ownership_claims` (1411), `complete_claims` (2443), `transfer_claims` (2352-2358), `stage_window_move_custody` (381-395). Not merged.

**Unused or one-caller code (grep over `source-rust/`, counts include the definition and tests).**
- `decompose_owned_subject(`: 7 matches (1 outside custody.rs, in `instructions/structural.rs`; 6 in custody.rs = definition + tests).
- `bind_boundary_arguments(`: 3 matches (`instructions/calls.rs`, `instructions/terminators.rs`, custody.rs definition).
- `bind_structural_primitive_values(`, `bind_affine_frontier(`, `retire_cleanup_home(`: 9 matches in 4 files.
- `project_shared_structural_observation(`, `direct_scalar_field_type(`, `fixed_index_step(`: 10 matches in 5 files.
- Definition-only grep for `byte_field_read`, `fixed_byte_array_observation`, `retire_unrestricted_views`, `commit_cleanup_action`, `whole_value_frontier`, `bind_entry_claims`, `complete_claims`, `rebind_structural_result_claims`, `validate_boundary_requirements`: each defined once (11 definitions in 2 files). I did not count their callers.
- No comment claims a single/canonical path.

**Stated limits.** Quoted:
- "owned cleanup requires its ordinary call transaction" (722).
- "erased component decomposition needs its retained custody" (1689).
- "Independent claims must remain settled by an explicit correspondence." (test comment, 2621.)
- "Linear claims own their regions separately; component entries do not restore an ended wrapper's availability." (doc comment, 1572-1574.)
- "Decomposition changes custody, never the allocation or its payload bytes." (1634.)

**Fail-open risks.**
1. `require_owned_value_claims` method (585-666) returns `Ok(())` early when `argument.access != Owned` (589-591), and when the parameter is not owned or the projection leaves inline storage (605-614). A borrowed argument therefore never reaches claim validation here. Classification: open (not traced to its callers).
2. `bind_entry_claims` (2071-2080): for each `ContentEntryClaim` it inserts a `LiveClaim` with `place: None` or the parameter's multiplicity if no parameter matches (`parameter.map(...)`), using `entry().or_insert`. A content claim naming a non-parameter root is accepted as a claim with no place. Classification: open.
3. `transfer_claims` (2360-2366) adds `LiveClaim { place: None, multiplicity: None }` for every callee content entry claim with `or_insert`, with no check against the caller. Classification: open.
4. `consume_projected_affine_complement` (1491-1507): when the argument is neither contained in nor containing a frontier entry, it returns `Ok(false)` rather than an error. The caller `consume_projected_ownership` then fails only if there are no selected claims either (1456-1458). A projection with matching claims but no frontier entry passes. Classification: open.
5. `consume_affine_projection_if_present` (1713-1763) returns `Ok(false)` when no frontier entry contains the path. The strict wrapper `consume_affine_projection` (1547) turns `false` into an error. Classification: traced inside the file (the two wrappers differ on purpose).
6. `restore_store_frontier` (85-203) returns `Ok(())` without changing the frontier when the path contains a `Referent` segment (93-95), when the destination is not an owned affine place (119-121), and when `owned_window_projection` gives `None` (123-125). A store through a reference or into a borrowed place leaves the frontier untouched. Classification: open.
7. `owned_window_projection` (319-348) returns `Ok(None)` for paths containing `Referent`, element-view roots and mutable-borrow parameters. Classification: open.
8. `stage_window_move_custody` (396-404): when the result is `Unrestricted`, no claims are selected and no frontier entry overlaps, it returns the current claims unchanged. Plausibly intended; no comment. Classification: open.
9. `has_live_loan` (680-688) matches references by identity plus `structural_paths_may_overlap`; it returns `false` when there are none. Fail closed in its use in `decompose_owned_subject` (1656).
10. `complete_claims` has an unused parameter `_boundary_parameters` (2447). No comment.

Tests (2492-3027): decomposition, claim transfer by path, aliasing, retained mutable lender. Read in full.

---

## 3. memory/extents/src/extent.rs (450 lines, read in full)

**Purpose.** The linear `Extent` carrier (a range of address space with rights) and its authority-conserving operations. Each failed operation hands the unconsumed `Extent` back in the error (header 1-6).

**Shape.** `struct Extent` (60-70): `base`, `length`, `address_space`, `rights`, `provenance`, `era`, `origin: ExtentRootOrigin`, `lineage: Lineage {root, path: Vec<SplitBranch>}`, `sharing: ExtentSharingMode`. Methods: `split_at` (121), `partition_owned` (175), `attenuate` (227), `merge` (237), `loan` (269), `loan_mut` (273). `OwnedExtentPartition` (288) with `rejoin` (311). `validate_merge` (327-385). Error types `SplitError`, `OwnedPartitionError`, `AttenuationError`, `MergeError` (387-450). Module `diagnostic` (8-30) has `validate_range` and `nonzero_identity`.

**Inputs and outputs.** Inputs are consumed `Extent`s and numbers. Outputs are new `Extent`s whose `lineage.path` records the split. No other module is read except `loans::ExtentLoan` and `roots::root_origins`.

**How it decides.** `merge` accepts only when both extents agree on `address_space`, `rights`, `provenance`, `era`, `sharing` (328-337), `lineage.root` (338), `origin` (343), have parent paths equal with different last branch (348-362), and are adjacent with the lower being `Lower` and the upper `Upper` (364-379). Keyed on identities and lineage path, never on addresses alone: the test `adjacency_never_merges_independent_grants` pins that (`lib.rs:896-903`). `attenuate` accepts only when `self.rights.contains(&rights)` (228). `loan_mut` refuses `PeerShared` (278).

**Cases by shape.** `partition_owned` has four cases on `(offset, end == length)` (204-219).

**Same job elsewhere.** The split/merge-with-lineage-path pattern is repeated for activation claims in `loans_and_claims.rs` (`split_claim` 397, `merge_claims` 453) using the same `Lineage` and `SplitBranch` types (imported at `loans_and_claims.rs:221-222`).

**Unused or one-caller code.** `Extent::end` (81) does `base + length` without a checked add; `validate_range` (9-17) is what prevents overflow at mint time. Not grepped for callers. The grep for `ActivationClaimLedger|begin_external_loan|admit_role_coverage|admit_acquisition_after_completion|map_owned(|map_borrowed(|OrderingEvent::` shows non-test users outside the extents crate only in `memory/primitives.rs:31-42` (`ActivationClaimLedger`).

**Stated limits.** None seen.

**Fail-open risks.** `rejoin` (311-324) uses `.expect("private owned partition retains exact ... sibling")`: it panics rather than accepts if the invariant breaks. `Extent::end` (81) can overflow only if the extent was built without `validate_range`; mint paths validate (`lib.rs:348`, 370), and `merge` and `split` preserve the sum. Classification: traced (fields are `pub(crate)`, built only in this crate: `lib.rs:414`, `mapping.rs:747-752` where mapped extent copies the destination's geometry).

---

## 4. memory/extents/src/lib.rs (1202 lines, read in full)

**Purpose.** Module wiring plus the inline modules `loans`, `roots` (`root_grants`, `root_origins`), and a re-export list (739-775). Also an in-file test module (777-1202).

**Shape.**
- `ExtentLoan<'a>` (57-61): a borrowed view of an `Extent` range, shared or exclusive (`LoanBacking` 52). `validate_subrange` (167-185).
- `ExtentRootGrant` (202-210) with `from_admitted_provider` (295), `from_established_program_local` (320, `#[doc(hidden)]`), `admitting_peer_shared_backing` (314), `mint` (347), `mint_provider_existing_content` (360), `mint_validated` (404). `ProviderExistingContentGrant` (213), `ValidatedExtentGeometry` (274).
- `ExtentProviderIssuance` (474-484) with 13 identities, `from_normalized_identities([u64; 13])` (534). `ExtentProviderInvocation` (487). `ExtentProgramLocalOrigin` (634) with 8 identities and a nonzero check (645-681). `enum ExtentRootOrigin { ProgramLocal, ProviderIssued }` (717).

**Inputs and outputs.** Reads `extent_contracts` ids; produces `Extent` only through `mint*`. Header says fresh roots enter "solely through admitted provider issuances or program-local origins" (`lib.rs:1-9`).

**How it decides.** `mint_provider_existing_content` refuses a program-local origin (379-387). `validate_subrange` refuses overflow or `end > extent.length` (172-180). Identities must be nonzero (`nonzero_identity` in `extent.rs:20-27`, used at 669, and by `from_normalized_identity` in `extent_contracts`, not read).

**Cases by shape.** Fixed-width identity arrays: 13 for providers (535), 8 for local origins (647). Two origins only.

**Same job elsewhere.** `ExtentLoan` accessors (90-153) duplicate the `Extent` accessors in `extent.rs:73-119` through a `match` on `LoanBacking`; `MappingSource` in `mapping.rs:106-171` does the same again for `Extent`/`ExtentLoan`.

**Unused or one-caller code.** `from_established_program_local` is `#[doc(hidden)]` and `pub`. Grep `from_established_program_local` over `source-rust/`: not run. Re-exports at 739-775 expose `begin_external_loan`, `map_owned`, `map_borrowed`, `OrderingEvent`, `admit_role_coverage`, `admit_acquisition_after_completion`. The combined grep (`ActivationClaimLedger|begin_external_loan|map_owned(|map_borrowed(`) over `source-rust/subsystems/` lists only four files: `memory/primitives.rs` and three files inside the extents crate. So external loans, mappings and ordering events have no caller outside this crate in the root workspace source (grepped; test modules inside the crate included).

**Stated limits.** `#![allow(clippy::result_large_err, reason = "extent failures deliberately return the conserved authority needed for retry")]` (31-34). Header says the crate is a "single conservation calculus" for "DMA buffers, MMIO ranges, page mappings" (11-20).

**Fail-open risks.** none seen. Every mint path validates geometry first (348-358, 370-377).

Tests (777-1202): conservation, existing-content, loans modules; helper constructors at 1115-1201. Read in full.

---

## 5. memory/extents/src/loans_and_claims.rs (1550 lines, read in full)

**Purpose.** Three bookkeeping models for hardware-facing authority: runtime-sized activation claims, loans to external borrowers (DMA) with completion receipts, and device ordering events with role coverage (header 1-6).

**Shape.**
- `activation_claims` (8-531): `ActivationClaimLedger` (316) with `establish` (344), `release` (378, LIFO only), `split_claim` (397), `merge_claims` (453), `drain` (505), `composed_demand` (519). `ActivationClaim` (254), `ActivationClaimRequest` (245). Bound math lives in `resource_bounds` (`aligned_claim_bound`, `compose_claim_bounds`, `validate_committed_within_bound`, imported 224-227).
- `external_loans` (533-1105): `begin_external_loan` (724), `ExternalLoan::complete` (674), `ExternalLoanGrant` (605), `ExternalReachReceipt` (559), `ExternalCompletionReceipt` (813).
- `ordering_events` (1107-1550): `OrderingEvent` (1166), `admit_role_coverage` (1286), `admit_acquisition_after_completion` (1344), `DeviceOrderingRole` (1117: Publication, CacheMaintenance, Notification, Completion, Acquisition).

**Inputs and outputs.** The first uses only plain numbers and `ActivationClaimBranch`. External loans consume `ExtentLoan`, grants and receipts and return `ExternalLoanCompletion`. Ordering admission consumes `Vec<OrderingEvent>` and returns the same events on rejection (1271-1284).

**How it decides.** Receipt matching is a chain of field equality tests, one message per mismatch: `complete` compares 12 fields of the receipt to the loan (678-704); `begin_external_loan` compares grant to loan (730-746) then receipt to loan (756-791). All keyed on identities (loan id, borrower, address space, provenance, era, lineage, rights, base, length). Claim merging requires the two newest live claims (460), same lineage root, siblings, same site and branch (467-486). `admit_role_coverage` keys on device, scope, request, authority, then role set equality (1299-1340).

**Cases by shape.**
- `ExternalReachMechanism` has two values: `AdmittedBorrowerContract`, `HardwareIsolation` (553-556). Neither changes admission; `begin_external_loan` only copies it (807).
- Direction rules: `DeviceReads` needs shared loan, `DeviceWrites` needs exclusive (736-743).
- Claims release strictly in reverse establishment order (382-394).

**Same job elsewhere.** Lineage split/merge at 397-503 repeats `extent.rs:134-173` and `237-267`. Receipt mismatch chains repeat in `mapping.rs:242-267` (`verify`) in a table form.

**Unused or one-caller code.** Grepped: `begin_external_loan`, `admit_role_coverage`, `admit_acquisition_after_completion`, `OrderingEvent::`, `ActivationClaimLedger` in `source-rust/`: matches only in extents files and `memory/primitives.rs`. In `primitives.rs` only `ActivationClaimLedger` is used (lines 31, 34, 42). So everything in `external_loans` and `ordering_events` has no non-test caller in the root workspace (grepped). `ExternalReachReceipt::from_admitted_provider` and the other `from_admitted_provider` constructors are named as called by admitted providers; I found no provider caller.

**Stated limits.** None in comments. The "authority" nature is in doc names (`from_admitted_provider`).

**Fail-open risks.**
1. `admit_role_coverage` records only the last failure message (loop sets `failure` then breaks, 1299-1323); fine, it still rejects.
2. `ordering_events::OrderingEvent::from_admitted_provider` validates ranges (1188) but not that the mapping in `OrderingRange.mapping` exists. No registry is consulted. Classification: open (unused in the workspace).
3. `ExternalLoan::complete` accepts completion with `established_facts.contains(&self.completion)`; an empty `completion` set accepts any facts (700). That is by design for grants without obligations (test at 996-1023). Classification: stated by test `external_write_loan_derives_exclusive_cpu_exclusion`.
4. `ActivationClaimLedger::release` with an empty ledger errors (390-393); fine.

Tests: 11 in activation claims (9-219), 4 in external loans (902-1104), 10 in ordering events (1366-1549). Read in full.

---

## 6. memory/extents/src/mapping.rs (1488 lines; lines 1-1000 read, 1001-1488 not read)

**Purpose.** Address-space translation: an `Extent` or loan is mapped into a destination extent under a `MappingGrant`; installation, peer-write revocation and release each need a receipt that binds the exact mapping (header 1-6).

**Shape.** `MappingGrant` (36), `MappingSourceMode` (22: Owned, BorrowedShared, BorrowedExclusive), `MappingEvidence` (182), `MappingTransitionReceipt<FactId>` (223) with `verify` (242-267), `PendingMap` (327), `MappedExtent` (311), `PendingPeerWriteRevocation` (566), `PendingUnmap` (827), `UnmappedExtents` (901). Entry functions `map_owned` (646), `map_borrowed` (682), `map_with_source` (709).

**Inputs and outputs.** Takes a source (`Extent` or `ExtentLoan`), a destination `Extent`, a `MappingId` and `&MappingGrant`. Produces `PendingMap`, then `MappedExtent` on a verified `TranslationActivationReceipt`, then `PendingUnmap` and `UnmappedExtents` returning destination and (if owned) source.

**How it decides.** `map_with_source` (709-783) refuses on: source custody mode versus grant (715), address spaces (717-720), required source and destination rights (721-727), equal lengths (728). Receipts are compared by whole-struct equality `&self.mapping != mapping` (248) then `established`, then obligations subset (253-262). The only string-keyed decision is none; the `TransitionWording` constants (270-296) are message text only. `loan_mut` refuses `BorrowedShared` (518); `stable_loan` refuses shared custody until peer-write revocation completes (531-537); `begin_peer_write_revocation` only for `BorrowedShared` (549).

**Cases by shape.** Three source modes. `MappingSource::Borrowed` with exclusive polarity gets no special handling after mapping beyond `mode()` (166-169).

**Same job elsewhere.** Receipt verification repeats the `complete` checks in `loans_and_claims.rs:674-704`. `MappingSource` accessor `match`es repeat `ExtentLoan` accessors (`lib.rs:90-153`).

**Unused or one-caller code.** `map_owned`, `map_borrowed`, `range_receipt_context`, `stable_loan`: grep in `source-rust/subsystems/` shows no callers outside the extents crate (see section 5).

**Stated limits.** none seen in comments.

**Fail-open risks.**
1. `UnmappedExtents` returns only the destination when the source was borrowed (`MappingSource::Borrowed(_loan) => None`, 872-875): the loan is dropped. By design.
2. `map_owned`/`map_borrowed` use `unreachable!` on source-mode round trip (671, 698). Panic, not accept.
3. `map_with_source` does not compare `source.length()` to anything beyond the destination length and does not check destination rights equal mapped rights; it replaces rights with `grant.mapped_rights` (748). A grant can therefore give rights the destination did not have. Classification: open (provider supplies the grant, "from_admitted_provider").

Not read: lines 1001-1488 (tests; names from grep: `owned_mapping_round_trips_...` 1034, `borrowed_mapping_retains_source_loan_and_shared_polarity` 1203, `mapped_range_receipt_context_*` 1228, 1261, `failed_mapping_returns_source_and_destination_authority` 1278, `peer_write_revocation_*` 1330, 1434, 1470, `shared_mapping_stable_loan_requires_completed_peer_write_revocation` 1374).

---

## 7. memory/materialization/src/lib.rs (24 lines, read in full)

Crate root. Declares `forbid(unsafe_code)` (4), re-exports (8-18) and gates `contract_tests` under `cfg(test)` (23-24). Doc says the service "never chooses a layout, activates source or generates executable code" (1-3).

---

## 8. memory/materialization/src/materialization.rs (1249 lines, read in full)

**Purpose.** Writes and reads field values into byte buffers according to an admitted layout plan (`LayoutPlanReport`): scalar fields, aggregate (whole-byte) fields, stored-integer and bit-fragment placements, and resolved symbolic writes.

**Shape.** `field_codec` (1-570): `update_scalar_field` (9), `stage_scalar_field` (25), `decode_aggregate_field` (52), `decode_aggregate_field_into` (69), private `prepare_aggregate_field` (135-282) returning `DecodedAggregateField::{Scalar, Chunks}` (79-89). `stored_integer_writes` (572-653): `apply_write` (575), `apply_fragment` (595), `read_container` (630), `write_container` (643); re-exports `layout_plans::stored_integer_checks::*` (573). Top level: `materialize_resolved_into` (673), `fixed_layout_byte_len` (721), `materialize_scalar_layout_into` (741), `materialize_aggregate_layout_into` (778), `stage_aggregate_layout` (798-1109), `decode_scalar_layout` (1111-1249).

**Inputs and outputs.** Input: `LayoutPlanReport` (entries with `field`, `member_identity`, `placement`), field schemas/values from `layout_plans::field_values`, a `ByteOrder`, and a byte slice. Output: bytes written (staged in a private buffer then copied, so failures leave the destination unchanged: 706-717, 766-775, 792-795) or decoded values. `stage_aggregate_layout` writes directly into `destination[..byte_len]` after `fill(0)` (850-851) and is documented in tests as "staged" scratch, not atomic (`field_codec` test 366-369).

**How it decides.** Field matching is by `materialization_field_key(&field, member_identity)` (for example 36-40, 151-156, 757-758): numbered fields join by stable member identity, unnumbered by name. Tests pin rename-by-identity (`lib` tests: `contract_tests.rs:182`, 435, 978). Placement kind picks the path: `IntegerAt`/`Bits` are scalar placements, `At` is whole-byte (859-873). Overlap is tracked in `occupied: Vec<bool>` (852, 931, 1051) and per-bit fragment masks in a `BTreeMap<usize, u8>` (1060-1105).

**Cases by shape.** Quoted and cited:
- Scalar placements require a `Whole` field shape of at most 8 bytes ("scalar placements require one field of at most eight bytes", 166-170; same bound at 880-885).
- Whole-byte fields: one `At` entry, or several for a `Repeated` shape with constant aligned stride (233-265, 966-1018).
- `At` mixed with scalar placements is refused (171-176, 865-873).
- `decode_scalar_layout` allows exactly one stored-integer entry per field and requires stored width byte-sized and no wider than the carrier (1173-1190).
- Missing `size` on a layout refuses ("requires a fixed-size layout plan", 724-731).

**Same job elsewhere.**
- Repeated-array offset/stride validation is written twice: decode at 225-267 and stage at 966-1018 (the decode adds an alignment check at 254-258; the stage does the alignment check later at 1030-1035).
- `decode_scalar_layout` re-implements fragment read via `read_container` while `apply_fragment` does the write; `symbolic_materialization.rs:914-942` `apply_scalar_entry` reuses `apply_fragment`.
- `materialize_resolved_into` (673-719) and the resolved-write validation in `symbolic_materialization.rs:466-475` both call `validate_write_source_value`.

**Unused or one-caller code.** Grep (over the primary checkout `C:/Development/Omega`, not the worktree, pattern `storage_materialization::|apply_write\(|stage_aggregate_layout|materialize_aggregate_layout_into`, count mode): non-test outside the crate only `source-rust/subsystems/interpreter/memory/providers.rs` (lines 1594, 1612, 3378, 3531: `stage_scalar_field` and `decode_aggregate_field_into`), plus one hit in `_recycling/.../post-handoff-writer/src/tests.rs`. A second grep over the worktree `source-rust/` for `derive_symbolic_materialization|materialize_resolved_into|materialize_scalar_layout_into|decode_scalar_layout|decode_aggregate_field|stage_scalar_field|update_scalar_field`: 96 matches in 5 files, and the only non-materialization-crate file is `providers.rs` (4 matches). So `derive_symbolic_materialization(_with_inner_layouts)`, `materialize_resolved_into`, `materialize_scalar_layout_into`, `materialize_aggregate_layout_into`, `stage_aggregate_layout`, `decode_scalar_layout`, `update_scalar_field` and `apply_write` have no caller in the root workspace outside this crate's own tests. The root README for this crate calls `apply_write` etc. the way materialization applies writes; the other user was in `_recycling/` (archived).

**Stated limits.** None in comments. Header refuses to choose layouts (lib.rs 1-3).

**Fail-open risks.**
1. `stage_scalar_field` (25-50) and `stage_aggregate_layout` (798) write into `destination` as they go; on error the destination may be partly written. The test at `materialization.rs:358-369` states this for staging: "assert_ne!(private[..8], atomic[..8])". Classification: stated (by test; `update_scalar_field` and `materialize_*_into` are atomic through a staging copy, 19-22, 792-795).
2. `stage_scalar_field` applies every entry that matches the key (36-48) and fails only when none match (41-45). A field with several entries gets all written. By design.
3. `decode_aggregate_field` ignores bits of `source` beyond what the entries cover. Not accept/reject relevant.

Tests inside the file: 5 (284-569). Read in full.

---

## 9. memory/materialization/src/symbolic_materialization.rs (942 lines, read in full)

**Purpose.** Joins a layout (and optional nested record/sum interior layouts) with symbolic field values and a resolver to produce a `SymbolicMaterializationPlan`: a list of actions that are either resolved writes, post-handoff runtime writers, or native pointer relocations (header 1-5).

**Shape.** `derive_symbolic_materialization` (31) forwards with no inner layouts to `derive_symbolic_materialization_with_inner_layouts` (46-529). Private: `PreparedInterior` (531: `Record`, `Sum`), `PreparedInnerLayout` (546), `prepare_inner_layouts` (566), `prepare_record_interior` (704), `prepare_sum_interior` (761), `first_untraversed_inner_layout` (819), `select_materialization_entries` (840), `write_from_entry` (875). `pub(crate) apply_scalar_entry` (914).

**Inputs and outputs.** In: `LayoutPlanReport`, `[SymbolicFieldInnerLayout]`, `[SymbolicFieldValue]`, `MaterializationContext` (placement, consumption instant, byte order, native relocation width), and a resolver closure `FnMut(RelocationTarget) -> Option<u64>` (51). Out: `SymbolicMaterializationPlan { byte_len, byte_order, placement, actions }` (523-528). The resolver is called once per distinct target and memoised (457-478).

**How it decides.**
- Field joins by `materialization_field_key` (99, 153, 346-349, 373-377).
- Resolved or not: `resolve(symbolic.target)` returns `Some` (resolved write) or `None` (461-478). For `None`: `ConsumptionInstant::AfterOmegaHandoff` gives `RuntimeWriter` (485-487); otherwise an `At` placement whose width equals `context.native_pointer_relocation_bits` gives `NativePointerRelocation` (488-498); an `At` with another width, an `IntegerAt`, or `Bits` is an error (499-516). So the decision is keyed on placement kind and a bit width.
- Path depth bound `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` (65, 734).
- Interior placement: repeated interiors need an element index within `element_count`, and the base is the minimum `At` offset (237-267); non-repeated interiors need exactly one whole `At` entry (268-298).

**Cases by shape.** Quoted refusals show the fixed set: sum paths must "spell a selected case and payload field or a common field" (337-340); "a case payload is the leaf of a sum path" (360); "a common field is the leaf of a sum path" (406); sum interiors carry "no nested record carriers" (614-617); element index not allowed on a case or payload (341-345, 363-367); repeated sum stride must be at least the sum size (619-626). Nesting depth limited by the record path bound (734-738).

**Same job elsewhere.** Source-offset composition and bounds validation (`validate_write(current_byte_len, &write)` then `validate_write(byte_len, &write)` at 210-218, 430-438) repeat at both leaf kinds. Per-hop key lookup and drift check appear at the top level (98-120) and per-hop (153-185). Aggregate placement logic is in `materialization.rs` (section 8).

**Unused or one-caller code.** See section 8 grep: no caller outside this crate's tests in the root workspace (grepped).

**Stated limits.** Error strings, for example "loader consumes symbolic field `{}` before Omega entry, but the target has no native {}-bit pointer relocation" (500-503) and "symbolic materialization has no integer fit proof" (507-508), "unresolved fragments require a fixed address or a post-handoff writer" (513). Quote under 20 words each.

**Fail-open risks.** none seen. Unresolved actions never become silent: they become `RuntimeWriter` only after handoff (485-487), and `materialize_resolved_into` rejects any non-resolved action (`materialization.rs:684-692`). Classification: traced inside the unit.

---

## 10. memory/materialization/src/contract_tests.rs (3090 lines, NOT read)

Only the function and module names were obtained by Grep (`^\s*(fn |mod |#\[test\])`). Modules: `conventional_materializations` (14), `symbolic_materializations` (1308), `symbolic_sum_materializations` (1853), `writer_fragments` (2708), plus helpers `nested_layout` (2961), `record_interior` (2989), `sum_field_layout` (2996), `recursive_sum_report` (3013), `sum_array_layout` (3059). Test names indicate what is pinned (for example `materializers_reject_layout_identity_aliases_before_observable_work` 245, `symbolic_inner_materialization_requires_a_bound_traversed_carrier` 1417, `unresolved_action_cannot_partially_materialize` 2863). These are inferred from names only. Grep count of `#[test]` names listed: 40 in the first 120 matches list; the crate map says 46 in the crate (not recounted).

---

## Coverage

- Read in full: `memory/bytes_and_views.rs` 1-5952 (5952); `memory/custody.rs` 1-3028 (3028); `extents/src/extent.rs` 1-450 (450); `extents/src/lib.rs` 1-1202 (1202); `extents/src/loans_and_claims.rs` 1-1550 (1550); `extents/src/mapping.rs` 1-1000 (1000); `materialization/src/lib.rs` 1-24 (24); `materialization/src/materialization.rs` 1-1249 (1249); `materialization/src/symbolic_materialization.rs` 1-942 (942). Total read in full: 15,397 lines.
- Skimmed: none.
- Not read: `extents/src/mapping.rs` 1001-1488 (488 lines, tests; names grepped only); `materialization/src/contract_tests.rs` 1-3090 (3090 lines; names grepped only). Total not read: 3,578 lines.
- The unit totals 18,975 lines (from the unit file).

## Five things a maintainer most needs to know about this unit

1. Nearly every runtime check on byte and element views is "does this value have the same `opaque_identity`, `path` and `structural_type` as the binding's descriptor home". The same test is written separately for byte views (`bytes_and_views.rs:3126`), element views (3706) and inline in `byte_descriptor_source` (616-629), and lookups fall back to scanning the current frame plus every call-stack frame and taking the first match (715-739, 4031-4049).
2. `bytes_and_views.rs` and `custody.rs` return the unit-less `malformed()` for most refusals. The reason is usually only visible by reading the failing branch; a few branches attach text (`in_argument_preparation`, `OwnedValueCustodyMismatch.reason`).
3. Several functions return `Ok(None)` or `Ok(())` for shapes they do not handle (`prepare_byte_descriptor_argument` 927-946, `restore_store_frontier` 93-125, `owned_window_projection` 327-334, `require_owned_value_claims` 589-614). Whether that is safe depends on callers I did not read. The empty-view skip at `bytes_and_views.rs:5019-5023` is the one place where a subset check is switched off by a length.
4. The `extents` crate is large, mostly unused by this workspace. Only `ActivationClaimLedger` is used outside the crate (`memory/primitives.rs:31-42`). External loans, mappings, ordering events, `ExtentRootGrant` mint paths and all `from_admitted_provider` constructors have no non-test caller in `source-rust/subsystems/` by grep (they carry the `ADDRESS-EXPOSURE` work noted in the crate map).
5. Of the `storage-materialization` API, only `stage_scalar_field` and `decode_aggregate_field_into` are called from outside the crate (`memory/providers.rs:1594, 1612, 3378, 3531`). `derive_symbolic_materialization*` and the whole-layout `materialize_*` functions are exercised only by the crate's own tests (grepped, 96 matches in 5 files).

## What you are unsure about

- Whether callers treat `Ok(None)` from `prepare_byte_descriptor_argument` / `prepare_owned_byte_descriptor_argument` as "no descriptor needed" safely; I did not read `instructions/calls.rs`.
- Whether the empty-view skip at `bytes_and_views.rs:5019-5023` is intentional (an empty view has no element to carry a membership) or a gap.
- Whether the `ADDRESS-EXPOSURE` TASKS item is the reason the extents crate has no caller; I did not read `TASKS.md` in this pass.
- One grep (`storage_materialization::|apply_write\(|...`) ran against the primary checkout `C:/Development/Omega`, not the worktree, because I gave the path as `C:/Development/Omega`; it includes `_recycling/` and the main working tree. Counts for that pattern may differ from the worktree by whatever differs between the two checkouts. The other greps used the worktree path.
- The counts I gave as "definition + tests" in custody.rs (6 in custody.rs for `decompose_owned_subject(`) were not split into definition and test sites.
- `contract_tests.rs` and the tail of `mapping.rs` were not read, so I cannot say what they pin beyond test names.
