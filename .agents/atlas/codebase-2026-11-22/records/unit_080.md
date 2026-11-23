# Record: unit_080 (area 14-interpreter)

Files in this unit (all in crate `terminal-interpreter`, `source-rust/subsystems/interpreter/memory/`, declared as `storage::*` modules by `#[path]` from `interpreter.rs`, inferred from `use crate::storage::...` paths):

1. `primitives.rs` (7003 lines)
2. `providers.rs` (4471 lines)
3. `records_and_arrays.rs` (6276 lines)

All three are read in full. Line numbers are file-local. Counts quoted come from Grep count/content runs over `source-rust/` (command named at each use). The Grep tool was used; no program was built or run.

---

## 1. `memory/primitives.rs`

### Purpose
Runtime homes for scalar values and the services around them: activation-claim ledgers (the `EstablishActivationClaim`/`Release`/`Split`/`Merge` operations), the read/write path for primitive scalars (including re-viewed, byte-region, array-element and field endpoints), the byte codec between scalar values and stored bytes, storage-address and alignment queries, the paid-row table `RuntimeRows`, scalar cells with optional host backing, and a counter that mints fresh structural identities (`primitives.rs:1-9`, header comment, read).

### Shape (modules in file order)
- `activation_storage` (11-787).
  - `ledger::RuntimeActivationLedger` (33): wraps `extents::ActivationClaimLedger` plus an optional storage lease. `reserve_claim` (48), `retire_claims` (61), `establish` (73), `release` (83), `split` (92), `merge` (108).
  - `RuntimeActivationStorage` (122): five `RuntimeRows`: `ledgers` keyed by `Frame = (call-stack depth, MachineId)` (121), `grants`, `owners`, `claim_descriptors`, `split_children`. `can_continue_entry` (141) is true only when all five are empty.
  - `TerminalExecution` methods: `execute_establish_activation_claim` (187), `is_live_activation_descriptor` (329), `execute_release_activation_claim` (351), `complete_activation_storage` (485), `execute_split_activation_claim` (496), `execute_merge_activation_claim` (587), private `prepare_activation_division` (681) / `publish_activation_division` (740), with `PreparedActivationDivision` (177) as the staged result.
- `primitive_storage` (789-4790), the bulk.
  - `aggregate_projection` (812): `aggregate_projection_storage` (841), `read_storage_projection` (934), `PrimitiveStorageWrites::pending_storage_value` (955) and `stage_projection` (999). Re-views a scalar window that spans several stored fragments as one value via bit masks (`mask` 817, `projection_bits` 824).
  - `encoding` (1047): codec `encode` (1785), `decode` (1820), `convert_scalar` (1732), `boolean_bits` (1865), `scalar_fits_storage` (1092), `shifted_byte_endpoint` (1638), plus the byte-region projection helpers `reference_byte_region_access` (1098), `captured_byte_region_access` (1117), `project_byte_regions` (1162), `apply_reference_storage_view` (1232), `position_reference_storage` (1268), `read_encoded_primitive` (1512), `prepare_encoded_primitive` (1561), `endpoint_scalar` (1613). Three unit tests (1875-1983).
  - Types: `PrimitiveStorageEndpoint` (2053) with seven variants (`RepresentationByte`, `Projection{fragments,logical,stored}`, `Scalar(place)`, `ScalarField(field)`, `ArrayElement{array,index}`, `OwnedArrayElement{home,index}`, `ViewByte{bytes,index}`); `PrimitiveStorage {endpoint, encoding: Option<PrimitiveStorageEncoding{source,view}>}` (2132, 2166); `PrimitiveStorageFragment` (2090); `PrimitiveStorageHome {depth, machine, place, structural_type}` (2019); `PrimitiveLoan` (2346); `PrimitiveStorageWrites {first_write, remaining_writes}` (2214) with `stage` (2220) and `commit` (2334); `ScalarStorageObservation` (2011).
  - `TerminalExecution` methods: `array_cell` (2377), `record_original_scalar_write` (2436), `capture_primitive_loan` (2514), `byte_element_storage` (2720), `bound_byte_element_storage` (2751), `primitive_access` (2842), `primitive_replacement_access` (2858), `reference_primitive_access` (2875), `aggregate_view_primitive_access` (3000), `original_view_primitive_access` (3077), `capture_primitive_home` (3101), `primitive_home_contract` (3118), `element_view_scalar_read` (3239), `primitive_home_access(_within)` (3360/3369), `primitive_view_access` (3595), `boundary_scalar_storage` (3610), `resolve_scalar_view_storage` (3628), `atomic_cell_storage` (3832), `scalar_leaf_field` (3899), `execute_primitive_establishment` (3995), `scalar_field_at_carrier` (4041), `read_primitive_place` (4258), `read_primitive_storage` (4338), `execute_primitive_store` (4462), `store_primitive_storage` (4606), `commit_primitive_store` (4618), `retire_plain_locals` (4665), `retire_reentered_results` (4685), `owned_array_home` (4742). Free fn `scalar_reference_observation` (4765), `scalar_field_projection` (1996), `original_scalar_storage_endpoint` (2140).
- `runtime_indexes` (4792-4885): `select_runtime_elements` (4803) and `select_runtime_projection` (4824) turn a runtime index operand into a `FixedIndex` path segment after a bounds check.
- `runtime_rows` (4887-5290): `RuntimeRows<Key, Value>` (4910) = `BTreeMap` plus an optional `EvaluationStorageLease` and the account. Methods `insert` (4970), `insert_vacant` (4985), `remove` (5003), `retain` (5014), `take` (5019), `validate_append` (5024), `append` (5041), `try_clone` (5058), `relocate_keys` (4927), `from_rows` (4959). Six unit tests (5148-5288).
- `scalar_storage` (5292-5894): `ScalarCell {Local(value) | Resident{scalar_type, backing: Rc<dyn TerminalScalarBacking>}}` (5482), public trait `TerminalScalarBacking` (5476), `ScalarStorageRows<Key>` (5591, a newtype over `RuntimeRows<Key, ScalarCell>`), `ResidentReplacementWrites` (5729), and `component_loans::ScalarLoanScope` (5315) whose `lend` (5322) makes a `ComponentScalarView` that dies when the scope drops (weak `Rc`).
- `storage_addresses` (5896-6893): `encode` (5949, 2-bit-prefixed variable-length integer packing), `path_coordinates` (5984), `ScalarStorageAddress{namespace,root,path}` (5943), `ExposedStorageSelector` (5917), methods `scalar_storage_address` (5998), `storage_address_endpoint` (6027), `array_storage_identity(_staged)` (6096/6107), `endpoint_identity` (6129), `location_alignment` (6226), `execute_storage_alignment` (6349), `execute_storage_address` (6384). Eight unit tests (6443-6892).
- `structural_identities` (6895-7003): `LocalStructuralIdentities {reserved: BTreeSet<u64>, next: Option<u64>, exposure_namespace}` (6904), `allocate` (6943), `allocate_staged` (6958), `reserve_host_at_cursor` (6975), `new_exposure_namespace` (6996, a process-global `AtomicU64`).

### Inputs and outputs
- Reads: the running `TerminalExecution` (`self.structural_values`, `scalar_array_values`, `structural_primitive_storage`, `structural_scalar_fields`, `structural_byte_storage`, `reference_referents`, `element_view_values`, `call_stack`, `live_claims`, `live_affine_frontier`, `program.catalogs`), the program's machine declarations (`self.program.machine(..)`, `structural_parameter`, `producing_operation`), and type geometry from `terminal_semantics` (`reference_storage_*`, `canonical_primitive_path`, `primitive_store_restriction`, `runtime_structural_field`, `scalar_array_leaf_shape`, ...).
- Mutates: those same tables, the paid row tables (cell counts reserved on an `EvaluationInvocationAccount`), `values` (scalar results), `exposed_address_selectors` (6424), the identity counter.
- Produces: `TerminalScalarValue`, `PrimitiveStorage` (an endpoint plus an optional view codec), `PrimitiveLoan`, address integers.

### How it decides
- Activation claims. Frame identity is `(call_stack.len(), current_machine)` (213, 374, 486, 537). A second establish of the same operation in the same frame is refused (214-221, keyed on frame and operation id). Success of `establish` is `committed <= bound` (222). Release must match the exact claim id from `projected_ownership_claims` (383-387) and the byte-descriptor home, and refuses if any other binding in this frame or in the call stack still targets that backing (399-461). Split/merge: if the ledger refuses the split or merge, the result is the `rejected_case` branch, not an error (`split` returns `.ok()` at 100-105; `merge` at 113; used at 565-584 and 652-678).
- Endpoint choice for a place: `primitive_home_access_within` (3369-3593) tries, in this order, (1) byte element of a byte-sequence descriptor (3410), (2) a path containing `Referent` (3431), (3) element-view loan (3441), (4) a `primitive_loan_values` row (3452), (5) `aggregate_argument_backings` (3472), (6) a byte-sequence binding (3491), (7) packed scalar array (3532), (8) ordinary structural view (3588). Each is keyed on identity of the place (a `PlaceId` row in a per-frame table), not on a display name.
- `primitive_home_contract` (3118-3237) accepts a place as a primitive home by looking it up in the machine: a local from `EstablishPrimitiveLocal` (`local_type`, 2359-2374), a `ByteSequenceLiteral` place (read only, 3167), a structural parameter with an access pattern (3176-3211), or an operation result (3212-3231). For writes it also calls `primitive_store_restriction` (3149) to check the declared domain memberships still hold. Keyed on place and structural type ids.
- `execute_primitive_store` (4462) looks up the destination's declared qualifications from the parameter or result declaration, then requires `primitive_store_restriction` and `primitive_store_byte_predicates` to be `Some`, then checks the scalar satisfies the restriction (4532-4537).
- `encoding::encode/decode` (1785/1820): value goes through `convert_scalar` to the stored scalar type, then is masked to the stored width (1812-1817). `decode` re-checks `terminal_scalar_belongs_to_type` (1860). Booleans accept only 0/1 (1868-1872).
- `convert_scalar` (1732-1783): same type passes; a "total reference view" pair goes through `scalar_reference_observation`; integer-to-bool uses `ScalarRepresentationViewPair::classify`; otherwise integer to non-address integer with `admits(value)`; everything else is `malformed`.
- Storage address: address integer = `encode(namespace, root, path)` (6408), where path segments are `(kind, coordinate)` pairs: 0 fixed index, 1 field name, 2 case, 3/4 field id (member / case payload) (5975-5995, 6412-6422). Selectors seen are recorded sorted in `exposed_address_selectors` (6424-6426) for later continuation checks (`ExposedStorageSelector::unchanged`, 5924).
- Identity allocation: `allocate_staged` skips every id in `reserved` (6962-6968), keyed on integer identity.
- Frame-vs-current-frame selection repeats a source-position rule: `home.depth == self.call_stack.len()` means "current frame, else `call_stack[depth]`" (7 sites in this file, from `Grep home\.depth == self\.call_stack\.len\(\)` count 7).

### Cases by shape
- `StructuralTypeShape` handling in `resolve_scalar_view_storage` (3658-3762): only `(Case, Sum|Mixed)`, `(Field, _)`, `(FixedIndex, FixedArray{Literal})` segments are walked; anything else is `malformed` (3760).
- `capture_primitive_loan` (2514): the parameter's shape must be `PrimitiveScalar` (2520); access must be SharedBorrow, MutableBorrow or WriteOnlyBorrow and multiplicity `Unrestricted` (2526-2536); then a fixed chain of five source shapes: re-lent loan, retained reference whose last segment is `Referent`, byte element, scalar array, otherwise projected primitive / field / view (2537-2707).
- `endpoint_identity` (6129): `RepresentationByte` unsupported (6134), `Projection` only with exactly one fragment (`let [first] = fragments.as_slice() else`, 6142) and a contiguous original endpoint.
- `location_alignment` (6226-6346): handles `Scalar`, `ScalarField`, `OwnedArrayElement`, `ArrayElement` (Runtime or Frame home), in-range `ViewByte`/`RepresentationByte` (alignment 1), and one-fragment `Projection`; the rest return the "needs the original selected storage geometry" error.
- `path_coordinates` (5984-5995): only `FixedIndex`, `Field`, `Case` path segments; other segments are `malformed`.
- Byte-region shift (`shifted_byte_endpoint`, 1638-1722): per endpoint variant; `Projection` must cover exactly 8 bits after slicing (1672-1679).
- `ScalarStorageObservation` has four fixed kinds (2011-2016); `boundary_scalar_storage` accepts only Owned, MutableBorrow, SharedBorrow (3617-3624).
- Byte arrays: bytes are always `u8` (`byte_type()` 1728, and `IntegerType::new(Unsigned, 8)` spelled out again at 2592, 2989, 3426, 3497, 3781, 4384, 4560; `Grep` count of that call over `memory/` is 15: 8 here, 6 in `bytes_and_views.rs`, 1 in `records_and_arrays.rs`).

### Same job elsewhere
- Row reservation: `RuntimeActivationLedger::reserve_claim` (48-59) and free `reserve_row` (4893-4907) are the same "grow lease by one cell, or create the lease from the account" logic. Also inline in `providers.rs:445-450`, `providers.rs:769-775`, `providers.rs:2079-2084` (`account.reserve_storage(1, 0)`), read.
- Encode/decode of a scalar to bytes: `encoding::encode/decode` here (1785/1820) is the single codec; `providers.rs:3399-3414` (`decoding::scalar`) and `providers.rs:3847-3867` (`ValueImageReader::scalar`) wrap it with little/big-endian byte slicing, and `providers.rs:1577-1627` does a third inline version for field accessors.
- Packed-home retirement (`retire_packed_home`) is called from `retire_reentered_results` (4715) and `custody.rs:803` (grepped, 1 external call site plus a `BTreeSet` impl at `records_and_arrays.rs:2881`).
- `RuntimeRows` pattern (a `BTreeMap` with lease accounting) is the one table type used across this crate: `Grep RuntimeRows` over `source-rust/` gives 254 occurrences in 14 files, all inside `subsystems/interpreter/` (no hits outside the engine crate, from the file listing).

### Unused or one-caller code
Greps run over `source-rust/` (Grep tool).
- `execute_storage_alignment` / `execute_storage_address`: 1 call site each, both in `instructions/execution.rs:3326, 3330`.
- `reclaim_orphan_scalar_records` (defined in `records_and_arrays.rs:6144`): 1 call site, `instructions/execution.rs:3257`.
- `primitive_replacement_access`: 1 caller, `records_and_arrays.rs:990` (plus a comment at `primitives.rs:3132`).
- `new_exposure_namespace`: 2 callers (`primitives.rs:6922`, `bytes_and_views.rs:3066`).
- `RuntimeRows::try_clone` (5058): code hits are a unit test (`primitives.rs:5228`) and `instructions/terminators.rs:207` (a `.try_clone(` there, not confirmed to be this type). Treat as suspected: 1 non-test caller.
- `ScalarCell::is_local_literal` / `ScalarStorageRows::has_only_literal_cells` (5512/5619): used at `embedding/continuation.rs:139-147` only (3 hits).
- `retains_entry_array_element` (4729): 1 non-test caller, `embedding/continuation.rs:499`.
- `ExposedStorageSelector`/`exposed_address_selectors`: 13 occurrences in 4 files (`continuation.rs` 1, `storage_observation/tests.rs` 2, `execution.rs` 3, `primitives.rs` 7).
- `RuntimeActivationStorage::can_continue_entry` (141): 1 call site, `embedding/program.rs:2366`.
- `complete_activation_storage` (485): 1 call site, `instructions/terminators.rs:1633`. The four `execute_*_activation_claim` fns: 1 dispatch site each, `instructions/execution.rs:3485-3488`.
- `execute_primitive_establishment`, `_read`, `_store`: 1 dispatch site each (`execution.rs:3353, 3357, 3459`).
- `RuntimeRows` is described as the shared paid-row table; 254 uses (see above). `ScalarStorageRows`: 25 occurrences in 5 files (Grep count).

### Stated limits (quotes under 20 words)
- "original byte-view scalar requires its captured array window" (2148).
- "resident replacement needs a codec for its changed active layout" (5691) and "...for its changed scalar declaration" (5712).
- "representation byte address requires its original scalar byte placement" (6135).
- "composed scalar observations have no single storage-address endpoint" (6144).
- "storage-address exposure requires a contiguous original scalar endpoint" (6149).
- "aggregate scalar window contains padding or noncontiguous storage" (868).
- "aggregate re-view has no unique contiguous scalar endpoint" (3019).
- "captured byte-region projection has no contiguous scalar geometry" (1076); "captured byte region has no contiguous geometry" (1200).
- "location alignment needs the original selected storage geometry" (6234).
- "an aggregate re-view window has no canonical storage address endpoint" (6045).
- "opaque component loan lacks original guest storage for mutation" lives in `records_and_arrays.rs:6077`, not here.

### Fail-open risks
- `RuntimeActivationLedger::split` (100-105) and `merge` (113): any ledger error is turned into `None`. The caller then takes the `rejected_case` branch (576-584, 676-678). A program is not accepted wrongly (a rejected branch is the conservative one), but an internal ledger error (as opposed to "range does not fit") is indistinguishable from a normal "no". Classification: open (no comment says why).
- `ScalarStorageRows::append` (5669-5677) calls `prepare_resident_replacement(self, None)`; with `root == None` the check at 5688-5696 that every resident key in the original has a replacement is skipped, and only incoming keys that match a resident are written (5698-5719). Incoming sets that omit a resident key are accepted in an append. Classification: open; the replacement path with `Some(root)` (`records_and_arrays.rs:2156-2163`) is the checked one.
- `PrimitiveStorage::has_literal_scalar_encoding` (2183): `is_none_or` returns true when there is no encoding. Intent: no codec means no callable-typed codec. Classification: stated by name only, traced as harmless (a false only matters when a `Callable` carrier is present).
- `encoding::encode` (1785): `Callable` carriers are encoded as the descriptor id (1792); `HostAddress` and `UndeterminedNaN` are refused (1794-1799), comment "Byte storage would drop the address's host provenance." Fail-closed.
- `array_cell` returns `Ok(None)` for endpoints that are not array-backed (2425); its caller `record_original_scalar_write` turns that into `malformed` (2507). Traced, closed.
- `execute_primitive_store` (4519-4531): the ASCII-backing check runs only when predicates include `ValidUtf8` and not `AsciiOnly`. Reading only; if the ASCII predicate is missing the check is skipped by design. Classification: open (see "unsure" about `path.len() - 1`).
- Otherwise none seen: the many `ok_or(malformed())` sites refuse.

---

## 2. `memory/providers.rs`

### Purpose
Provider-backed storage: byte allocations that the host (or the interpreter) supplies for a granted address-range extent, the descriptors that bind a structural place to a window of one, the "resident" value custody that lives in such a window, and the access-lifecycle operations that move values in and out of it (initialize, view, move out, relocate, empty-run release), plus the encoder/decoder that turns a record value into the byte image the layout plan describes and back (`providers.rs:1-8`, read).

### Shape (modules in file order)
- `backing` (20-190): `ProviderBytes {Owned(Vec<u8>) | Host(TerminalProviderBacking)}` (26), read/write guard enums (31, 36), public `TerminalProviderBacking{bytes: Arc<Mutex<ProviderBackingState{bytes, retired}>>}` (94) with `new`, `byte_count`, `retire` (118), `lock` (132), `same_allocation` (143). Two unit tests.
- `entry` (193-636): public `TerminalProviderStorageInput` (210), `...Rejection` (219), `TerminalReturnedProviderStorage` (225). `take_returned_provider_storage` (232) hands storage back from a normal structural return. `is_live_provider_descriptor` (338), `is_live_provider_owner` (349), `provider_storage_geometry(_matches)` (404/414). `bind_provider_storage` (436) and private `provider_storage_entry_home` (488). `returned_storage_claim` (618).
- `supply` (637-801): public `TerminalProviderStorageRequest` (650), `TerminalProviderStorageSupply` (672), trait `TerminalProviderStorageResolver` (678); `bind_dynamic_provider_storage` (686) binds a late-supplied extent to a place.
- `empty_run` (805-971): `release_empty_run` (819), `empty_run_zero_field` (952).
- `extraction` (972-1297): `extract_access_resident` (989).
- `field_access` (1298-1678): `execute_provider_field` (1325) reads or writes one scalar field in a window; `field_window` (1650); `ProviderFieldContent {Resident | Borrowed}` (1319).
- `initialization` (1679-1843): `InitializationRejection` (1698), `PreparedProviderImage` (1704), `RuntimeProviderStorage::prepare_initial_image` (1711), `stage_rejection_failure` (1795).
- `lifecycle` (1844-2325): `complete_access_initialization` (1866), `reject_access_transfer` (2125).
- `profile_values` (2326-2638): `read` (2349) reads a `ResourceProfile` out of structural storage; `ProfileValue` cursor type (2492).
- `profiles` (2639-2887): `lifecycle_profile` (2665), `capture_resource_profile_call` (2786), `complete_resource_profile_call` (2859), `PendingResourceProfile`/`CompletedResourceProfile` (2649/2657).
- `relocation` (2888-3300): `relocate_access_run` (2909).
- `value_image` (3301-4263): `decoding::restore` (3333), `geometry::ImageGeometry` (3616), `prepare` (3774), `ValueImageReader` (3837), `field_schema` (4054), `record_layout` (4098), `require_runtime_fields` (4216), `zeroed` (4237), `materialize` (4245).
- `view` (4264-4321): `RuntimeProviderStorage::prepare_view` (4273).
- Root items (4322-4471): `ProviderStorageBinding {allocation, offset, length, resident}` (4327), `ResidentStorageSlot` (4335), `ProviderStorageSlot` (4348), `RuntimeProviderStorage {allocations: arena::Arena, residents: arena::Arena, descriptors, completed_profiles}` (4358), `binding_is_live` (4379), `live_provider_allocation` (4408), `live_resident_custody` (4431), `ProviderByteAllocation` (4462).

### Inputs and outputs
- Reads: `TerminalExecution` tables (`provider_storage`, `structural_values`, `structural_cases`, `structural_scalar_fields`, `live_claims`, `live_affine_frontier`), `program.access_lifecycle_plans()` (the compiler-published plans: `plan.storage`, `plan.placement`, `plan.schema`, `plan.image_fields`, `plan.target`), the call's `BoundaryMachineDeclaration`, resource-profile validators from `access_admission`, and `storage_materialization` for field byte staging/decoding (`providers.rs:1594, 1612, 3378, 3531, 4253`).
- Mutates: `provider_storage.{allocations,residents,descriptors,completed_profiles}`, `structural_values` (new result place), the structural frontier and claims through `prepare_constructor_operands`/`commit`, identity cursor.
- Produces: effect results (`TerminalEffectResult::Unit | Scalar`), `TerminalReturnedProviderStorage`, bytes in the host backing.

### How it decides
- Window liveness: `binding_is_live` (4379-4406) keys on arena handle, `offset+length <= extent.length()`, and, when `resident` is set, that the resident slot's `window` equals the binding with `resident` cleared (4396-4402). Identity-and-range test.
- Which lifecycle plan applies: `find` over `access_lifecycle_plans()` by structural carrier type id and `Granted`/`Vacant` domain ids in the value's qualifications (260-267), or by `plan.boundary` id (4455).
- Entry-time admission (`provider_storage_entry_home`, 488-615): entry storage open (492), extent has a provider issuance (495), `extent.length() == bytes.len()` (500), argument is an Owned entry parameter (512), the path's case is active (524-533), the place has no descriptor yet (541), all retained extent roles agree on base/length/granted/vacant (544-560), declared premises are in the value (561-574), an exact live claim exists (575-581), geometry fields read back equal the extent (582-592), `profile.restrict_to_loan` succeeds (597-600), and no live allocation overlaps in the same origin/address-space/era (601-613).
- Extent and profile equality, not names, decide that two supplies are the same allocation: `backing.same_allocation` (`Arc::ptr_eq`, 144) plus equal `extent`, `profile.receipt()` and `profile.profile()` (733-744).
- Run relocation chooses rejection over error when source and destination windows overlap in one allocation (3017-3027) or when the selected or admitted supply fails `validate_run_relocation_resources` (3073-3080).
- Field access (`execute_provider_field`): an `AccessOperation` is chosen from `TerminalAccessorKind` (1347-1355; `Take` unsupported), the receiver borrow polarity from `StructuralAccess` (1356-1360), then `authorize` or `authorize_region_projection` on the access plan entry (1377-1396); supply row matched by `entry.key()` (1432-1449).
- Layout entries are matched by source spelling: `layout.entries.iter().find(|layout| layout.field == entry.field())` (3363-3369) and `filter(|entry| entry.field == field)` (1657); record fields get invented layout names with `format!("field:{:?}", field.id)` (3526, 3949, 4116). The name is a debug-format of a field id, used as the key between the layout plan and the runtime.
- Case tag on decode: `case_position = u64::from_le_bytes(tag)` indexes into `cases` (3567-3573); out of range is an error "resident image contains an invalid case tag".

### Cases by shape
- Boundary call argument lists must have exact arity: empty-run release `[source]` (827); initialization `[extent, original, _profile]` (1880); extraction `[source, profile_argument]` (1002); relocation `[source, destination, source_profile, destination_profile]` (2918); rejection `[extent, original, ..]` (2135). Result contracts: one `ready` field (1912, 3123), `[extent_field, value_field]` (1035), a single result claim `[binding]` for empty-run (912).
- `value_image::decoding::structure` (3452-3554) and `ValueImageReader::structure` (3894-3970) handle exactly `PrimitiveScalar`, `FixedArray{Literal}`, `Record`, `Sum`, `Mixed`; other shapes: "resident image is outside the retained schema reflection contract" (3546-3550) / "owned image shape is outside..." (3964). Recursion depth capped at 128 (3457, 3898).
- `record_layout` (4098-4214) matches `(StructuralRecordStorage, field.storage)` into `Packed`, `Authored+Fragmented`, `Authored+Repeated`, `Authored+Scalar{stored_scalar: Some(Integer)}`, `Authored+other placement`, and refuses the rest with "owned image record has no resolved storage recipe" (4206-4210).
- `profile_values::read` (2349-2388) reads a fixed schema: record of 2, up to 32 region entries (`counted_entries(32)`, 2359), each of 6 fields; peer 2 cases, stable 4 cases, external 2 cases with up to 16 transfers, atomic 2 cases with 12 permission booleans (2517-2605, `count` arguments). Capacity is compared to the declared array length (2617).
- `TerminalAccessOwner::{Extent, Run{..}}` and `TerminalAccessLifecycleOperation::{Initialize, MoveOut, View, Relocate}` are matched exhaustively in `lifecycle_profile` (2693-2759).
- `field_access` scalar width: output buffer `[0_u8; 16]` (1610) so scalar widths up to 16 bytes; write bits are narrowed to `u64` with an error comment "resident scalar codec exceeds its 64-bit materialization carrier" (1579-1583).
- `ProviderFieldContent::Resident` accepts only `ObservationModel::Stable` descriptors (1398-1404).

### Same job elsewhere
- The five lifecycle operations repeat one skeleton: stage owned operands with `stage_owned_value`, compare four result-contract fields (`structural_type`, `multiplicity`, `qualifications`, `projected_qualifications`) to the contract (1026-1030, 1903-1906, 2173-2177, 3114-3118, 842-845), loop result claims through `constructor_claim_is_active` and `bind_result_claim` (1241-1249, 2061-2070, 2300-2311, 3217-3225), then `stage_result_affine_frontier`, `prepare_constructor_operands`, install and `commit_cursor`. Five copies in this file; `records_and_arrays.rs:2537-2665` (`establish_record`) is a sixth cousin.
- Window arithmetic ("offset+length overflow check, convert to usize, `backing.get(start..end)`") repeats at 1164-1174, 1539-1550, 2089-2106, 3081-3092, 3261-3270.
- `bind_provider_storage` (436-486) and `bind_dynamic_provider_storage` (686-799) both create an allocation row with a funded lease and an `insert_vacant` descriptor; the dynamic path also rechecks overlap against existing allocations (723-754).
- Image encode/decode mirror pair: `value_image::prepare` (3774) with `ValueImageReader`, and `restore` (3333) with `ValueImageDecoder`. The decoder is a separate walker over the same shapes.
- `Region`-style overlap tests: `provider_storage_entry_home` overlap loop (601-613) and `bind_dynamic_provider_storage` overlap loop (745-753) are nearly the same predicate.

### Unused or one-caller code
Greps over `source-rust/` (Grep tool).
- `release_empty_run`, `extract_access_resident`, `relocate_access_run`, `complete_access_initialization`, `reject_access_transfer`: 1 call site each in `instructions/calls.rs` (3130, 3194, 3200, 3210, 3214) (grep with `!**/providers.rs`; `reject_access_transfer` is called twice, 3194 and one in `relocation` module inside providers).
- `TerminalProviderBacking`: 17 occurrences in 4 files (host `allocation.rs` 2, `execution.rs` 4, `interpreter.rs` 1, `providers.rs` 10). `byte_count` is called at `host/.../allocation/backing.rs:59, 131, 132` (3 external uses).
- Provider supply types (`TerminalProviderStorageResolver`, `...Supply`, `resolve_provider_storage`): 28 occurrences in 7 files.
- `take_returned_provider_storage`, `bind_provider_storage`, `TerminalProviderStorageInput`: 13 occurrences in 3 files (`program.rs` 5, `interpreter.rs` 1, `providers.rs` 7), so the public pieces are re-exported by `interpreter.rs`.
- `ImageGeometry::view_source_compatible` (3732): grep of `view_source_compatible` shows the call at `references.rs:2557` and several other `view_source_compatible` calls on `ReferenceStorageGeometry` in `terminal_semantics`/native; this wrapper has 1 caller in this crate, read.
- `RuntimeRows::insert_vacant`: 5 uses in this unit's providers module (grep count 6 in providers.rs with other patterns).

### Stated limits (quotes under 20 words)
- "resident take requires partial custody transfer" (1351-1353).
- "resident scalar events require Stable observation" (1401-1403).
- "resident field write requires preservation of retained qualifications and bindings" (1501).
- "external field write cannot synthesize an unadmitted read-patch-write" (1535-1537).
- "resident custody requires live forwarding of carried references" (1933-1935).
- "resident zero-runtime elements require authored custody" (3483-3485).
- "placement with non-runtime fields requires an authored custody agreement" (4229-4231).
- Comment at 3689-3691: closed target-independent geometry stays usable "even if the admitted profile cannot represent a descriptor".
- Comment at 6926 style: none here. Comment at 4229: see above.

### Fail-open risks
- `ImageGeometry::for_execution` (3643-3664) turns a carrier-layout conversion failure into a stored `Err` that only surfaces when a query needs carriers (3688-3698). Stated at the site (3689-3691: "Closed, target-independent geometry remains usable..."). Traced: queries that need carriers return `ValueImageGeometry(err)`; ones that do not are fine.
- `ImageGeometry::region`, `location_alignment_limit`, `view_source_compatible` return `Option`/`bool` from geometry without the carrier error (3732-3763); a missing carrier layout yields `None`/`false` (refuse), not accept. `view_source_compatible` returning `true` depends on `terminal_semantics` (not read).
- `prepare_view` (4273-4319): both the supplied profile and the admitted profile must validate, and both must admit the base alignment; `actual.ok_or(malformed())` (4318). Closed.
- `profile_values::read`'s `counted_entries` requires `count <= capacity` and `length == capacity` (2617) and fails on mismatch. Closed.
- `execute_provider_field`: when `descriptor.observation() != Stable` and a write needs a read-patch-write, refused (1534-1538). When observation is non-Stable and the access is read-only, the stable-supply check is skipped (1523-1538), since a non-Stable operation does not need stable supply. Stated by the code structure; no comment.
- None seen that accept an unmatched or missing row; all `None` lookups go through `ok_or(...)`/`let ... else { return Err }`.

---

## 3. `memory/records_and_arrays.rs`

### Purpose
Building records and arrays from existing values under paid row reservations (`ConstructorStorage` stages moved or copied storage, then publishes it atomically), claim and affine-frontier bookkeeping for those constructions, packed scalar arrays (`PackedScalarArrayValue`) with copy-on-write cells and optional host-resident cells, empty-array tracking, boundary capture of storage contents, retirement and garbage collection of retired homes, and the key-ordered "subtree" helpers over `BTreeMap` tables (`records_and_arrays.rs:1-8`, read).

### Shape (modules in file order)
- `record` (10-2733) with sub-modules:
  - `byte_fields::prepare_constructor_byte_field` (38): decides whether a record's byte-sequence field accepts an operand and returns its `ByteSequenceBinding`.
  - `operands` (165): `ResultClaimOrigin {Construction, Transfer}` (182), `PreparedConstructorOperands` (187) with `commit` (194), `bind_constructor_claims` (210), methods `validate_transferred_affine_complement` (264), `stage_constructor_operand` (371), `stage_constructor_claims` (424), `constructor_claim_is_active` (503), `stage_result_affine_frontier` (527), `commit_constructor_operands` (554), `prepare_constructor_operands` (565). Three unit tests (593-747).
  - `owned_value` (749): `StagedOwnedValue` (761), `stage_owned_value` (774).
  - `storage` (879): `ConstructorValue {Access | PackedScalars}` (895) with `resolve` (906), `replacement_writes` (964), `visit_scalar_leaves` (1004), `memberships` (1074); `ConstructorStorage` (1105), a bag of 15 staged tables (original_roots, completed_profiles, provider_descriptors, activation_descriptors, host_atoms, scalars, bytes, views, byte_descriptors, references, primitives, byte_arrays, empty_arrays, owner_writes, cases); methods `stage` (1623), `stage_array` (1656), `stage_access` (1772), `stage_subtree` (1898), `stage_resident_image` (1299), `prepare_replacement` (2111), `publish` (2186), `validate_publication` (2200), `publish_cells` (2251); `DetachedConstructorStorage` (1153), `PreparedConstructorReplacement` (1179), `ConstructorResidentWrites` (1155).
  - `structural_array` (2302): `establish_immutable_elements` (2342), `execute_establish_structural_array` (2447).
  - Root of `record`: `establish_record` (2537), `stage_owned_value_arguments` (2666).
- `scalar_array` (2735-4491): `bindings::ScalarArrayBindings` (2763, rows with a `live` flag so a retired home keeps its payload for a captured loan), trait `PackedHomeRetirement` (2861, impl for `ScalarArrayBindings` and `BTreeSet<PlaceId>`), `empty_storage::{EmptyArrayState, EmptyArrayStorage}` (3012/3029) tracking initialized zero-leaf arrays, `loans` (3275): `prepare_scalar_array_reference` (3300), `prepare_scalar_array_loan` (3321), `capture_scalar_array_loan` (3369), `lent_collection_type` (3493), `borrowed_array_memberships_match` (3522); `storage::ScalarArrayStorage<Element>` (3552, `record_storage::Record`); public `TerminalScalarArrayValue` (3684, an `Arc` portable form) and crate-private `PackedScalarArrayValue` (3732, an `Rc` runtime form with `set_element` 3791, `install_resident` 3821, `snapshot` 3852, `mutable_storage` 3861 as copy-on-write); `prepare_scalar_array_argument` (3877), `scalar_array_snapshot` (3945), `stored_scalar_array_copy` (3982), `execute_scalar_array_establishment` (4072). Large unit-test module (4113-4490).
- `structural_storage` (4493-6276): `contents` (4501): `TerminalStorageContents {Scalar, Case, Bytes, Elements}` (4526), `TerminalBoundaryStorageValue` (4544), `StorageLeaf`/`StorageLeafKind` (4549/4621; six leaf kinds), `describe`/`describe_at`/`select` (4663-4726), recursive `collect`/`collect_fields` (4728/4982), `scalar_storage` (5072), `capture` (5127); `retirement` (5262): `Region` (5278), `RetainedStorage` (5355), `queue_retired_descriptor_home` (5569), `collect_retired_storage` (5588); `original_allocation` (5808): `owned_allocation_at` (5843); `StorageKey` trait (5912) with impls for place, scalar-field key, byte-field key; `cells` range iterator (5957), `prune_rows` (5978); `HostStructuralPayload {Opaque, Materialized, Modified}` (6060) and `TerminalExecution` methods `require_guest_storage_mutation` (6067), `invalidate_stored_provenance` (6085), `export_host_atom` (6100), `forward_host_atom` (6128), `reclaim_orphan_scalar_records` (6144).

### Inputs and outputs
- Reads: execution tables (same set as primitives, plus `original_storage_roots`, `structural_host_atoms`, `component_lent_host_atoms`, `empty_array_storage`, `private_entry_ram`, `access_view_occurrences`, call-stack frames), program declarations through `structural_types()`.
- Mutates: through `ConstructorStorage::publish` (2186) / `publish_cells` (2251) which `validate_append` then `append` into 14 execution tables, and `PreparedConstructorOperands::commit` (194) which prunes consumed subtrees and replaces `live_affine_frontier` and `live_claims`.
- Produces: staged values with fresh identities, `TerminalBoundaryStorageValue` lists for host boundaries (`capture`, 5127), pinned scalar endpoints (`PinnedScalar`, 4653).

### How it decides
- Claim binding (`bind_constructor_claims`, 210-261): per result claim binding, if the path is not active in the case then no transported claim and no live claim may exist for it (231-236); if active, the claim must be transported at exactly the same destination path (238-239) or be the single "nominal root" (241); duplicates are refused (224, 257). Keyed on `ClaimId` and path.
- `nominal_root` (441-467) is accepted only for Construction origin, Linear multiplicity, empty path, a fresh claim id, a declaration with `owns_linear_claim` and `intrinsic_claim_root_by == Ok(true)`, and a Record/Sum/Mixed shape.
- `stage_constructor_operand` (371-422): an operand is "affine" if the live frontier has an overlapping entry for its place (378-384); then the frontier entry is removed (whole place) or `consume_projected_affine_complement` runs (392-398); "claimed" if any live claim path starts with the argument path (403-405); a non-affine, non-claimed projected operand is refused (418-420).
- Byte-field acceptance (38-162) keys on the declared carrier kind: shared readable view (checks bytes against domain predicates 56-75), `BoundedOwned{capacity}` (length <= capacity, 76-96), borrowed (declared type/multiplicity/descriptor transfer rules, 97-157), else `malformed` (158).
- Subtree movement: `stage_subtree` (1898-2100) selects a subtree of each table by `cells(map, root)` (range scan on `(opaque_identity, path)` keys with `starts_with(root.path)`, 5957-5969) and relocates each key by swapping the root prefix (1905-1917). With `transfers_custody == false`, provider descriptors, activation descriptors and non-shared references are refused (1941-1948, 1959-1962, 2010-2025).
- Retirement GC (`collect_retired_storage`, 5588-5804): builds the live root set from every frame's structural values, aggregate backings, loans, byte bindings, continuation, result (5609-5697), expands it through references (5698-5733), then removes retired references/views and resident cells whose identity is not retained (5743-5803). Keyed on opaque identity and path prefixes via `Region::selected_suffix` (5306).
- `reclaim_orphan_scalar_records` (6144): periodic by counters: scalars collect when the identity cursor passes `next_scalar_collection_identity` (6150), atoms when `host_atoms + completed_profiles >= next_host_atom_collection_size` (6151-6155), deferred if any loan/backing tables are non-empty (6162-6187) or scalar field count < 4096 (6184). Roots collected from live tables and everything else is removed (6239-6273). Thresholds 1024 (6160, 6248) and 4096 (6184) are literal constants.
- Storage description `collect` (4728-4980) keys on type shape and on `selected_path` when selecting a leaf (4694-4726).

### Cases by shape
- `lent_collection_type` (3493-3519): only `FixedArray` types with a scalar-leaf shape, or an `ElementView` whose source path resolves to a `FixedArray` of the same element type; else `None`.
- `stage_array` (1656-1770): `PrimitiveScalar`, `FixedArray{Literal}`, else `malformed` (1767); 8-bit unsigned arrays are special-cased into byte storage (1697-1726, comment-free); other arrays recurse per element.
- `stage_access` (1772-1896): ordered shape tests: activation-claim descriptor (1780), borrowed byte carrier (1801), `ElementView` (1823), bounded owned byte sequence (1849); then the shared subtree walk.
- `ConstructorValue::visit_scalar_leaves` (1004-1072): only `PrimitiveScalar` or a `FixedArray` with a scalar-leaf shape; others `Ok(None)`.
- `collect` in `contents` (4728): `PrimitiveScalar`, `AtomicCell` (only emits a leaf for MutableBorrow, 4751), `Record`, `ElementView`, `FixedArray` with non-literal extent refused as `UnclosedArrayExtent` (4844-4849), `FixedArray{Literal}`, `Sum`/`Mixed`, and everything else becomes an `Opaque` leaf (4971-4977).
- `establish_immutable_elements` (2342): result must have no qualifications, no claims, no establishments, and an element type that does not own a linear claim, has `no_code_affine_cleanup` and `descriptor_cleanup` (2352-2359); only `ElementView` with access `None | SharedBorrow` (2454-2457).
- `prepare_scalar_array_loan` (3321): non-Owned parameter, `Unrestricted` multiplicity, plus: for non-fixed-array formals both qualification lists must be empty ("supported_memberships", 3343-3345).
- Dense-id ratchet: `tools/ratchets/dense_id_maps.txt:115` lists this file at `5` (a count of ratcheted uses, read); `RuntimeRows` itself is a `BTreeMap` keyed by place/identity ids (primitives.rs:4911), as the AGENTS dense-ID rule describes.

### Same job elsewhere
- Row accounting: all `ConstructorStorage` tables are `RuntimeRows` (1105-1151), which is the type defined in `primitives.rs:4910`.
- `ScalarArrayBindings::retire_packed_home` (2871) and `PackedHomeRetirement for BTreeSet<PlaceId>` (2881): two implementations of "retire a home" for a live-flagged table and a plain set; both used via `custody.rs:788-803` (grepped, 1 external trait user).
- `EmptyArrayStorage::prepare_retirement`/`retire` (3219/3249) mirror the generic `prepare_subtree_retirement`/`prune_subtree` pattern used in `PreparedConstructorOperands::commit` (198-203, 5978 `prune_rows`).
- `StorageLeaf`/`collect` (4728) walks the same shape list as `ConstructorStorage::stage_array` (1656) and `ValueImageReader::structure` (`providers.rs:3894`); suspected duplicated shape walkers; each reads a different table.
- `prune` (5971, `#[cfg(test)]`) duplicates `prune_rows` (5978) for plain `BTreeMap`; test-only.
- `ScalarStorageRows::prepare_resident_replacement` (`primitives.rs:5680`) and `PackedScalarArrayValue::prepare_resident_install` (3804) both check "resident cell must keep its original backing"; second copy at `StructuralByteArrayStorage::prepare_replacement` (`ConstructorStorage::prepare_resident_writes`, 2169-2174; not read).

### Unused or one-caller code
Greps over `source-rust/` (Grep tool, uncapped).
- `establish_record` (2537): 1 call, `instructions/structural.rs:1010`, via `execute_establish_record`, dispatched at `execution.rs:3483`.
- `execute_establish_structural_array` (2447): 1 dispatch site (`execution.rs:3482`); `execute_scalar_array_establishment` (4072): 1 (`execution.rs:3349`).
- `ConstructorStorage::retain_replacement_subtree` (1527): 1 external caller, `embedding/boundary_storage_inputs.rs:203`.
- `preserve_replaced_residents` (2102): 2 external callers in `instructions/structural.rs:1669, 1914`.
- `EmptyArrayStorage::initialized_byte_array` (3185): 1 external caller, `boundary_inputs.rs:880`.
- `initialized_empty_array_in_frame` (`primitives.rs:3299`): 1 external caller, `bytes_and_views.rs:3852`.
- `ConstructorStorage::prepare_replacement` (2111): grep count of `prepare_replacement\(|prepare_resident_replacement|\.relocate_keys\(|insert_vacant\(` across the crate is 34 in 7 files; per-name counts not separated.
- `PackedScalarArrayValue::is_empty`, `len`, `elements` (test-only, 3764): not grepped; `TerminalScalarArrayValue::{elements,len,is_empty}` are `pub` (3692-3700); count not run (stated as not checked).
- `StorageKey` trait has three impls (5917, 5931, 5942); `cells` is the single iterator used by this file and others. "Single shared range-scan": uses of `structural_storage::cells`: not separately counted; 40+ visible call sites inside this file alone (read).
- `RuntimeRows` described above (254 occurrences, 14 files).

### Stated limits (quotes under 20 words)
- "resident extraction requires live forwarding of carried loans" (1309).
- "typed sequence snapshots require read-only established backing" (4781).
- "host aggregate storage has no resident codec for placed cells" (5085).
- "host aggregate re-view requires independently represented scalar leaves" (5148).
- "opaque component loan lacks original guest storage for mutation" (6077).
- "modified host structural payload requires materialized host export" (6120).
- "provider storage transport requires owned custody and live backing" (1945).
- "replacement cannot substitute another original allocation" (2141).
- Comments describing intent: "Each element delivered the declared domains" (2515), "from_values reserves and meters the complete retained roster" (2379), "A separately owned component may still hold a linear claim" (310), "A materialized resident referent owns its cells even though it has no local place" (5759).

### Fail-open risks
- `export_host_atom` (6100-6126): returns `Ok(())` and leaves the value unchanged when the home has no host-atom row and no `Modified` ancestor. Intent: a value that is not host-backed exports as is. Classification: traced (a modified host payload is caught at 6114-6123; unmodified or non-host values pass). Stated indirectly by the error text at 6120.
- `forward_host_atom` (6128): returns `None` when no `Opaque` row exists; caller treats that as "nothing to forward" (not read). Open.
- `reclaim_orphan_scalar_records` (6144): conservative "return without collecting" when `hidden_backings` are present (6185). That keeps storage (no accept/reject).
- `stage_constructor_operand` (418-421): returns `Ok(affine || claimed)`; a projected operand that is neither is refused (419), a whole-place operand that is neither returns `Ok(false)` (not transferring custody), after `require_owned_value_claims`. Traced: the caller then copies storage rather than moving it (`transfers_custody == false` paths refuse custody-requiring descriptors, 1941-1948).
- `ConstructorStorage::validate_projected_memberships` (1432-1448): rows required for a path that is not active are skipped (1443 `if active && ...`), by design for inactive sum cases. Traced.
- `validate_element_memberships` (2309-2336): a missing projected qualification row is accepted when the case projection in the child is inactive (2323-2331). Traced.
- `StorageDescription` `collect` `_ =>` arm (4971): any other type shape becomes an `Opaque` leaf rather than an error, and `capture` then skips `Opaque` leaves (5250-5252). So a shape not listed (for example `Reference`, `ByteSequence`, `AtomicCell` under a non-mutable borrow) is captured as nothing, not refused. Classification: open; not traced to see whether the host-boundary caller later requires a complete roster.
- `EmptyArrayStorage::initialized` (3080-3164): returns `Ok(true)` when all pending children resolve; a type with `length == 0` returns `Ok(false)` (3132). Traced, closed.

---

## Coverage

- `source-rust/subsystems/interpreter/memory/primitives.rs`: lines 1-7003 read in full (7003 lines).
- `source-rust/subsystems/interpreter/memory/providers.rs`: lines 1-4471 read in full (4471 lines).
- `source-rust/subsystems/interpreter/memory/records_and_arrays.rs`: lines 1-6276 read in full (6276 lines).
- Total read in full: 17,750 lines. Skimmed: 0. Not read: none.
- Other files were not read; call-site counts in this record come from Grep, not from reading those callers.

## Five things a maintainer most needs to know about this unit

1. All three files are big composites of nested `pub(crate) mod` blocks, not single-purpose files (for example `primitives.rs` holds activation claims, scalar codec, address exposure, row tables and identity counters). The `TerminalExecution` struct is extended by `impl` blocks in many modules here, so one execution method may live in any of them.
2. Every runtime table is a `RuntimeRows` (a `BTreeMap` plus a storage lease); a write that grows a table reserves a cell on the invocation account and `validate_append`/`append` check that both sides share the same account (`primitives.rs:4910-5056`). Staging builds a separate `ConstructorStorage` and publishes it in one step (`records_and_arrays.rs:2186-2299`), so a refused publish leaves the execution tables untouched.
3. Scalar reads and writes go through a `PrimitiveStorageEndpoint` (seven kinds) plus an optional `(source, view)` codec; a single place can resolve to different endpoints depending on a fixed priority chain (`primitives.rs:3369-3593`). Both encode and decode funnel through `encoding::encode/decode` (`primitives.rs:1785,1820`).
4. Provider-backed storage is keyed by identity and window (`ProviderStorageBinding {allocation, offset, length, resident}`, `providers.rs:4327`) and the five access-lifecycle operations (`providers.rs:819, 989, 1866, 2125, 2909`) each repeat the same contract-check / claim-bind / commit sequence; layout entries are matched to record fields by the string `format!("field:{:?}", field.id)` (`providers.rs:3526, 3949, 4116`).
5. Many "unsupported" shapes refuse with static strings (see "Stated limits"); the closest thing to an accepting default is `contents::collect`'s `_ => Opaque` arm (`records_and_arrays.rs:4971`), which `capture` then skips (`records_and_arrays.rs:5250`).

## What you are unsure about

- `primitives.rs:4526-4530` (`&path[..path.len() - 1]`): would underflow if the path were empty while the byte predicates include `ValidUtf8` without `AsciiOnly`. I did not trace whether `primitive_store_byte_predicates` can return such predicates for an empty path.
- Whether `RuntimeRows::try_clone` has any non-test caller: the only matching text in `instructions/terminators.rs:207` is `.try_clone(`, which I did not read, so I cannot say if it is this method.
- Whether `ScalarStorageRows::append` skipping the resident-key presence check (root `None`) can hide a lost resident cell; callers (`continuation.rs`, `custody.rs`) were not read.
- The meaning of the dense-ID ratchet number `5` for `records_and_arrays.rs` (`dense_id_maps.txt:115`); I did not read the ratchet's rules, so I do not know what is counted.
- Whether `contents::collect`'s `_ => Opaque` arm (`records_and_arrays.rs:4971`) is later rejected by the boundary caller when a non-listed shape appears; I did not read `embedding/boundary_*.rs`.
- The files are declared as `storage::*` modules via paths I only inferred from `use crate::storage::...`; I did not read `interpreter.rs` to confirm the `#[path]` mapping.
- For the "same job elsewhere" items that point to other files (`StructuralByteArrayStorage::prepare_replacement`, `custody.rs`), I did not read those sites; they are marked suspected.
