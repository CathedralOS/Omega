# Unit 83 notes

Files covered (checkout `<checkout>/`, all paths relative to `omega-rust/psi/compilation/representations/terminal-psi/`):
1. `semantics/src/references.rs` (1-4508)
2. `semantics/src/scalars.rs` (1-5021)
3. `semantics/src/storage.rs` (1-3683)
4. `semantics/src/structural.rs` (1-5804)
5. `semantics/src/substitution.rs` (1-929)
6. `src/artifacts.rs` (1-566)

Grep counts below are "word-match occurrences across `omega-rust/**/*.rs`" (command: `grep -rw --include=*.rs -c NAME .` summed), which include the definition and re-export lines in `semantics/src/lib.rs`; where it matters I list the actual sites.

---

## 1. `semantics/src/references.rs` (4508 lines)

### Purpose
Pure judgments (no I/O, no mutation of modules) about references and borrowed views in Terminal Psi: where a returned reference may come from (origins), whether a callee's declared reference regions cover what its body returns, how reference call results bind to actual arguments, whether a reference/descriptor can be reborrowed or copied, the byte geometry of stored types (offsets/sizes/alignments of a path inside a structural type), and a resident-reference placement check. It is a bag of `pub(crate) mod` blocks re-exported from `lib.rs`.

### Shape (module by module)
- `reference_completion` (references.rs:6-154): `ReferenceCompletionOccurrence` (:14, identity, parents, returned, in_caller_storage, disposed); `borrowed_entry_reference_referent` (:22); `ReferenceCompletionError` (:45, 5 variants); `validate_reference_result_completion` (:71) walks parent chains with a 0/1/2 visited colouring (:82-118) then checks no live child survives a closing parent (:119-131); `cleanup_disposes_reference` (:135).
- `reference_origins` (:155-2096), submodules:
  - `borrowed_leaves` (:163-349): `BorrowedResultLeaf{path,access,kind}` (:177), `BorrowedResultLeafKind::{Reference(type),View}` (:172), `borrowed_result_leaves` (:183) via private `BorrowedResultRoster::visit` (:234) / `fields` (:324). Enumerates every reference/view leaf in a result type, capped by `maximum_leaves`.
  - `call_results` (:350-582): `ReferenceCallContract::{Machine{parameters,sources},Requirement(slot)}` (:361), `ReferenceCallOrigin::{Parameter{source,transfers_existing},Invocation{..}}` (:369), `ReferenceCallSource` (:381), `ReferenceCallResult{path,sources}` (:388), `bind_reference_call_results` (:393), `bind_public_reference_call_results` (:479).
  - `memberships` (:583-1078): `ReferenceResultMemberships` (:596; holds `&TerminalModule` and a `OnceCell<Vec<DescriptorMachine>>` storage graph built lazily), `for_operation` (:609), `reference_call_preserved_memberships` (:691), private `preserve_call_memberships` (:716), `reference_result_memberships_weaken` (:859), plus 2 unit tests (:1012, :1043).
  - `descriptor_regions` (:1084-1363): `validate_descriptor_result_regions` (:1101), helpers `input_origin_path` (:1218), `append_projection` (:1264), `contents_path` (:1289), `inline_view_is_covered` (:1330).
  - `result_regions` (:1364-1685): `validate_dynamic_reference_regions` (:1374), `validate_boundary_reference_regions` (:1391), private `validate_reference_regions` (:1409), `validate_input_region` (:1492), `reference_leaves` (:1584), `ReferenceLeaf` (:1681).
  - Parent-level items in `reference_origins`: `ReferenceOriginBinding` (:1697, `transfers_existing` :1705), `bind_reference_origin` (:1727), `reference_origin_refines` (:1767), `reference_origin_is_covered` (:1849), `descriptor_region` (:1906), `reference_result_refines_regions` (:1917), `reference_origin_paths_may_overlap` (:1975), `captured_reference_origin_type` (:2000), `reference_origin_type` (:2017), `resolve_reference_origin_type` (:2036), `exact_origin_type` (:2067), `collection_element_type` (:2078).
- `reference_storage_geometry` (:2097-3659): `ReferenceStorageProjection{byte_offset,logical_scalar,stored_scalar}` (:2110), `ReferenceScalarStorage::{Contiguous,Fragmented}` (:2116), `ReferenceStorageRegion` (:2126), `ReferenceArrayWindow` (:2134), `StorageGeometryError` (:2180, 15 variants), `RecordStorageResolver` trait (:2205) with `UnselectedRecordStorage` (:2213, always `Err(UnresolvedLayout)`), `ReferenceRecordField` (:2225), `ReferenceStorageGeometry<Types,Extents,Records>` (:2241) with `layout` (:2307), `sum_layout` (:2322), `region` (:2345), `region_source_compatible` (:2360), `record_fields` (:2407), `record_field_dependencies` (:2513), `dependencies` (:2286); free functions `geometry` (:2571), `sum_geometry` (:2690), `sum_layout` (:2711), `carrier_geometry` (:2772), `byte_sequence_geometry` (:2779), `reference_storage_size/layout/projection/region` (:2799-2834), `storage_region` (:2836), `reference_scalar_storage` (:2887), `storage_path` (:2923, the offset walker returning `StoragePathProjection`), then 5 unit tests (:3194-3658).
- `reference_views` (:3660-4412): descriptor/view compatibility (`shared_descriptor_copy_matches` :3669, `descriptor_value_transfer_matches` :3691, `descriptor_reborrow_matches` :3706, `descriptor_reborrow_shapes_match` :3729, `descriptor_geometry_matches` :3767), `is_copyable_carrier` (:3800), `is_shared_reference_carrier` (:3812), `representation_view_declaration` (:3850), `reference_source_storage_path` (:3866), `reference_source_projection_compatible` (:3886), `reference_source_projection_accessible` (:3942), `descriptor_source_projection_accessible` (:4026), `reference_byte_field_compatible` (:4093), scalar width/compatibility (`reference_storage_byte_width` :4136, `reference_storage_view_is_compatible` :4148, `reference_storage_export_byte_width` :4168), `reference_storage_source_compatible` (:4213), `plain_storage` (:4269), `storage_scalars_satisfy` (:4284), `scalar_reference_view_is_total` (:4323), `scalar_reference_view_value` (:4337), `reference_source_compatible` (:4375).
- `resident_references` (:4413-4508): `resident_reference_plan` (:4421), `validate_resident_owner_geometry` (:4467), `validate_resident_owner_fields` (:4479).

### Inputs and outputs
Inputs are almost always `impl Iterator<Item=&StructuralTypeDeclaration> + Clone` (a type table walked by linear `find`/`filter` on `row.id`), plus `terminal_psi` declaration rows (`StructuralParameterDeclaration`, `TerminalDynamicSlot`, `TerminalDynamicReferenceRegion`, `StructuralResultDeclaration`) and, for memberships, a whole `&TerminalModule` (:602). Outputs are `bool`, `Option<TypeId>`, `Result<_, &'static str>` (error strings as the diagnostic), or fresh `Vec`s (`Vec<ReferenceCallResult>`, `Vec<StructuralPathQualification>`). No mutation of inputs. The only interior state is the `OnceCell` graph cache at :598 (built by `descriptors::terminal_borrowed_storage_graph`, :676).

### How it decides (keys)
- Type lookup is by `StructuralTypeId` equality over an iterator (`row.id == root`), e.g. :242, :3817, :4855 area; "exactly one" is enforced by `exactly_one` at :3855 and by taking `next()` twice at :243-248 and :3817-3823; several other lookups use `find`/first match without checking for duplicates (:1591, :1601, :1818, :2087-2091).
- Reference origin coverage (`reference_origin_refines`, :1767) is keyed on place identity (`actual.place == allowed.place`), `access` equality, path-prefix equality of projections (`Element` in the allowed path matches any `FixedIndex` in the actual path, :1789-1792), and `ReferenceOriginExtent::{Projected,WithinRegion}` (:1797-1845). `Projected` demands equal path length (:1798).
- Region validation (`validate_input_region`, :1492) is keyed on `required.admits(access)` and walks segments checking stored view / reference access ceilings; rejects with fixed strings.
- Storage geometry is keyed on the selected target layout (`records.realization(root, application)`, :2616, :3063) or the type's own `Authored`/`Packed` storage; `UnselectedRecordStorage` means every `TargetLayout` record fails closed with `UnresolvedLayout` (:2213-2223).
- Scalar views: `scalar_reference_view_is_total` (:4323) accepts equal types, same-width non-address integers up to 64 bits, or whatever `ScalarRepresentationViewPair::classify` says needs no landing.
- Memberships preservation only handles output leaves whose type is `Reference{access: SharedBorrow}` (:761-767; others `continue`), only sources with `extent == Projected` and `ReferenceCallOrigin::Parameter` (:772-778); anything else sets `complete=false` and drops the membership (:828-831).
- No decision keyed on display names or `format!`; error text are static strings. None seen of label/string-compare keying.

### Cases by shape
- `borrowed_result_leaves::visit`: handles `Reference`, `ElementView`, `ByteSequence(BorrowedView|BoundedView)` as leaves; recurses into `FixedArray` (literal nonzero length only: guard `length.literal() != Some(0)` at :279; non-literal length with a non-empty element roster errors "requires a closed array extent" at :287-289), `Record`, `Mixed`, `Sum`; everything else contributes no leaves (`_ => {}` :309). Zero-length arrays are skipped entirely.
- `reference_leaves` (:1584): only `Reference`, `Record`, `Sum`, `Mixed`, `FixedArray{Literal}`; "`_ => return None`" at :1651 for any other shape that contains a reference (e.g. non-literal array, ElementView).
- `storage_path` (:2923): handles `Sum|Mixed` with Field/Case, `Record` with Field, `FixedArray{Literal}` with `FixedIndex` / `FixedRange`, and `RepresentationView`; `_ => return None` (:3173). `AccessView`/`Accessor` record storage returns None (:3074-3075). Fragmented fields in a sum return None (:3044-3046).
- `geometry` (:2571): `Reference`, `ByteSequence`, `ElementView`, `AtomicCell`, `PrimitiveScalar`, `Record`(TargetLayout/Authored/Packed), `FixedArray`, `Sum`, `Mixed`; `_ => Err(UnsupportedType)` (:2684). `BoundedOwned` byte sequences are `UnsupportedByteSequence` (:2793-2795). Nesting limit hard-coded 128 (:2579; same constant at :4290).
- `reference_storage_byte_width` (:4136): only non-address integers with bits%8==0 and width <=255 bytes, `Boolean`, `Binary32`, `Binary64`; all other scalars `None`.
- `reference_storage_export_byte_width` (:4168): loops over a fixed candidate set (Boolean, f32, f64, signed/unsigned ints 8..=128 step 8) to assert every compatible stored scalar has the same width.
- `reference_storage_source_compatible` (:4213): `(Some(stored), PrimitiveScalar)` and `(None, record_or_fixed_array_shape_pattern!())` only; `_ => false` (:4265).
- `resident_reference_plan` (:4421): only `SharedBorrow|MutableBorrow`; exactly one `access_view_plans` row with `plan.carrier == referent` (:4457-4463).

### Same job elsewhere
- Two leaf enumerators for "reference leaves of a result type": `borrowed_result_leaves` (:183, includes views, errors on cycles/ambiguity) and `reference_leaves` (:1584, references only, returns `None`, different traversal order of Mixed: cases then fields at :1622-1628 vs fields then cases at :307-319).
- The "find existing `ReferenceCallResult` with this path else push" block appears twice inside `bind_public_reference_call_results` (:524-533 and :570-577).
- The "inline byte view at this path" test (`field.field_type.byte_sequence_carrier()` is `BorrowedView|BoundedView`) is written out at :1347-1355, :1565-1573 and similar logic at :3998-4005, :4065-4074.
- Array-element stride/geometry is computed in `geometry` (:2663-2677) and again in `storage_path` (:3125-3133, :3144-3165) rather than through one function.
- `scalar_reference_view_is_total` (:4323) overlaps with scalar view classification in `scalars.rs` (`ScalarRepresentationViewPair`, see section 2; it is called from here).
- `reference_storage_byte_width` (:4136) is a different width table from other layout width helpers (suspected; not read outside unit).

### Unused or one-caller code
- `ReferenceStorageGeometry::record_field_dependencies` (:2513): grep `-w record_field_dependencies` over `omega-rust` = 1 occurrence (its own definition). No callers.
- `reference_scalar_storage` (:2887): 3 occurrences = definition, `lib.rs:121` re-export, one use in `psi/interpreter/terminal-interpreter/src/storage_references.rs:264`. 1 call site.
- `validate_reference_result_completion` (:71): 2 call sites outside (`legalization_reference_custody.rs:6817`, `current_ownership.rs:7287`), both under `omega/`; `cleanup_disposes_reference` 2 sites (`legalization_reference_custody.rs:6790`, `current_ownership.rs:7279`); `borrowed_entry_reference_referent` 2 sites (`legalization_reference_custody.rs:2138`, `current_ownership.rs:7272`). The two `omega/` callers each build their own `ReferenceCompletionOccurrence` list: a shared judgment with two feeders (read none of the feeder code).
- `reference_storage_export_byte_width` (:4168): 2 outside sites (`structural_inputs.rs:34`, `selection_inputs.rs:4031`).
- Other counts (occurrences, incl. def + re-export): `reference_storage_region_source_compatible` 6, `reference_byte_field_compatible` 6, `reference_result_refines_regions` 5, `resident_reference_plan` 5, `bind_public_reference_call_results` 4 (def, re-export, 1 internal call at :473, 1 other), `validate_descriptor_result_regions` 4, `reference_call_preserved_memberships` 4, `reference_result_memberships_weaken` 4, `is_shared_reference_carrier` 3 (def, lib.rs re-export, 1 internal call at :3809). `bind_public_reference_call_results` 4 occurrences = def (:479), internal call (:473), re-export (:1688), `lib.rs` re-export; no outside caller. (Counts are word-match lines incl. re-exports, not call sites.)
- `UnselectedRecordStorage` is the default resolver type parameter.
- No comments in this file claim "single/shared/canonical".

### Stated limits
- The file has almost no comments (header at :1-4 only). Limits appear as error strings: "dynamic result has more borrowed leaves than public regions" (:221), "dynamic literal view has no shared input-region contract" (:1148), "dynamic returned view has unresolved indirect backing" (:1269), "dynamic returned view has an unsupported backing projection" (:1283), "returned origin needs a captured stored-descriptor identity" (:1751).

### Fail-open risks
- `reference_source_projection_accessible` (:3942-4024): the loop `match segment { Referent => .., FixedIndex|RuntimeIndex => .., _ => {} }` ignores all other segment kinds (`_ => {}` at :4020), and in the index arm `runtime_structural_path_tip(...)` returning `None` just skips the ElementView/ByteSequence checks (`declaration` is `None`, :3984-4005); function ends `true` (:4023). A path whose prefix cannot be typed therefore passes with no access ceiling check. Status: open (callers not traced; 22 word-occurrences). `descriptor_source_projection_accessible` (:4026) first calls this function and then adds checks only for the tip, so the same gap applies.
- `reference_source_projection_compatible` (:3886): no `None` accepts; falls through to `false`. Fail closed.
- `reference_origin_paths_may_overlap` (:1975): returns `true` on `RepresentationView` segments, different cases, and when paths run out; conservative direction (more overlap). Not a fail-open for acceptance. Note: the `zip` stops at the shorter path and returns `true` (:1997).
- `preserve_call_memberships` (:716): when a source cannot be resolved the membership is dropped (fewer qualifications retained). Direction is fail-closed.
- `validate_reference_result_completion` is fail-closed (Err on missing parent). `cleanup_disposes_reference` defaults false.
- `inline_view_is_covered` (:1330): paths containing `Element` return false.
- Summary: one open item above; otherwise none seen.

---
## 2. `semantics/src/scalars.rs` (5021 lines)

### Purpose
Semantic judgments about scalar (integer/boolean/float) operations in Terminal Psi: (a) the proof obligation ("goal") each proof-bearing exact/wrapping/saturating integer operation carries, and whether it can be replaced by a cheaper operation once the goal is discharged; (b) a goal-free scalar-leaf evaluator for constant folding; (c) liveness of scalar values/block parameters across a machine (`scalar_demand`); (d) a forward relational analysis of unsigned 64-bit indices (`scalar_ordering`); (e) requirements for refined/bounded scalar fields; (f) scalar "representation views" (float<->bits, bool<->byte); (g) typing of scalar reads through structural paths.

### Shape
- `proof_bearing_scalar` (scalars.rs:6-1711):
  - `canonical_goal` submodule (:28-803): `CanonicalScalarGoal` enum, 6 variants (:42); `kernel_proposition` (:92) builds the `Proposition` to prove, via private `exact_{add,subtract,multiply}_representability_proposition` (:273, :219, :155), `exact_shift_count_proposition` (:571), `exact_shift_left_representability_proposition` (:442), `exact_cast_representability_proposition` (:505), `nonzero_divisor_proposition` (:641), `exact_division_defined_proposition` (:684); `canonical_goal` (:749) maps (schema.goal, input arity) to a goal.
  - `elision` submodule (:804-1193): `ProofBearingScalarLeafElision` (:821: semantics, replacement `OperationKind`, `result_literal`); `elidable_proof_bearing_scalar_leaf` (:845); `canonical_goal_is_discharged` (:931); `discharged_literal` (:961); `literal_substituted_goal`/`literal_scalar_term` (:1008, :1069).
  - Module level: `ProofBearingIntegerPolicyBinding` (:1204) + `proof_bearing_integer_policy_binding` (:1223) maps 11 `OperationSemanticTag`s to numerics `(IntegerPolicyPrimitive, ArithmeticDomain)`; `catalog_goal_shape` (:1267); `ProofBearingScalarSemanticRow{tag,schema}` and its `ALL` table generated by macro `proof_bearing_scalar_rows!` (:1337-1383, 12 rows); `validate_proof_bearing_scalar_semantic_rows` (:1392); `proof_bearing_scalar_semantic_row` (:1420); `ProofBearingScalarLeafSemantics` (:1431); `build_denotation` (:1551); `proof_bearing_scalar_leaf_semantics` (:1665).
- `scalar_array` (:1712-1791): `scalar_array_leaf_path` (:1716, ordinal -> FixedIndex path through nested literal-length arrays), `scalar_array_leaf_shape` (:1751), `scalar_array_leaf_declaration` (:1763).
- `scalar_demand` (:1792-3026): `operation_inputs::inputs` (:1799, exhaustive match of every `OperationKind`, returns `false` = "cannot enumerate"), `ranked_coordinates` (:2003), `retained_rows::{RecordNamedRows, record_named_rows, visit_invocation_evidence_values, visit_erased_value_arguments}` (:2046-2183), `ScalarDemandMode::{KeepAllOperations,DropUnusedTotalOperations}` (:2190), `qualification_parameter_blocks` (:2212), `retained_values_by_machine` (:2254), `complete_scalar_demand` (:2304, mutates `&mut TerminalMachine`: drops unused total ops and unused block parameters + the matching edge arguments), `ScalarDemand::prepare` (:2330, builds `live` and `removed`), `remove_unused_block_parameters` (:2626), tests (:2671-3025).
- `scalar_field_requirements` (:3027-3181): `scalar_field_requires_evidence` (:3033), `..._initialization_evidence` (:3040), `scalar_field_requirement` (:3052), `atomic_resident_requirement` (:3065), `integer_range_requirement` (:3075), `scalar_field_accepts_literal` (:3093), `scalar_field_accepts_zero` (:3125), `scalar_field_entails` (:3146).
- `scalar_leaf_schema` (:3182-3342): plain enums describing a leaf (`ScalarLeafResultShape`, `ScalarLeafOperandShape`, `ScalarLeafDenotation`, `ScalarLeafGoalShape`, `ScalarLeafFactShape`, `ScalarLeafCrashPolicy`, `ScalarLeafWorkPolicy`, `ScalarLeafFrontierPolicy`) and `ScalarLeafSchema` (:3273); `goal_free_scalar_leaf` (:3320).
- `scalar_leaf_semantics` (:3343-3803): `ScalarLeafLiteral` (:3356), `constant_goal_free_scalar_leaf` (:3361, constant folding), `scalar_leaf_inputs` (:3503), `validate_result_shape` (:3558), `validate_operand_shape` (:3572), `denotation_term` (:3630), `goal_free_scalar_leaf_semantics` (:3778).
- `scalar_observations` (:3804-4111): `structural_read_scalar_type` (:3918), one unit test (:3807).
- `scalar_ordering` (:4112-4840): `Effect`, `Binding`, `Edge`, `Step` (public input graph), private `Term{Value,Unsigned(u64)}`, `Relation{left,right,strict}`, `State{relations,comparisons,types,literals,storage}` with `proves` (:4337, reachability over a relation graph), `after` (:4456), `across` (:4566), `intersect` (:4413); `ScalarOrdering::{new,with_contracts,known_unsigned,proves,values_unchanged_since}` (:4673-4838); `contracts::{relations,instantiate_contract_ordering}` (:4117-4206).
- `scalar_representation_view` (:4841-4975): `ScalarRepresentationViewPair::{FloatBits,BitsFloat,BooleanByte,ByteBoolean}` (:4848) with `classify` (:4866), `requires_landing` (:4890), `view` (:4894); `ScalarViewValue` (:4930), `integer_bits` (:4943), `integer_from_bits` (:4958).
- `unchanged_scalar` (:4976-5021): `unchanged_scalar_identity` (:4985) resolves a value through `Incoming` origins to one identity, cycle-guarded by an `active` stack.

### Inputs and outputs
Reads `terminal_psi::Operation`/`OperationKind`/`TerminalMachine`/`TerminalModule` rows, `BTreeMap<ValueId,ScalarType>` / `BTreeMap<ValueId,ScalarLeafLiteral>` maps (:847-848, :3362-3363), `semantic_vocabulary` types (`IntegerType`, `Proposition`, `ScalarTerm`), and `numerics`/`proof_admission` (:3029, :10-16). Produces `Proposition`s, replacement `OperationKind`s, `ScalarLeafLiteral`s, liveness vectors, and (only in `complete_scalar_demand` :2304) mutation of `machine.blocks[*].operations/parameters/terminator arguments`.

### How it decides
- Elision (:845): keyed on the operation tag (via `OperationSemanticTag::for_operation`, :1425), on integer values known from `literals` (a `ValueId -> literal` map), and on whether the goal simplifies to `Proposition::Truth` after literal substitution (:955-957) or special-cased literal nonzero-divisor tests (:937-954). Replacement kind keyed on `OperationKind` variant (:873-923). Add/Subtract replacement additionally requires both operand value types equal the result integer type and the result non-address (:888-893). `IntegerExactCast` replacement requires `source_type.can_widen_to(result_type)` (:917).
- Goal propositions are keyed on `IntegerType` carrier (`Fixed` only; others `Err(...RequiresFixedInteger)` e.g. :159, :576, :645, :689), sign, bits, and operand shape (`ScalarTerm::Value` or `Integer` only; other terms error `...RequiresValueOrLiteralOperand` :395).
- `complete_scalar_demand` (:2304): keyed on `ValueId` equality via sorted vectors + binary search (dense-id style, :2343, :2414); liveness by backward reachability from retained values, rank values, operation inputs (:2568-2593); dominance check for edge arguments via `crate::control_graph::dominators` (:2375, :2458-2461).
- `scalar_ordering::State::proves` (:4337): keyed on `ValueId` and `u64` literal terms; only values whose recorded type is exactly `u64` unsigned participate (`is_index`, :4432).
- Field requirements (:3052): keyed on `StructuralFieldType` variant: `RefinedScalar`, `BoundedInteger`, `AtomicBoundedInteger`.
- `structural_read_scalar_type` (:3918): keyed on `ScalarReadProjection` variants and type declaration ids (exactly-one lookup :3923-3927).
- No decisions keyed on names or strings in this file (only `unreachable!`/`expect` message strings and one test `identities.intern("u64")` at :2877).

### Cases by shape
- Proof-bearing cohort is a fixed list of 12 tags (:1370-1383): `IntegerExactCast`, `ExactIntegerShiftLeft/Right`, `ExactIntegerAdd/Subtract/Multiply/Divide/Remainder`, `WrappingIntegerDivide/Remainder`, `SaturatingIntegerDivide/Remainder`; `proof_bearing_inputs` (:1472) matches the same list and returns `None` for the rest.
- `ExactArithmeticRepresentable` handles only `ExactIntegerAdd|Subtract|Multiply` expressions (:131-146); other shapes `Err(ExactArithmeticExpressionShapeMismatch)`.
- Elision replacement table (:873-923): exact shifts, exact add/sub (type-matched), exact multiply, exact cast (widening only); `_ => return Ok(None)` for exact divide/remainder, wrapping/saturating divide/remainder except through `discharged_literal` (:978-1005), which handles Subtract(x,x), Multiply by literal 0, shifts of literal 0, remainder by literal 1, wrapping/saturating remainder by 1 or signed -1.
- `inputs()` (:1799): handles every `OperationKind`; returns `false` for `CallStructural` (:1998) and for any call with non-empty `crash_continuations` (:1979, :1991).
- `ranked_coordinates` (:2011): only `TerminalRankedScc::Natural`; empty component lists or empty rank lists return `None`.
- `scalar_array_leaf_*` (:1716, :1763): only nested `FixedArray` with literal lengths ending in a `PrimitiveScalar`; `PrimitiveScalar` with no array returns `None` (`if array`, :1783).
- `ScalarRepresentationViewPair::classify` (:4866): only float<->same-width fixed integer (32/64), bool<->8-bit fixed integer.
- `integer_bits` (:4943): integers up to 64 bits.
- `validate_operand_shape` (:3585): shape table with `_ => false`.
- Comparison: for Boolean-result `BinaryInteger` ops left/right must be equal types or both non-address (:3600-3604).

### Same job elsewhere
- Two parallel tables of the scalar leaf: `ProofBearingScalarSemanticRow::ALL` (:1340) and the goal-free schema rows reached through `operation_semantic_row(..).goal_free_scalar_leaf()` (:3366, row table lives in `semantic_rows`, outside this unit); `build_denotation` (:1551) and `denotation_term` (:3630) each rebuild the `ScalarTerm` for an operation from (schema.denotation, inputs): one for proof-bearing, one for goal-free.
- `integer_bits`/`integer_from_bits` (:4943, :4958) are used by `references.rs:4320` (`scalar_reference_view_value`, `reference_storage_view_is_compatible`).
- The "unsigned 64-bit index" type is constructed in several places (`IntegerType::new(IntegerSign::Unsigned, 64)` at :4126, :4179, :4436, :4546, :3854 test, :3965).
- `retained_rows::record_named_rows` (:2095) and `visit_invocation_evidence_values` (:2058) are the shared enumerators of "values/operations a record names"; `visit_erased_value_arguments` (:2076) is also used inside `ScalarDemand::prepare` (:2480-2518).
- `scalar_demand::complete_scalar_demand` is called from two lowering passes: `psi/.../06_checked-trees-to-lowered-psi/src/machine_lowering.rs:1642` and `07_lowered-psi-to-lowered-psi/src/scalar_and_value_passes.rs:580` (grepped); both also call `qualification_parameter_blocks` and `retained_values_by_machine`.

### Unused or one-caller code (grep `-w` over `omega-rust/**/*.rs`, line matches)
- `elidable_proof_bearing_scalar_leaf` (:845): 3 lines: definition, `scalars.rs:1199` re-export, `lib.rs:574` re-export. No call site anywhere else in `omega-rust` (grepped).
- `proof_bearing_integer_policy_binding` (:1223): 3 lines, one internal use at :1406; re-export lib.rs:575.
- `validate_proof_bearing_scalar_semantic_rows` (:1392): called inside `proof_bearing_scalar_semantic_row` (:1423), so the whole table is validated on every lookup (each call to `proof_bearing_scalar_leaf_semantics` :1669); no other caller.
- `visit_invocation_evidence_values` (:2058): 3 lines; one internal caller (:2106).
- `instantiate_contract_ordering` (:4174): 1 external call (`optimization-unit/semantics/src/current_ownership.rs:1850`).
- `ScalarOrdering::values_unchanged_since` (:4790): 2 callers, both in `storage.rs:2136`, `:2144`.
- `scalar_field_initialization_requirement` (:3045): 1 external call (`optimization-unit/semantics/src/unit_validation.rs:2170`).
- `unchanged_scalar_identity` (:4985): 3 call sites: `descriptors.rs:12792`, `bytes.rs:974`, and `optimization-unit/semantics/src/descriptor_results.rs:2756` (omega side).
- `qualification_parameter_blocks` and `retained_values_by_machine`: each 2 call sites (machine_lowering.rs:1617/1627, scalar_and_value_passes.rs:594/612).
- `scalar_field_entails` (:3146): callers `structural.rs:114`, `descriptors.rs:1154, 1286-1287, 1336`, `emission.rs:279, 4108` (grepped).
- No comment in this file says "single/shared/canonical" for a function; `CanonicalScalarGoal` and `canonical_goal` are named canonical but not documented as such.

### Stated limits
- Mostly error variants rather than comments: `ExactArithmeticRequiresFixedInteger`, `ExactShiftCountRequiresFixedValueInteger`, `NonzeroDivisorRequiresFixedInteger`, `ExactDivisionRequiresFixedInteger` (:160, :578, :646, :690): non-`Fixed` carriers are rejected.
- `// ... ` comments: `unreachable!("exact-multiply projection is selected by expression shape")` (:170) etc.; header comment at :1-4 only.

### Fail-open risks
- `scalar_field_entails` (:3146-3180): after the carrier check it does `let Some(goal) = scalar_field_requirement(required, FIRST) else { return true }` (:3159-3161). `scalar_field_requirement` returns `None` for any field type that is not `RefinedScalar`/`BoundedInteger`, including `AtomicBoundedInteger` (:3052-3063; `atomic_resident_requirement` is a separate function, :3065). `StructuralFieldType::scalar_type()` returns `Some` for `AtomicBoundedInteger` (`types_and_values.rs:975`). So `required = AtomicBoundedInteger(range)` with `established = Scalar(same carrier)` is reported as entailed. Callers where `required` can come from a type's `scalar_restriction` or a declared field: `structural.rs:114-117`, `emission.rs:279, 4108`. Status: open for `emission.rs:279/4108` and `descriptors.rs` callers (not read); for `structural.rs:114` the `required` is `carrier.scalar_restriction` and structural.rs:4170-4189 only recognises `BoundedInteger`/`RefinedScalar` restrictions, so atomic is probably not reached there (inferred). By contrast `scalar_field_accepts_literal` (:3116-3117) explicitly falls back to `atomic_resident_requirement`.
- `scalar_field_accepts_literal` (:3116-3120): `else { return true }` when no requirement; guarded by the carrier-type equality at :3100 and literal-shape check at :3097-3099; traced: plain scalar fields only.
- `scalar_field_entails` first line `established == required` -> true (:3150), structural equality.
- `elidable_proof_bearing_scalar_leaf` (:845): `canonical_goal_is_discharged` `_ => goal.kernel_proposition().is_ok_and(== Truth)` (:955-957): fail-closed on error. Replacement of `ExactIntegerMultiply` by wrapping multiply (:906-911) carries no operand-type check unlike add/sub (:888-893); it relies on the goal being `Truth` (`exact_multiply_representability_proposition` requires `exact_integer_operands` with equal types, :172-178). Traced from the definitions in this file: not reachable with mismatched types because the goal construction errors.
- `complete_scalar_demand` / `ScalarDemand::prepare`: returns `None`/`false` on any inconsistency (duplicate ids, unreachable block, arity/type mismatch, undominated argument, any `Terminator::Crash`, `inputs` false). Fail-closed. When `mode == KeepAllOperations` operations are never dropped (:2314).
- `ScalarOrdering::with_contracts` (:4681): malformed step graphs leave `states` all `None`, so `proves`/`known_unsigned` answer `false`/`None` (:4693-4706). Fail-closed.
- `State::across` (:4587-4600): mismatched binding arity/types returns `Self::default()` (no facts). Fail-closed.
- `intersect` (:4413): joins by retaining only common facts; storage entries differing become `None`. Fail-closed.
- `structural_read_scalar_type`: returns `None` on any unknown. Fail-closed.
- `unchanged_scalar_identity`: cycles return `None` (:4998).
- Other: none seen.

---
## 3. `semantics/src/storage.rs` (3683 lines)

### Purpose
Judgments about where values live: (a) a serial-activation flow checker that proves "activation claims" (linear exclusive byte-descriptor grants carved out of a bounded backing) and the loans taken from them are established, split, merged, released and never escape; (b) rules for what a parameter/block-parameter storage place is (arrival storage); (c) alias/projection bookkeeping for borrowed storage and region overlap; (d) foreign/boundary storage extent dependencies; (e) collection/element-view backing and representation layout; (f) storage dependency collection, projected storage regions, vacant-local and zero-valid-storage checks.

### Shape
- `activation_storage` (storage.rs:6-1525): `ActivationClaimShape{claim_type,claim_field}` (:15); `activation_claim_shape(_in)` (:20, :27) checks an `EstablishActivationClaim` operation's backing/attempt/claim types; submodule `flow` (:122-1523): private `Place{root,path}` (:135), `Live` (:140, site/bound/place/claim/pending/counts_demand/split_origin/split_role), `Loan` (:151), `State{live,loans}` (:158) with `select/sites/assemble/copy_value/relocate/parents/close_loans/rebind_claims`; `ModuleActivation` (:545) computes the set of machines that may suspend by fixpoint (:585-609); `validate_machine` (:530), `validate_module` (:537), `validate_flow` (:638) which does a worklist walk over `(block, State)` pairs and returns the maximum co-live claim bound (`u64`); one test module (:1408).
- `arrival_storage` (:1526-1784): `view_descriptor_projected_qualifications` (:1534), `view_descriptor_parameter` (:1551), `borrowed_storage_parameter` (:1587), `borrowed_storage_reference_ceiling` (:1603), `descriptor_parameter_access` (:1622), `arrival_parameter` (:1652), `root_before_edge` (:1705), `projection_before_edge` (:1715), `StorageNames`/`storage_names` (:1755, :1765).
- `borrowed_storage` (:1785-2184): `Projection` (:1795; address steps, `path`, `exact`, `indirect`, `window`, `captured_at`) with `new/at/window/with_window/without_runtime_coordinates/enclosing_storage/fixed_address/reference_origin_path/then`; `Region` (:2083), `regions_overlap_at` (:2102), `window_excludes_index` (:2122), `resolve` (:2152) over `AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>` (:1792).
- `calling_storage` (:2185-2609): `ForeignRecordStorage` (:2190), `foreign_record_storage` (:2214), `foreign_opaque_carrier` (:2251), `foreign_storage_extent_dependencies` (:2268), `calling_abi_extent_dependencies` (:2352), `callback_storage_has_slots` (:2378), `storage_has_slots` (:2386); tests (:2432-2608).
- `collection_backing` (:2610-2775): `CollectionBacking{extent,storage}`, `CollectionExtent::{Fixed,LiveLength,StoredLength}`, `collection_view_descriptor` (:2641), `collection_backing` (:2676), `collection_view_backing` (:2733), `collection_shape_view_backing` (:2743), `collection_view_preserves_coordinates` (:2766).
- `element_representation` (:2776-3027): `ElementRepresentationLayout{size,alignment,leaves}`, `scalar_size` (:2803), `element_representation_layout` (:2837, budget 16384 leaves, depth 128), `bit_intervals` (:2948), `element_representation_relation` (:2988).
- `representation_regions` (:3028-3260): `representation_element_layout` (:3052), `representation_byte_extent` (:3079), `ElementViewRegionGeometry` (:3108), `element_view_region_geometry`/`element_view_shape_region_geometry` (:3120, :3131), `whole_representation_count_goal` (:3214), `representation_region_goal` (:3226), `representation_region_length` (:3245).
- `run_custody` (:3261-3339): `empty_run_release_contract` (:3277).
- `storage_dependencies` (:3340-3481): `inline_storage_extent_dependencies` (:3348), `storage_operand_dependencies` (:3393).
- `storage_regions` (:3482-3583): `StorageRegion`, `crosses_unaliased_reference` (:3502), `projected_storage_region` (:3508), `canonical_field_storage_region` (:3541).
- `structural_locals` (:3584-3611): `validate_vacant_structural_local` (:3588).
- `zero_valid_storage` (:3612-3683): `zero_valid_record_storage` (:3620), `scalar_declaration_accepts_zero` (:3676).

### Inputs and outputs
Input: `TerminalModule`/`TerminalMachine`/`Block`/`Operation` rows, `&[StructuralTypeDeclaration]` or iterators over them. Outputs: `Result<_, &'static str>` / `Result<_, String>` diagnostics, booleans, small value structs. No mutation of module data; `flow::validate_flow` returns the maximum co-live claim demand (computed through `extents::compose_claim_bounds`, :1241, outside this unit).

### How it decides
- Activation flow (:638-1405): keyed on `PlaceId` root + structural path (`Place::under` is root equality plus path prefix, :170), `OperationId` of the establishing site, `ClaimId`, and `OperationKind` variant. A claim is "pending" until a `StructuralCaseMembership` observation or `StructuralCase` terminator selects the established case (:225-240, :1266-1295).
- Suspension: `may_suspend` (:552) is true when `operation.suspension_crossing` is set, for `BoundaryCall`/`CallDynamic`, or when the direct callee is not in the module or is itself suspending; fixpoint over machines (:585-609). Keyed on `MachineId` membership.
- Arrival storage: keyed on `StructuralPlaceKind::{Parameter,BlockParameter}`, parameter `access` (`reading_borrow_pair_pattern!`), `multiplicity` (:1555-1562, :1591-1592), and uniqueness of declarations (`exactly_one`, or `matching.next()` twice).
- `foreign_record_storage` (:2214): selects the layout realization by string equality `row.target == target` (:2232) where `target: &str` is the target name (e.g. tests use `"macos_arm64"`, `"linux_x86_64"`, :2490, :2604); `callback_storage_has_slots` passes the same string to `native_layouts.for_target(native, target)` (:2407). So target selection here is keyed on spelling.
- `window_excludes_index` (:2122): keyed on `ValueId`s and `ScalarOrdering` facts (`values_unchanged_since`, `proves`), plus positions `(BlockId, u32)`.
- `Projection::new` (:1815): derives `exact` and `indirect` from the path: prefix of `Field|FixedIndex` is exact, a `Referent` after the first segment makes it indirect (`crosses_unaliased_reference`, :3502 skips segment 0).
- Zero validity: keyed on type shape and `scalar_restriction` (:3676-3681).
- No decisions keyed on display names or `format!` except the `target` string above; `format!` is used only to build error messages (:3361, :3402, ...).

### Cases by shape
- `activation_claim_shape_in` (:27): "[established, rejected] = cases" exactly two outcomes (:71), `[field] = established.fields` exactly one claim (:83), backing must be `ByteSequence(BoundedOwned{capacity: bound})` (:59-61), claim must be `ByteSequence(BorrowedView{access: Some(MutableBorrow)})` (:98-101), `[binding] = result.claims` exactly one (:105).
- `State::validate_paths` (:184): only `Field|Case|FixedIndex` path segments, message "activation transport requires a finite inline carrier path" (:196); path length cap `types.len()*2` (:191).
- `validate_flow` operation arms handled: `EstablishActivationClaim`, `SplitActivationClaim`, `MergeActivationClaim`, `ReleaseActivationClaim`, `EstablishRecord`/`EstablishStructuralCase`, `EstablishStructuralArray`, `MoveStructuralField`, `StoreStructuralField`, `StructuralLeafCopy`/`StructuralCaseLeafCopy`, `TransformContent`, `StructuralByteViewFieldStore`, `MoveStructuralValue`, `ByteSequenceSubslice|ElementViewSubslice|EstablishReference|EstablishAccessReference|EstablishElementView|EstablishMutableElementView`, `ReleaseReference`; `_ => {}` for the rest (:1213). Results containing a claim must come from one of 9 listed kinds (:1223-1234). Split requires `0 < bound_at < parent.bound` (:808). Merge pops the last two live entries (:892-893) and requires split roles 1 and 2 with the same origin.
- `collection_shape_backing` (:2685): `ElementView`, literal-length `FixedArray`, `ByteSequence` only when the element type is an unsigned 8-bit non-address integer; `_ => None`.
- `collection_view_descriptor` (:2641): `BoundedView` only when capacity equals `Fixed` extent, element is plain `u8`, region `WholeCollection`, and the source path does not end in `FixedRange` (:2653-2668).
- `element_representation_layout::visit` (:2841): `PrimitiveScalar`, literal-length `FixedArray`, `Record`; record fields may only be `Scalar | BoundedInteger | AtomicBoundedInteger | IeeeFloat | Structural` (:2893-2906) and any erased field returns `None` (:2890-2892).
- `scalar_size` (:2803): only `Boolean`, non-address integers of 8/16/32/64 bits, `Binary32`, `Binary64`.
- `representation_byte_extent` (:3087): literal-length `FixedArray` of `u8`, or `BoundedView`.
- `empty_run_release_contract` (:3277): exact shapes `[storage, count]` fields (:3304) and `[_base, length]` (:3319); the extent record's first field is bound to `_base` and not inspected; count/length must be unsigned 64-bit (:3333-3337).
- `foreign_storage_extent_dependencies::visit` (:2273): `Sum` and `Mixed` return `Err("case data is not yet classifiable as a boundary value; pass it by reference")` (:2323).
- `zero_valid_record_storage` (:3620): `Record`, `FixedArray`, `Sum` (first case only), `Mixed`, scalar, `BoundedOwned` byte sequence; `_ => false`.
- `projected_storage_region` (:3508): path truncated at the first `RuntimeIndex | RepresentationView | Referent`.

### Same job elsewhere
- `align(value, alignment)` is duplicated: `storage.rs:2816` and `references.rs:2140` (read both; identical bodies).
- Scalar byte widths: `storage.rs:2803 scalar_size` (8/16/32/64-bit integers only) vs `references.rs:4136 reference_storage_byte_width` (any non-address integer with bits % 8 == 0). `element_representation_layout` calls `reference_storage_layout` (:2887, :2929) and `reference_storage_projection` (:2932) from references.rs and then rejects the layout unless the two agree leaf by leaf (:2933-2938).
- "element is plain `u8`" test (unsigned, 8 bits, not address, no restriction) is written out at `storage.rs:2657-2662`, `:2705-2711`, `:3097`, and at `references.rs:2087-2090`, `:4227-4229`, `:4280-4281`.
- Hand-rolled "find the single declaration with this id" via `filter(..)` + `next()` twice appears at `storage.rs:1542-1546`, `:1565-1570`, `:1596-1600`, `:3592-3600` next to calls to `exactly_one` (`:1656`, `:2739`, `:3083`) and `terminal_psi::find_structural_type` (:52, :66, :90).
- Storage extent dependency collection: `inline_storage_extent_dependencies` (:3348) and `foreign_storage_extent_dependencies` (:2268) and `ReferenceStorageGeometry::dependencies` (references.rs:2286) each walk a type graph collecting `ScalarApplicationId`s with different input shapes (BTreeMap of declarations; module; type iterator).
- Reference-flow loans: `activation_storage::flow::State.loans` (:151) tracks derived loans from claims; reference custody is also tracked in `references.rs` (`reference_completion`) (suspected same-subject overlap; not compared further).
- Uses `BTreeMap`/`BTreeSet` keyed by dense ids at :127, :1530, :1788, :2610 imports and `AliasSources` (:1792), `declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>` (:3350, :3395); `roots: BTreeMap<PlaceId, StructuralTypeId>` (:659). (AGENTS.md says dense-id tables should be flat vectors; I did not check `tools/dense_id_maps.txt` for exemptions.)

### Unused or one-caller code (grep over `omega-rust/**/*.rs`, `terminal_semantics::` and `crate::` paths)
- `collection_view_backing` (:2733): no use outside storage.rs other than the `lib.rs:678` re-export.
- `element_view_region_geometry` (:3120): no use outside storage.rs other than `lib.rs:667` re-export (the `_shape_` variant is the one called internally; its outside use not separately counted).
- `representation_region_length` (:3245): no use outside storage.rs other than `lib.rs:668` re-export.
- `zero_valid_record_storage` (:3620): only a self-recursive call (:3648) and the `lib.rs:614` re-export; no outside call site.
- `element_representation_layout` (:2837): outside storage.rs only the `lib.rs:674` re-export; internal callers `:2997-2998`, `:3056`.
- `validate_flow` (:638): 1 outside call (`optimization-unit/semantics/src/unit_validation.rs:72`); `activation_claim_shape_in` 2 outside calls (`unit_validation.rs:41`, `:7347`); `validate_machine`/`validate_module` are used by the in-file test and `ModuleActivation` (outside callers not individually traced because the names are generic).
- `foreign_record_storage` and `foreign_opaque_carrier`: 1 outside call each (`backend/layout/src/boundary_values.rs:465`, `:412`). `foreign_storage_extent_dependencies` and `calling_abi_extent_dependencies`: 1 outside call each (`terminal-installation/src/scalar_applications.rs:2041`, `:1983`). `inline_storage_extent_dependencies`: 1 (`terminal-interpreter/src/engine_execution.rs:624`).
- `representation_region_goal`: 1 outside call (`unit_validation.rs:1624`). `projected_storage_region`: 1 outside call (`current_ownership.rs:9131`). `scalar_declaration_accepts_zero`: 1 inside this crate (`structural.rs:5443`).
- Total line-count of a combined name grep for `element_view_shape_region_geometry|representation_element_layout|representation_byte_extent|whole_representation_count_goal|collection_view_descriptor|collection_view_preserves_coordinates|element_representation_relation|empty_run_release_contract|callback_storage_has_slots|storage_operand_dependencies|ForeignRecordStorage|storage_names|regions_overlap_at` is 82 lines across 22 files (command: Grep count mode) — these have outside users; I did not split by name.
- No comment in this file calls something single/shared/canonical.

### Stated limits
- "live activation claims at suspension exceed this serial realization" (:723), "simultaneous live instances of one activation site exceed this serial realization" (:759), "activation-bearing content transformation exceeds this serial realization" (:1122), "serial activation replay cannot conflate live occurrences of one loan formation" (:1158, :1185).
- "case data is not yet classifiable as a boundary value; pass it by reference" (:2323).
- "activation-bearing store needs proof of activation-local destination storage" (:1065); "activation-derived structural call result needs a retained origin mapping" (:735); "activation-bearing copy needs exact reference backing correspondence" (:276).
- File header (:1-4) is the only prose comment.

### Fail-open risks
- `validate_flow` operation match `_ => {}` (:1213): operations not in the 14 handled kinds do not change `State`. Compensating guards: structural call results/arguments (:726-749) and the "result contains a claim must be one of 9 kinds" check (:1216-1239). An operation of another kind that takes a claim-bearing place as an input without producing a claim-bearing structural result is not examined. Status: traced partly (only reads of this file); callers beyond `unit_validation.rs:72` and `ModuleActivation` not traced; I did not enumerate which other `OperationKind`s take structural arguments.
- `validate_flow` edge handling (:1263): `arguments.get(index).copied().unwrap_or(&[])` treats a missing argument list as empty, and `zip` with the target's `structural_parameters` (:1304, :1324, :1371) silently truncates if the counts differ, so live claims bound to unmatched arguments are not relocated. Whether differing counts can reach here depends on earlier structural validation (not read). Status: open.
- `validate_flow` Conditional terminator: `if let Some(...) = observations.get(condition)` (:1292): a condition that is not a recorded `StructuralCaseMembership` observation leaves claims pending (fail-closed direction since pending claims block suspension and release).
- `view_descriptor_projected_qualifications` (:1539): empty `projected` returns `true` without looking up the type; correct (nothing to check).
- `zero_valid_record_storage` (:3651, :3660): `Sum`/`Mixed` accept when the first case's fields are zero-valid; the code assumes the all-zero tag is the first case with no comment at the site. A sum whose all-zero tag does not select its first case would be accepted. Status: open (not traced).
- `zero_valid_record_storage` Record arm (:3638): the per-field decision is delegated to the caller-supplied closure; erased fields are not filtered here. Status: open (single caller `structural.rs`/not traced).
- `Region::overlaps` / `resolve` / `window_excludes_index`: unknown or cyclic alias chains produce `root: None` which overlaps everything (:2092-2093, :2176-2181): conservative.
- `foreign_opaque_carrier` (:2251): if no opaque representation row exists and the type has no `quotient_carrier`, returns `Ok(None)` and the type is treated as plain storage (:2260-2265); an opaque type with no row and no carrier link is therefore accepted as ordinary. Status: open.
- Others: none seen.

---
## 4. `semantics/src/structural.rs` (5804 lines)

### Purpose
Judgments about owned structural values and their type declarations: typing a path through a record/sum/array/reference (`primitive_place`, `record_field`, `static_path`), inline carriers for scalar/byte fields, nominal cleanup targets, which claim-bearing paths an owned value still holds (`owned_structure`), residual pieces left after partial moves (`partial_affine`), shared-mutation classification, claim catalogues, the table of structural/effect operations and the observation each one produces (`structural_effect`), whether a stored value is disposable, structural-window projections, and "all-zero" construction of a type (`zeroed_structural`).

### Shape
- `atomic_cell` (structural.rs:6-29): `atomic_cell_type` (:11).
- `inline_carriers` (:30-410): `InlineCarrierError` (:39), `inline_carrier_declaration` (:44), `validate_inline_carriers` (:75), `inline_field_presentation` (:134); 5 unit tests (:155-409).
- `nominal_cleanup` (:411-462): `target` (:417) finds the cleanup machine for a `NominalAffineCleanup`.
- `owned_structure` (:463-1069): `complete_claims::{validate_complete_claim_frontier (:479), ClaimTypes (:512), contains_claim (:518), visit (:545)}`, `child_types` (:672), `contains_owned_boundary` (:706), `validate_accounted_descendants` (:731), `selected_contains_owned_boundary` (:843), `validate_owned_claim_frontier` (:876), `selected_owned_children` (:1004), `selected_children` (:1014).
- `partial_affine` (:1070-1334): `partial_affine_residuals` (:1095, two-pass count-then-emit over `visit`/`fields_residuals`/`child`), `is_partial_affine_path` (:1078).
- `primitive_place` (:1335-2175): `primitive_place_type` (:1347), `fixed_array_place_shape` (:1376), `primitive_projection_type/declaration/restriction` (:1403, :1418, :1452), `primitive_read_requirement` (:1474), `borrowed_primitive_projection_declaration` (:1487), `primitive_read_projection_type` (:1542), `primitive_read_field` (:1556), `canonical_primitive_path`/`_read_path` (:1576, :1590), `canonical_static_projection` (:1614), `runtime_structural_path_tip` (:1646), `runtime_structural_copy_type` (:1667), `projection_stays_in_inline_storage` (:1700), `inline_backing_container` (:1732), `StructuralStorageSelection` (:1762), `stored_descriptor_paths` (:1814), `byte_carrier_geometry_matches` (:1875), `projection_storage_dependencies` (:1937), the core walker `walk_primitive_projection_with_policy` (:1962) returning private `ProjectionTip::{Declared,ScalarField,ByteElement}` (:1861), `projection_tip_with_policy` (:2156).
- `record_field` (:2176-2480): `sequence_field_allows_observation` (:2190), `record_field_carrier` (:2232), `case_payload_carrier` (:2255), `observed_field_declaration` (:2278), `static_field_projection` (:2298), `projected_field_declaration` (:2328), `runtime_structural_field` (:2342), `observed_field_backing_access` (:2374), `walk_record_fields` (:2394).
- `shared_mutation` (:2481-2625): `SharedMutation::{None,Inline,Unbounded}`, `shared_mutation` (:2496).
- `static_path` (:2626-2976): `constant_runtime_index` (:2638), `runtime_index_bound` (:2654), `runtime_index_selects` (:2688), `structural_paths_select_same_location` (:2696), `structural_paths_may_overlap` (:2713), `interior_member_borrow` (:2774), `canonical_structural_path_tip` (:2799), `canonical_case_leaf_type` (:2810), `canonical_structural_storage_path` (:2821), `canonical_storage_dependencies` (:2831), `canonical_projection_type` (:2841), `structural_memberships_disjoint_from_write` (:2941), `qualification_path_carrier` (:2952).
- `structural_claims` (:2977-3075): `structural_claim_paths_may_overlap` (:2984), `ResultClaimCatalogError` (:2996), `validate_result_claim_catalog` (:3002), `result_claim_paths_match_returns` (:3036), `matches_paths` (:3043).
- `structural_declarations` (:3076-3094): `structural_declaration`, `structural_shape` (first-match lookup by id).
- `structural_effect` (:3095-5054): `StructuralEffectObservation` (37 variants, :3264-3454), `structural_effect_leaf_observation(_in)` (:3458, :4039) mapping `(StructuralEffectAction, OperationKind)` to an observation, equation builders `byte_equal_equation` (:3111), `byte_predicate_equation` (:3214), `literal_length_equation` (:3159), `subslice_length_equation` (:3181), `element_*` (:4084-4268), schema enums (:4330-4460), `StructuralEffectLeafSchema` (:4453), macro table `structural_effect_rows!` (:4551-4625, 37 rows), `structural_effect_semantic_row` (:4627), `validate_structural_effect_semantic_rows` (:4637), `validate_structural_effect_schema` (:4649), `validate_structural_effect_result` (:5002), `subslice_extent_equation` (:5035).
- `structural_store` (:5055-5210): `structural_store_value_admitted` (:5065), `discardable_store_value` (:5109), `disposable_storage` (:5128), `fields_are_disposable` (:5183).
- `structural_window` (:5211-5313): `structural_window_origins` (:5221), `structural_window_selects_child` (:5244), `structural_window_projection` (:5263).
- `zeroed_structural` (:5314-5804): `ZeroedStructuralLeaf` (7 variants, :5328), `MAX_ZEROED_LEAVES = 131_072` (:5325), `zeroed_structural_shape` (:5396), `zeroed_structural_fields_satisfy` (:5403), `zeroed_declaration_graph` (:5412), `zeroed_structural_leaves*` (:5559-5581), `zeroed_projected_byte_qualifications_hold` (:5767).

### Inputs and outputs
Inputs: `impl Iterator<Item=&StructuralTypeDeclaration> + Clone` or slices; `terminal_psi` path segments (`StructuralPathSegment` runtime paths, `CanonicalStructuralPathSegment` declaration-id paths); `TerminalMachine` (for `nominal_cleanup::target`, `constant_runtime_index`, `structural_store_value_admitted`); `Operation`/`OperationKind` for `structural_effect`. Outputs: `Option<TypeId/ScalarType/Vec<..>>`, `bool`, `Result<_, &'static str>`, `Proposition`s, `StructuralEffectObservation`. No mutation except the caller-provided `BTreeSet` dependency sinks (`projection_storage_dependencies` :1937, `canonical_storage_dependencies` :2831).

### How it decides
- Path typing is keyed on type id equality over declaration iterators and on field `identity` (`TerminalIdentityIndex`) for runtime paths versus field `id` for canonical paths (`live_field` at :2068 vs `live_field_by_id` at :2181/:2470; `exactly_one(... field.id == *identity)` at :2895). Uniqueness enforced by a local `unique` closure (:1971-1977) or `exactly_one`; `structural_declaration`/`structural_shape` (:3081, :3088) take the first match without a uniqueness check.
- `shared_mutation` (:2496) keyed on shape: atomic cells and atomic fields `Inline`; non-shared-borrow views/refs/bytes `Unbounded`; unknown/ambiguous type `Unbounded` (:2524-2529); recursion returns `None` (:2518-2520).
- `structural_paths_may_overlap` (:2713): keyed on equal segments, `AllElements`/`RuntimeIndex` wildcards (overlap), `FixedRange` interval overlap; `_ => left == right` (:2766).
- `interior_member_borrow` (:2774): keyed on access kind pair and strict path-prefix with at least one non-`RepresentationView` suffix segment.
- Observation mapping (:3471): keyed on the pair `(schema.action, operation.kind)`; unmatched pair is `Err(StructuralEffectActionShapeMismatch)` (:4030).
- Disposable storage (:5109): keyed on `multiplicity`, `no_code_affine_cleanup`, `descriptor_cleanup` flags and shape.
- Zeroed types: sum/mixed zero value is always case 0 (`cases.first()`, :5481, :5730, :5782); tag scalar type hard-coded as signed 32-bit (`IntegerType::new(IntegerSign::Signed, 32)`, :5731) regardless of target tag width (`TargetCarrierLayout.case_tag_byte_size` exists in references.rs:2753-2756).
- No decisions keyed on display names or string compare; strings are only error messages.

### Cases by shape
- `inline_carrier_declaration` (:44): fields `Scalar|IeeeFloat|BoundedInteger|RefinedScalar` with a `PrimitiveScalar` carrier of equal scalar type, or byte sequences with matching geometry (`byte_carrier_geometry_matches` :1875); `_ => None`.
- `partial_affine_residuals` (:1095): allows paths of `Field|FixedIndex|Case` only; `Record` with zero fields returns `None` (:1154); `FixedArray` only literal nonzero length (:1160-1163); `Sum|Mixed` only if some moved path begins with `Case`, all moved paths must agree on one case (:1196-1209); `_ => return None` (:1247). Erased field returns `None` (:1263). Byte fields: `BoundedOwned` is inert, view carriers need an inline carrier (:1269-1272).
- `walk_primitive_projection_with_policy` (:1962): `Referent` on `Reference`; `Field` on record/mixed/payload; `FixedIndex` on `FixedArray` (literal bound check only when length is literal, :2092), `ElementView`, `ByteSequence`; `RuntimeIndex` on `FixedArray|ElementView|ByteSequence`; `FixedRange` only before a `RepresentationView`; `_ => return None` (:2137).
- `canonical_projection_type` (:2841): `Referent|Case|Field|FixedIndex`; non-literal array length is accepted only when collecting `dependencies` (:2918-2920).
- `plain_scalar_field` (:1867): only float-typed fields are "plain" scalar fields; integers/booleans get no plain scalar typing there (they are handled via `PrimitiveScalar` carrier declarations).
- `primitive_read_field` (:1556): fields `Scalar(Boolean)`, `Scalar(Integer)`, `BoundedInteger`, `RefinedScalar` only.
- `element_view_leaf` (:4146): `Field|FixedIndex|RuntimeIndex` segments, record field types `Structural|BoundedInteger|RefinedScalar|Scalar|IeeeFloat`; `scalar_restriction` only `None|BoundedInteger|RefinedScalar` (:4170-4189, `_ => None`).
- `validate_structural_effect_schema` (:4649): one hand-written match arm per action (38 actions) checking the exact result/custody/external/frontier tuple.
- `disposable_storage` (:5128): `AccessView|Accessor` records not disposable (:5149-5153).
- `zeroed_declaration_graph` (:5412): `PrimitiveScalar`, `AtomicCell`, `FixedArray`, plain `Record`, `Sum`, `Mixed`; `_ => return false` (:5478) so `Reference`, `ElementView`, `ByteSequence` types are not zero-constructible except as fields via `zeroed_byte_field` (:5545: `BoundedOwned` or shared-view byte sequences whose predicates hold for empty).
- `zeroed_structural_leaves_impl` (:5581): leaf classes `ArrayScalar`, `EmptyScalarArray`, `ByteArray` (literal nonzero length of unsigned 8-bit), recursive literal arrays, sums, records.

### Same job elsewhere
- Several separate walkers type a path through declarations: `walk_primitive_projection_with_policy` (structural.rs:1962), `canonical_projection_type` (:2841), `walk_record_fields` (:2394), `structural_read_scalar_type` (scalars.rs:3918), `storage_path` (references.rs:2923), `resolve_reference_origin_type` (references.rs:2036), `complete_claims::visit` (:545). Each re-implements Field/Case/FixedIndex/Referent stepping with slightly different accepted segment sets.
- Two "does this type contain a claim" functions: `complete_claims::contains_claim` (:518, keyed on `owns_linear_claim`) and `activation_storage::flow::contains_claim` (storage.rs:378, keyed on a precomputed `claim_types` set).
- Child-field enumeration: `child_types` (:672), `selected_children` (:1014), `fields_residuals` (:1252), `inline_storage_extent_dependencies` (storage.rs:3348), `fields_are_disposable` (:5183), `fields_mutation` (:2579); each loops fields filtering `relevance.is_erased()`.
- First-case-is-zero convention at structural.rs:5481, :5730, :5782 and storage.rs:3651, :3660.
- "unsigned 8-bit non-address integer" test at structural.rs:5698-5702, :5787-5789, :3097 (storage.rs), :1912 (`ByteElement` u8), plus the other sites listed in section 3.
- `SharedMutation` classification vs `is_shared_reference_carrier`/`is_copyable_carrier` (references.rs:3800-3848): both decide whether a type can be shared without mutation (suspected overlap; not compared further).
- Uses `BTreeMap`/`BTreeSet` for dense ids: `owned_structure` (:466, :707-710, :731-747, :1005, :1072), `structural_claims` (:2979, :3047), `projection_storage_dependencies` (:1941).
- Op-semantic tables: `structural_effect_rows!` (:4551) and `proof_bearing_scalar_rows!` (scalars.rs:1337) are two instances of the same macro-driven `operation_semantic_cohort!` pattern (macro in `semantic_rows`, outside unit).

### Unused or one-caller code (grep over `omega-rust/**/*.rs`; counts exclude structural.rs and lib.rs unless stated)
- No use anywhere outside structural.rs/lib.rs (grepped `-w`, listed lines are definitions or lib.rs re-exports only): `fixed_array_place_shape` (:1376), `primitive_read_requirement` (:1474), `constant_runtime_index` (:2638), `runtime_index_bound` (:2654), `interior_member_borrow` (:2774), `structural_effect_semantic_row` (:4627), `byte_equal_equation` (:3111), `byte_predicate_equation` (:3214), `literal_length_equation` (:3159). Inside structural.rs none of these has a caller besides re-exports (`structural_effect_semantic_row` is not called by `structural_effect_leaf_observation_in`, which calls `validate_structural_effect_semantic_rows` and `exact_structural_effect_semantic_row_in` directly, :3464-3466).
- One use outside: `runtime_index_selects` (`terminal-interpreter/src/storage_primitives.rs:4738`), `primitive_place_type` (`pipeline/06.../attached_unit.rs:4258`), `zeroed_structural_fields_satisfy` (`codec/src/foundation_validation.rs:4245`), `contains_owned_boundary` and `selected_owned_children` (`optimization-unit/semantics/src/current_ownership.rs:3126-3128`, an import alias), `primitive_read_field` (`descriptors.rs:1326`), `projection_tip_with_policy` (`content_transformations.rs:2772`).
- Small counts outside (lines): `validate_inline_carriers` 2, `inline_field_presentation` 2, `validate_complete_claim_frontier` 2, `validate_accounted_descendants` 2, `selected_contains_owned_boundary` 1, `validate_owned_claim_frontier` 3, `runtime_structural_copy_type` 2, `StructuralStorageSelection` 1, `byte_carrier_geometry_matches` 1, `element_view_write_goal` 1, `structural_store_value_admitted` 5, `discardable_store_value` 2, `structural_window_origins` 3, `zeroed_structural_leaves` 2, `zeroed_structural_leaves_with_established_erased_fields` 1, `zeroed_structural_leaves_modulo_references` 2.
- Higher use (line counts outside): `qualification_path_carrier` 34, `projected_field_declaration` 26, `structural_window_projection` 24, `canonical_static_projection` 22, `atomic_cell_type` 20, `observed_field_declaration` 20, `primitive_read_projection_type` 20.
- No comment in this file claims a function is the single/shared/canonical route. Names containing "canonical" (`canonical_static_projection`, `canonical_structural_path_tip`, `CanonicalStructuralPathSegment`) refer to declaration-id paths.

### Stated limits
- Error strings only: "complete value requires a closed array extent" (:630), "owned value array extent is not closed" (:947), "owned descendant accounting requires a closed array extent" (:784), "complete value has a cyclic by-value type" (:531), "owned value occurrence is unresolved" (:995), "complete value claim frontier has no owned child geometry" (:666).
- Constants: `MAX_ZEROED_LEAVES = 131_072` (:5325); array enumeration caps via `limit` parameters.
- File header (:1-4) is the only comment text.

### Fail-open risks
- `walk_primitive_projection_with_policy` (:2090-2099): `FixedIndex` on a `FixedArray` whose length is not literal (`ArrayExtent::Application`) is accepted for any index (`length.literal().is_none_or(...)`), and the canonical step is emitted as `None` (:2093-2097). A statically out-of-range constant index into an application-length array passes this walker. Status: open; bounds for those are presumably checked at run time/with a proof elsewhere (not traced).
- `canonical_projection_type` (:2918-2920): same relaxation only when `dependencies.is_some()` (explicit).
- `validate_flow`-style `_ => {}` patterns do not occur here. `shared_mutation` fails toward `Unbounded` for unknown types (:2524-2528).
- `inline_carrier_declaration` returns `None` for fields without `inline_carrier` (:51), i.e. "no inline carrier", and callers using `?` stop. `validate_inline_carriers` (:75): fields with `inline_carrier == None` are skipped (`continue`, :90-92): a field that should have one but does not is not flagged by this function. Status: open (whether `None` is legal is decided elsewhere).
- `structural_store_value_admitted` (:5071): `Vacant` disposition returns `true` before any check; by name and use (a vacant destination holds nothing to drop). Status: traced from name only.
- `zeroed_*` first-case assumption: a sum whose all-zero tag does not select case 0 is accepted as zero-constructible. Status: open (convention is repeated in :5481, :5730, :5782 and storage.rs:3651).
- `zeroed_structural_leaves_impl` hard-codes a signed 32-bit tag (`:5731`): the emitted `SumSelection` leaf carries a scalar type that may differ from the target tag; consumers outside the unit decide. Status: open.
- `partial_affine_residuals` and ownership accounting functions return `None`/`Err` on any anomaly. Fail-closed.
- `scalar_field_entails` use at :114 passes the carrier's `scalar_restriction` as `required`; this file elsewhere only treats `BoundedInteger`/`RefinedScalar` as valid restrictions (:4170-4189), so the atomic gap noted in section 2 is not reached from here by that evidence (traced from this file only).
- Others: none seen.

---
## 5. `semantics/src/substitution.rs` (929 lines)

### Purpose
Capture-avoiding substitution of scalar values and structural places inside propositions, scalar terms and crash-route guards, plus an in-memory index of established facts ("axiom roster") used to avoid pushing duplicate propositions. It rewrites terms; it does not simplify or prove.

### Shape
- `substitute_proposition_values` (substitution.rs:17): replaces `ValueId`s with `ScalarTerm`s through every `Proposition` variant.
- `substitute_proposition_structural_places` (:117): rebases `PlaceId` roots to `(PlaceId, path prefix)` using `BTreeMap<PlaceId,(PlaceId,Vec<CanonicalStructuralPathSegment>)>`.
- Private helpers: `substitute_integer_math_term_values` (:256), `substitute_integer_math_term_places` (:293), `substitute_scalar_term_places` (:309), `substitute_content_term_places` (:365); `pub(crate) substitute_scalar_term_values` (:659, exhaustive over `ScalarTerm` variants, using four local macros `typed_binary_body`, `typed_unary_body`, `shift_body`, `cast_body`).
- Roster: free functions `roster_equation_position` (:393), `roster_has_fact` (:404), `push_unique_fact` (:416); traits `AxiomRoster`/`AxiomRosterMut` (:422, :427) implemented for `Vec<Proposition>`, `[Proposition]`, `SharedRoster`; `SharedRoster{shared: Rc<Vec<Proposition>>, tail}` (:458); `FactIndex{positions: HashMap<u64,Vec<usize>>}` (:517) with `new/equation_position/position_of/has_fact/record/push_unique`; hashing via a private `FactHasher` (:615) and `equation_key` (:599, symmetric XOR of the two side hashes mixed with a constant).
- `substitute_crash_routes` (:894): applies value substitution to every `CrashRouteGuard::Predicate` in each `CrashRouteBucket`.

### Inputs and outputs
Pure functions over `semantic_vocabulary` terms and `terminal_psi::{CrashRouteBucket, CrashRouteGuard, CrashPredicateTerm}` returning new values. `FactIndex`/`SharedRoster` hold state (`HashMap`, `Rc<Vec>`).

### How it decides
- Replacement is keyed on `ValueId` (BTreeMap lookup, :701-704) or `PlaceId` root (:320, :337, :157-159, :189, :201, :210). `MathValue` is replaced only when the replacement's integer type equals the recorded `source_type` (:268); otherwise left as is.
- Symmetric float comparisons and `StructuralIdentity` are re-ordered after substitution (`left > right` swap, :68, :176, :216).
- Fact lookup: equality `Equal(Value, Value)` is matched in either orientation (:399, :544); other propositions by structural equality (`axioms.contains`, :412).
- Crash routes: a guard whose substituted proposition is literally `Truth` becomes `CrashRouteGuard::Truth`, one that is literally `Falsehood` is dropped, a bucket whose alternatives become empty is dropped, and a bucket containing `Truth` collapses to `[Truth]` (:905-926). The substitution does not evaluate propositions, so `Truth`/`Falsehood` only appear if the proposition already was one.
- No keying on names/strings.

### Cases by shape
- `substitute_proposition_values` leaves `Atom`, `IeeeFloatComparison`, `ByteSequenceEqual`, `StructuralIdentity`, `StructuralCaseMembership`, `ContentConservation` unchanged (`.clone()`, :24, :49-59, :78-91, :111-113): these variants have no value substitution at all.
- `substitute_proposition_structural_places` handles all variants; for `ContentConservation` it calls `substitute_content_term_places` (:246-252).
- `substitute_scalar_term_places` (:309): `StructuralRead`, the "predicate quint" pattern (root+path), unary/binary operand patterns, and `Value|Boolean|Integer|IeeeFloatLiteral|Application` as leaves.
- `substitute_scalar_term_values` (:659): explicit arm for every `ScalarTerm` variant; the `scalar_term_predicate_quint_pattern!` terms, literals and `Application` are cloned unchanged (:724-728).

### Same job elsewhere
- Two "place rebase" implementations that prepend a prefix: `substitute_scalar_term_places` (:319-329 and :337-345) versus `substitute_content_term_places` (:370-383), which replaces only the root and discards the prefix (`.map(|(root, _)| *root)`, :379).
- Fact lookup: `roster_has_fact` (:404, linear scan) and `FactIndex::has_fact` (:561, hashed) implement the same decision twice; `push_unique_fact` (:416) and `FactIndex::push_unique` (:584) likewise.
- Crash-route substitution is also performed in `psi/.../06_checked-trees-to-lowered-psi/src/retention.rs:2977`, `:3139` and `omega/.../01_abstract-operations-to-abstract-operations/src/validation.rs:2919` (these call `substitute_crash_routes`; read none of them).
- `SummaryCrashBucket`/`route_substitution` in `pipeline/05_typed-trees-to-checked-trees/src/published_facts_crash_calls.rs` (suspected parallel crash-route substitution on a different type; not read).

### Unused or one-caller code (grep `-w` over `omega-rust/**/*.rs`, excluding this file)
- `roster_equation_position`, `roster_has_fact`, `push_unique_fact`, traits `AxiomRoster`/`AxiomRosterMut`, `SharedRoster`: 0 uses outside this file (command: `grep -rn --include=*.rs "AxiomRoster\|SharedRoster\|push_unique_fact\|roster_has_fact" .` printed nothing; `roster_equation_position` 0 by word count). `FactIndex` (this type): 0 uses outside this file (the only other hits for the word are an unrelated enum variant and error type name in `00_terminal-psi-to-abstract-operations/src/optimization.rs:26,58`). Inside the file, `FactIndex` uses the roster traits and `SharedRoster` only implements them; nothing constructs a `FactIndex` or `SharedRoster`.
- `substitute_proposition_structural_places`: 4 call sites (`terminal-psi/codec/src/foundation_validation.rs:2068`, `descriptors.rs:3547`, `:7040` and the grep also lists the definition). `substitute_crash_routes`: 3 call sites (listed above). `substitute_scalar_term_values`: used in `descriptors.rs:4236`, `:10601` (and imported at `:4049`). `substitute_proposition_values`: 11 lines outside (not split by site).
- `pub mod substitution` is exported from `lib.rs:40`.

### Stated limits
- Header comment says "Capture-avoiding" (:2) but the code has no binder handling because no `Proposition` variant seen here binds variables; no limits stated.

### Fail-open risks
- `substitute_content_term_places` (:365-383): discards the place prefix when rebasing a content projection subject (`|(root, _)| *root`), unlike the scalar/byte/identity paths. A content term whose root is rebased to a sub-path of another place would still refer to the whole destination root. Status: open (unknown whether prefixes are always empty for content subjects; not traced).
- `substitute_crash_routes` (:909-926): dropping `Falsehood` alternatives and empty buckets removes crash routes; reachable only if a predicate literally was `Falsehood` (no simplification happens here). Status: traced within this file.
- `FactIndex::equation_position` etc.: hash collisions are resolved by a final equality check (:543-546, :558), so a collision cannot produce a false positive.
- Others: none seen.

---

## 6. `terminal-psi/src/artifacts.rs` (566 lines)

### Purpose
Pure data records carried next to a `TerminalModule` in Terminal Psi artifacts: the program-entry receipt, the debug map (files, spans, subjects), per-machine provenance for diagnostics, and the proof-kernel node/rule/witness schemas and evidence-route enum. Per the header, validation is done by codecs/verifiers elsewhere (artifacts.rs:1-7).

### Shape
- `checked_program_entry::CheckedProgramEntryTerminalReceipt` (artifacts.rs:16; fields `source_signature_identity: [u8;32]`, `source_machine_name: String`, `declaration`, `terminal_psi_identity`, `terminal_entry: MachineId`), `new` (:25), getters.
- `debug_map` (:62-156): `DebugFileId(NonZeroU32)` (:72), `DebugSourceDigest([u8;32])` (:88), `DebugSourceOrigin::{User,Toolchain}` (:110), `DebugSourceFile{id,origin,byte_len,digest,path: String}` (:116), `DebugSourceSpan` (:125), `DebugSubject` (9 variants, :132), `DebugSite` (:145), `TerminalDebugMap{semantic,files,sites}` (:151).
- `machine_provenance` (:157-321): `TerminalMachineProvenance{machines: BTreeMap<MachineId,(String,Option<SourceSpan>)>, callers: BTreeMap<MachineId,MachineId>}` (:164) with `for_root` (:170), `name` (:224), `record_caller` (:228), `named_ancestor` (:232), `label` (:246), `source_span` (:258); one test (:264-319).
- `proof_kernel` (:323-555): `ProofSystemMarker` (:330; `CURRENT`, `new(raw)` accepts only the value `5`, `get()` returns 5 hard-coded at :344), `AdmissionKind` (:349, 3 variants), `CertificateEnvelope{identity,proof_system_marker,proof}` (:356), `AdmissionEvidence` (:363), `EvidenceRoute::{KernelDerived,CertificateDerived,Admitted,SourceChecked}` (:372), `PrimitiveJudgment` (4 variants, :386), `ProofNode{conclusion,rule}` (:394), `ProofRule` (28 variants, :400-503), witness structs (:509-544). Everything is re-exported at :556-566.

### Inputs and outputs
`for_root` (:170) reads `module.unoptimized_behavior` and `module.provider_candidates` and returns a new provenance with `callers` rebuilt by BFS from `entry`, keeping the existing `machines` names (cloned) and adding the root's name. Other items are plain data (derive `compiler_storage::Record` for the receipt and provenance, :15, :163).

### How it decides
- Provenance attribution is keyed on `MachineId` equality: BFS follows `Invocation` and `NominalCleanup` callees and, for `BoundaryInvocation`, every `provider_candidates` row with the same `boundary` id (:200-216). First caller recorded wins (`entry().or_insert`, :229); first name wins (:225). `named_ancestor` walks the caller chain at most `callers.len()+1` steps (:237).
- `label` formats names into diagnostic text (`format!`, :249-254); no judgment depends on it.
- No other judgments.

### Cases by shape
- `for_root` handles `Invocation`, `NominalCleanup`, `BoundaryInvocation`; `_ => {}` ignores other `UnoptimizedBehaviorOperation` kinds (:217).

### Same job elsewhere
- `TerminalMachineProvenance::for_root` is used by `psi/compilation/psi-compiler/terminal-production/src/lib.rs:670` and `:708` (grepped); the call-graph walk over `unoptimized_behavior` is one of several call-graph walks in the compiler (suspected, e.g. `ModuleActivation::new` suspending fixpoint at storage.rs:585-609; not the same data).
- Uses `BTreeMap` keyed by dense `MachineId` (:165-166).

### Unused or one-caller code
- Occurrence counts outside this file (line matches over `omega-rust`): `TerminalMachineProvenance` 14, `CheckedProgramEntryTerminalReceipt` 26, `TerminalDebugMap` 19, `CertificateEnvelope` 8, `ProofSystemMarker` 17, `EvidenceRoute` 22, `AdmissionEvidence` 7, `DebugSourceFile` 6. `for_root` has 2 call sites (above); `name` and `record_caller` not separately counted (`record_caller` is called inside `for_root`, :196).

### Stated limits
- Header: "pure data schemas; canonical codecs and independent verifiers validate them at artifact boundaries without trusting producer construction" (:5-6). The constructors here do no validation (`CheckedProgramEntryTerminalReceipt::new`, `ProofSystemMarker::new` aside).

### Fail-open risks
- None seen. `ProofSystemMarker::new` rejects any raw value other than 5 (:335-341). `for_root` missing body or machine results in attribution simply not extending (`continue`, :191); no acceptance depends on it.

---

## Coverage

All six files were read in full, in order, in chunks (no range skimmed, none skipped).
- `semantics/src/references.rs` 1-4508 read in full (4508 lines).
- `semantics/src/scalars.rs` 1-5021 read in full (5021 lines).
- `semantics/src/storage.rs` 1-3683 read in full (3683 lines).
- `semantics/src/structural.rs` 1-5804 read in full (5804 lines).
- `semantics/src/substitution.rs` 1-929 read in full (929 lines).
- `src/artifacts.rs` 1-566 read in full (566 lines).
- Total read in full: 20,511 lines. Skimmed: 0. Not read: 0.
- Unit tests (`#[cfg(test)]` blocks inside these files) were read but only summarised, not analysed line by line: references.rs 895-1077, 3194-3658; scalars.rs 2671-3025, 3806-3899; storage.rs 1408-1522, 2432-2608; structural.rs 155-409.
- Cross-file claims came from grep (Grep tool and `grep -rw` summed counts); I read only the lines around the matches listed (e.g. `types_and_values.rs:965-1010` for `StructuralFieldType::scalar_type`), not the other call sites.

## Five things a maintainer most needs to know about this unit

1. These are pure judgment libraries over `StructuralTypeDeclaration` tables; every lookup is a linear scan of the declaration iterator by `StructuralTypeId`, and the "exactly one" rule is enforced inconsistently (`exactly_one` / `find_structural_type` / local `unique` closures / first-match `structural_declaration` at structural.rs:3081). The same path-walk (Field/Case/FixedIndex/Referent over declarations) is written at least seven times with different accepted segment sets (structural.rs:1962, :2394, :2841; scalars.rs:3918; references.rs:2036, :2923; structural.rs:545).
2. Several public items re-exported from `lib.rs` have no user outside their own file: `record_field_dependencies` (references.rs:2513; no re-export, no caller), `elidable_proof_bearing_scalar_leaf` (scalars.rs:845), `fixed_array_place_shape`, `primitive_read_requirement`, `constant_runtime_index`, `runtime_index_bound`, `interior_member_borrow`, `structural_effect_semantic_row`, `byte_equal_equation`, `byte_predicate_equation`, `literal_length_equation` (structural.rs), `collection_view_backing`, `element_view_region_geometry`, `representation_region_length`, `zero_valid_record_storage` (storage.rs), and the whole roster/`FactIndex` machinery in substitution.rs (:393-657).
3. Convention "all-zero value of a sum is its first case, tag type signed 32-bit" is repeated without comment: structural.rs:5481, :5730-5731, :5782 and storage.rs:3651, :3660; the layout code in references.rs gets the real tag size from `TargetCarrierLayout.case_tag_byte_size` (references.rs:2752-2756).
4. Operation semantic tables (`ProofBearingScalarSemanticRow::ALL`, scalars.rs:1337-1383; `StructuralEffectSemanticRow::ALL`, structural.rs:4551-4625) are revalidated in full on every row lookup (`proof_bearing_scalar_semantic_row` :1423, `structural_effect_semantic_row` :4630, and `structural_effect_leaf_observation_in` :3463-3465 when the tag is in the cohort).
5. Fail-closed is the dominant style (unknown returns `None`/`Err`/`false`), with a few relaxations I could not close from this unit alone: `scalar_field_entails` returns true when the required field type has no `scalar_field_requirement` (scalars.rs:3159-3161, includes `AtomicBoundedInteger`), `reference_source_projection_accessible` skips ceilings for untyped prefixes and unhandled segments (references.rs:3984-4023), the activation flow ignores unlisted operation kinds (storage.rs:1213), FixedIndex into application-length arrays is accepted without bound (structural.rs:2092), and content-term place rebasing drops the path prefix (substitution.rs:379).

## What I am unsure about

- Whether `scalar_field_entails` can actually be called with an `AtomicBoundedInteger` as `required`: of the callers (structural.rs:114, descriptors.rs:1154/1286/1336, emission.rs:279/4108) only structural.rs:114 was reasoned about, and only from the shape of `scalar_restriction` handling at structural.rs:4170-4189.
- Whether the `references.rs` accessibility functions' skipped checks are reachable: I did not read the callers (22 and 15 word-matches).
- The `_ => {}` arm in `validate_flow` (storage.rs:1213) and the `zip`-truncation at :1304/:1324/:1371: whether earlier validation guarantees equal argument/parameter counts and that unlisted operation kinds cannot carry claims.
- Whether `FixedIndex` bounds into `ArrayExtent::Application` arrays are proven elsewhere (structural.rs:2092).
- Zero-caller claims are for the Rust tree under `omega-rust` only; the Omega-written compiler (`source/psi`, `source/omega`) and `bootstrap/` were not searched, so items listed as having no Rust caller might be mirrored there.
- Exact external call-site counts for items given as "line counts outside" include definition/re-export lines in other files and word-matches of identically named items; they are not call-site counts.
- I did not read `lib.rs`, `semantic_rows`, `descriptors.rs`, `extents`, or `control_graph`, which several functions here depend on (`extents::compose_claim_bounds`, `crate::control_graph::dominators`, `operation_semantic_cohort!`).

