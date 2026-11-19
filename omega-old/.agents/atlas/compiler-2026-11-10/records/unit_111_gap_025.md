Second pass for unit 111

Ranges covered (paths under `omega-rust/psi/interpreter/terminal-interpreter/src/`):
- `boundary_effects.rs` 2301-3787 (1487 lines)
- `boundary_inputs.rs` 1-2814 (2814 lines)

Total 4301 lines, all read in full (I read `boundary_effects.rs` from 2285 so I could see where the first range starts; those 16 extra lines are not counted).

Mounting (grepped `lib.rs:34-35,51-52`): `boundary_effects.rs` is `crate::boundary::effects`; `boundary_inputs.rs` is `crate::boundary::structural_inputs`. Inline `mod` names inside the files are the path segments used elsewhere.

No contradiction with `unit_111.md` for these ranges. Its statement that `original_storage` starts at :2153 and `scalar_path` is at :2294 matches what I saw. Its claim that `boundary_inputs.rs` was not read means nothing in it overlaps.

---

## A. boundary_effects.rs 2301-3787

Range 2301 starts inside `original_storage::TerminalOriginalStorageLoan::scalar_path` (`impl` starting before 2294, fn at 2294-2308). The first read module of `original_storage` ends at 2334.

### A1. `original_storage` tail (2301-2334)
- **Purpose.** Finish the lookup of a scalar path inside an original storage loan, and define the typed reference value that points at one.
- **Shape.** `scalar_path` (:2294) maps a view-relative path to a backing path. `TerminalOriginalScalarBackings = Vec<Vec<Rc<dyn TerminalScalarBacking>>>` (:2311). `TerminalOriginalReferenceValue{path,target,referent_type,access}` (:2314-2319) with plain getters (:2320-2333).
- **How it decides.** If `scalar_projection` is present, it binary-searches the sorted `(path, backing_path)` table by exact path equality (:2298-2302). A missing path returns `None`. If there is no projection table, it appends `view_path` to `referent` (:2304-2306), so any path is accepted without checking it against a type.
- **Fail-open.** :2304-2306: with no `scalar_projection`, `view_path` is concatenated without validation. Reaches it: any caller passing a path for a loan without a projection table. Status: open (callers not traced; the table-less case is presumably a whole-referent loan).

### A2. `results` module (2335-2955)
- **Purpose.** Admit and commit the result a host import returns: crash admission, pre-call "can this result shape be supplied at all" checks, and storing the returned value in the execution.
- **Shape.**
  - `TerminalByteSequenceValue` (:2354), `copy` (:2366). `TerminalEffectResult` enum (:2378-2387): `Crash, Unit, Scalar, ScalarAggregate, Structural, ByteSequence, CapturedReference, Reference`. `TerminalReferencePayload{ScalarAggregate,Zeroed}` (:2390). `TerminalReferenceResult{referent,references}` (:2396).
  - `admit_boundary_crash` (:2405), `preflight_boundary_result` (:2444), `supports_captured_reference_result` (:2484), `commit_boundary_reference_result` (:2512), `supports_byte_sequence_result` (:2532), `supports_host_reference_result` (:2551), `supports_scalar_aggregate_result` (:2578), free fn `requires_materialized_host_custody` (:2615), `commit_boundary_result` (:2675).
  - Two `#[test]` fns (:2798, :2837).
- **Inputs/outputs.**
  - `admit_boundary_crash` takes the declaration, the effect and a cause. It builds a `TerminalCrash` whose `frontier_lower_bound` is `self.live_claims.keys()` (:2440) and `site_guard` is empty (:2439).
  - `commit_boundary_result` mutates `values`, `byte_sequence_values`, `structural_values`, `structural_host_atoms` or calls `publish_scalar_aggregate`/`publish_host_reference_result`.
- **How it decides.**
  - Crash: requires the effect to be a `BoundaryCall` and `*boundary == declaration.id` (:2420), converts scalar arguments to literals (:2423), then defers to `validate_boundary_crash_outcome` (:2424). Keyed on boundary identity and cause.
  - Preflight (:2449-2480): rejects if `requires_checked_result`; rejects a structural result if any of these hold: it needs materialized custody and none of the four supported carriers apply; its place is already in `structural_values`; its place is in `live_affine_frontier`; it has claims and is not `Linear`; it has projected qualifications. All keyed on place identity and type shape.
  - `commit_boundary_result` (:2681-2777) is a `match (declared result, expected result, returned value)` with six arms; each arm checks `structural_type`, `multiplicity` and `qualifications` equality between declaration and expectation, plus a `supports_*` predicate. `_ => Err(malformed())` (:2777).
  - `requires_materialized_host_custody` (:2615-2672) walks the type graph with `pending`/`visited`; returns `true` on any `Reference`, `ByteSequence`, `AtomicCell`, `Sum`, `BoundedInteger`/`RefinedScalar` field, byte-sequence field, scalar-array leaf shape, or an undeclared type id (:2628-2630).
- **Cases by shape.**
  - `supports_scalar_aggregate_result` (:2596-2610): `Record | FixedArray | PrimitiveScalar` go to `ScalarAggregatePlan::new(...).is_ok()`; `Sum` passes if every case's fields are non-erased with a scalar type; `_ => false` (:2609).
  - `supports_byte_sequence_result` (:2543-2547): only `ByteSequence(BorrowedView{access: Some(SharedBorrow)})`. The test at :2837-2953 pins this (exact access list `SharedBorrow`, `MutableBorrow`, `None`; only `SharedBorrow` and non-opaque is accepted).
  - `supports_captured_reference_result` (:2484-2509): requires every `result_reference_regions` entry to have an empty path and a `Parameter` origin, and the type to be a `Reference` with `SharedBorrow | MutableBorrow`.
  - `supports_host_reference_result` (:2551-2576): referent is either a scalar-aggregate plan or has `zeroed_structural_leaves_modulo_references`.
  - In `preflight_boundary_result` the host-reference carrier additionally needs a region with empty path and `Invocation` origin (:2462-2469).
- **Same job elsewhere.**
  - `requires_materialized_host_custody` (:2615) and `entry::requires_contents` in `boundary_inputs.rs` (:2601) are two separate type-graph walks (`pending` stack + `visited`/`BTreeSet`) over `StructuralTypeShape` returning a bool. They differ in the answer set: this one returns `true` for `Sum`, `ByteSequence`, `Reference`, `AtomicCell`, byte-sequence fields; `requires_contents` returns `true` only for `Reference`, `AtomicCell`, `BoundedInteger`/`RefinedScalar`, and treats `Sum`/`Mixed` by walking cases. Same job pattern, different predicate.
  - Four `supports_*_result` predicates each repeat `qualifications.is_empty() && projected_qualifications.is_empty() && claims.is_empty()` (:2489-2491, :2536-2538, :2555-2557; the scalar aggregate one relaxes claims, :2582-2584).
  - Suspected (not read): `engine_execution.rs:2386` also calls `requires_materialized_host_custody`; callers of `preflight_boundary_result`/`commit_boundary_result` are `storage_references.rs:1822,2001` and `engine_calls.rs:1897,3390,3576`.
- **Unused/one-caller (grepped, content search over `omega-rust`).**
  - `requires_materialized_host_custody`: 4 hits (def :2615, use :2456, use :2749 in this file, `engine_execution.rs:2386`). 3 call sites.
  - `admit_boundary_crash`: 1 call site, `engine_calls.rs:3530`.
  - `preflight_boundary_result`: 3 call sites (`storage_references.rs:1822`, `engine_calls.rs:3390`, plus none in this file besides the def).
  - `commit_boundary_reference_result`: 1 call site, `engine_calls.rs:3571`.
  - `supports_captured_reference_result`: 3 external call sites (`engine_calls.rs:3392`, `boundary_storage_inputs.rs:2056`, this file :2461).
  - `commit_boundary_result`: 3 non-test call sites (`storage_references.rs:2001`, `engine_calls.rs:1897`, `:3576`) plus 2 in this file's test.
- **Stated limits (quotes).**
  - "host import cannot establish result provenance owned by another issuer" (:2452).
- **Fail-open.**
  - `commit_boundary_result` Opaque arm (:2737-2762): accepts a `TerminalEffectResult::Structural` value when the type does not require materialized custody, and stores `value.opaque_identity` as an opaque host atom without inspecting contents. Wrong program that gets through: a host import supplying an opaque id for any non-materialized structural type. Status: traced within the function (the guard `!requires_materialized_host_custody` plus `value.path.is_empty()` and equal qualifications are the stated conditions); the identity's origin is not checked here, open.
  - `requires_materialized_host_custody` returns `true` for an undeclared id (:2628-2630). That makes the check stricter, so it is fail-closed there.
  - `commit_boundary_result` Unit arm (:2710-2714) accepts with no check beyond the match; only reachable when both result kinds are Unit. Not a risk.
  - Otherwise none seen.

### A3. `scalar_loans` module (2956-3506)
- **Purpose.** Stage scalar arguments passed to a host call as copies, let the host write replacements, then validate and write the replacements back into interpreter storage after the call. Also checks that mutable host storage has every original qualification covered.
- **Shape.**
  - `TerminalBoundaryScalarBuffer` (:2980, fields: `original_storage`, `identity: Arc<()>`, `argument_index`, `field_path`, `access`, `value`, `restriction`, `stored_scalar`, `encoding`). Its `replace` (:3020) is the host's mutator.
  - `ScalarLoanBinding` (:3044). `BoundaryScalarLoans{buffers,bindings}` (:3075): `capture` (:3085), private `capture_storage` (:3129), `validate_writeback` (:3192), `commit` (:3247).
  - `impl TerminalExecution`: `validate_scalar_loan_qualifications` (:3276), `validate_scalar_loan_restoration` (:3286), `validate_storage_loan_qualifications` (:3298), `validate_storage_loan_restoration` (:3305), private `validate_storage_loan_restoration_at` (:3313), `captured_scalar_restriction` (:3398).
  - Free fns: `satisfies_source_restriction` (:3059), `captured_scalar_restoration_owner` (:3461), `scalar_qualification_owner` (:3487).
- **Inputs/outputs.**
  - `capture` reads the `StructuralParameterDeclaration`/`StructuralArgument`, resolves the primitive loan (`capture_primitive_loan`), and pushes a buffer and a binding that share an `Arc<()>` identity (:3163-3188). Writes nothing to execution.
  - `commit` writes only for non-`SharedBorrow` bindings: `record_original_scalar_write` for `direct_original`, else `store_primitive_storage` (:3253-3269).
- **How it decides.**
  - Capture requires `parameter.access == argument.access`, access in `reading_borrow_pair_pattern`, `Unrestricted` multiplicity, and for `MutableBorrow` no qualifications or projected qualifications (:3099-3107). The scalar type must equal the declared `PrimitiveScalar` (:3093-3115).
  - Writeback validity (:3200-3222) is keyed on: `Arc::ptr_eq` identity, argument index, field path, access, scalar type, `terminal_scalar_belongs_to_type`, stored scalar, encoding, fit to storage, and for `SharedBorrow` `buffer.value == binding.original`. It then re-reads the current storage and requires it to equal the original (or the new value when `direct_original`) (:3223-3242). This is a "nothing else changed it meanwhile" check.
  - `replace` (:3024-3034): only `MutableBorrow`, same scalar type, belongs to type, fits storage, satisfies restriction.
  - `satisfies_source_restriction` (:3059-3072): with an encoding, converts back to source representation via `encoding.source_value`, then `satisfies_field`; a conversion error yields `false`.
  - Restoration (:3313-3396): target and owner must share `opaque_identity` and `target.path.starts_with(owner.path)` (:3319-3321). It then scans the current machine's and every call frame's `structural_values` for values whose identity/path prefix owns the target (:3339-3345), and requires `normal_qualifications_cover` for the value, for each matching parameter declaration (by `parameter.place == *place`, :3357) and for each operation result with that place (:3370). Then it scans `reference_referents` views (:3382-3394). An uncovered case gives `UnsupportedSemanticVariant("mutable host storage has an uncovered original qualification or owner")`.
  - `captured_scalar_restriction` (:3398-3458): finds the one field with `field.id == cell.field` in the parent's declaration (:3444-3447); requires non-erased, a scalar type, and for `Atomic*` fields only `MutableBorrow` (:3448-3456).
- **Cases by shape.**
  - `captured_scalar_restriction`: for a `Projection` endpoint it checks fragments and returns `Ok(None)` (:3403-3418). Mutable (non-shared) access to a projection with a restricted fragment is refused unless the restriction is plain `Scalar` (non-boolean) or `IeeeFloat` (:3408-3415).
  - `captured_scalar_restriction`: only `ScalarField` endpoints get a restriction; every other endpoint kind returns `Ok(None)` (:3420-3422).
  - `scalar_qualification_owner` (:3487-3505): `Scalar`, `ScalarField`, `ArrayElement`, and a `Projection` whose fragments all share one owner give an owner; `OwnedArrayElement`, `RepresentationByte`, `ViewByte` give `None`.
  - `captured_scalar_restoration_owner` (:3461-3485): a `Projection` with no single owner is refused: "mutable host scalar projection has no single original restoration owner" (:3472).
- **Same job elsewhere.**
  - Capture/validate/commit triple with an `Arc<()>` identity and a "current value must equal original" check is the same pattern as `byte_buffers::writeback` earlier in this file (:484-699 per unit_111 notes; I did not re-read it). The scalar and byte versions are separate types.
  - `satisfies_source_restriction` has 2 outside callers (`storage_records_and_arrays.rs:4711`, `boundary_storage_inputs.rs:154`), so it is shared.
- **Unused/one-caller (grepped).**
  - `captured_scalar_restriction`: 2 external call sites (`storage_records_and_arrays.rs:4709`, this file :3150) plus self-recursion at :3406.
  - `satisfies_source_restriction`: 4 call sites outside its definition (this file :3033, :3159, :3218; plus 2 external, above; total 5 including the two external ones; I counted: :3033, :3159, :3218 in this file, `storage_records_and_arrays.rs:4711`, `boundary_storage_inputs.rs:154`).
  - `scalar_qualification_owner`, `captured_scalar_restoration_owner`, `validate_scalar_loan_restoration`, `validate_storage_loan_restoration`: combined count mode gave 10 hits in this file, 3 in `boundary_storage_inputs.rs`, 2 in `storage_records_and_arrays.rs` (15 total including definitions). I did not break that down per function.
- **Stated limits (quotes).**
  - "projected host scalar replacement requires its original field restrictions" (:3413).
  - "mutable host storage has an uncovered original qualification or owner" (:3336).
  - "boundary scalar replacement violates access, carrier or original field restriction" (:3036).
- **Fail-open.**
  - `validate_scalar_loan_qualifications` (:3276-3283) and `validate_scalar_loan_restoration` (:3286-3296) return `Ok(())` when `scalar_qualification_owner` is `None`. That is the case for `OwnedArrayElement`, `RepresentationByte`, `ViewByte`, and for a `Projection` whose fragments disagree or are empty. Wrong program that gets through: a mutable host loan of such storage skips the qualification-coverage check entirely. Status: open (no comment at the site; for `OwnedArrayElement`/`RepresentationByte`/`ViewByte` I did not trace whether qualifications can attach to them).
  - `captured_scalar_restriction` returns `Ok(None)` for non-`ScalarField` endpoints (:3418, :3421), meaning no field restriction is enforced on the replacement. A mutable loan of an array element or whole scalar therefore skips field-type refinement. Status: open.
  - `validate_storage_loan_restoration_at` (:3339-3394) iterates only the values it finds; if no live value owns the target it passes (no value iterated, no reference iterated, returns `Ok`). Status: open (I did not trace whether the earlier `actual_storage_type_from_owner` call at :3423 makes an ownerless target impossible; it does `actual.ok_or_else(malformed)` at `boundary_inputs.rs:1848` but only for the `ScalarField` endpoint).
  - `satisfies_source_restriction` with `encoding = None` and a restriction accepts any `value` for which `satisfies_field` is true; no fail-open.

### A4. Top-level items after the modules (3507-3787)
- **Purpose.** The effect vocabulary and handler trait a host implements, the runtime-supply tables binding boundaries to providers, and the x86 flags environment.
- **Shape.**
  - `TerminalEffect` enum (:3519): `MemoryFence`, `BoundaryCall{operation,boundary,arguments,structural_arguments,byte_sequence_arguments,completion_receipts,result}`, `ComponentCall{...,effects: Vec<TerminalEffect>}`, `PortWrite{operation,service,port,value}`.
  - `TerminalCallStorageRequirements` (:3550). `trait TerminalEffectHandler` (:3555) with default methods. `TerminalEffectRejection{reason: String}` (:3647).
  - `TerminalRuntimeSupplies` (:3658) and `AdmittedBoundarySupply` (:3674: `CheckedProvider`, `ComponentProvider`, `DeferredComponent`, `HostImport`). `TerminalRuntimeSupplyRequest` (:3686). `From<&AdmittedProviderSelections>` (:3697). `with_component_provider` (:3727).
  - `TerminalX86FlagsEnvironment{rflags}` (:3767): `new` (:3771), `snapshot` (:3779), `restore_arithmetic_bits` (:3783).
  - Re-export of `terminal_installation` types (:3651-3655).
- **How it decides.**
  - Handler defaults are fail-closed: `resolve_provider_storage` returns `Ok(None)` (:3556); `prepare_original_storage` returns an error "handler does not supply original storage homes" (:3592-3594); `handle_effect_with_buffers` rejects non-empty storage buffers (:3605-3609); `handle_effect_with_byte_buffers` rejects non-empty byte buffers (:3618-3622); `handle_effect_result` rejects any `BoundaryCall` with a `Structural` result (:3630-3640); then calls `handle_effect` and returns `Unit`.
  - `with_component_provider` (:3732): rejects if the provider's `receiver()` differs from `self.code_identity` or the boundary already has a supply.
  - `TerminalX86FlagsEnvironment::new` (:3772): rejects if bit 1 is clear or any bit outside the mask `0x3f7fd7` is set. `snapshot` clears bits 16 and 17 (:3780). `restore_arithmetic_bits` copies only mask `0x8d5` (:3784).
- **Cases by shape.**
  - `poll_host_computation` and `poll_effect_readiness` default to `Ok(true)` (:3562-3574); `accepts_reference_results` defaults to `false` (:3576).
- **Same job elsewhere.** `BTreeMap<BoundaryMachineId, AdmittedBoundarySupply>` (:3666) is keyed by dense boundary id; the repo rule in `AGENTS.md` prefers flat tables for dense ids (inferred; I did not check `tools/dense_id_maps.txt`).
- **Unused/one-caller (grepped).** `restore_arithmetic_bits|.snapshot()|TerminalX86FlagsEnvironment::new` count mode over `psi/interpreter`: 1 hit in this file (that combined pattern, including the `snapshot` and `new` definitions' uses); `engine_execution.rs` 3, `storage_records_and_arrays.rs` 7. I did not isolate the per-function counts.
- **Fail-open.**
  - `poll_host_computation`/`poll_effect_readiness` default `Ok(true)` (:3562-3574): a handler that does not override them reports every effect ready and every host computation complete. Wrong program: none that is invalid; this is a liveness default, "stated" by the name only (no comment). Status: open.
  - `handle_effect_result` default (:3626-3643): for any effect that is not a structural-result `BoundaryCall` it calls `handle_effect` and returns `Unit`, even for a `BoundaryCall` with a scalar result. The interpreter's `commit_boundary_result` then rejects `Unit` against a `Scalar` expectation (arm match at :2715-2723 needs `TerminalEffectResult::Scalar`; `_ => Err(malformed())` :2777). Traced, fail-closed downstream.

---

## B. boundary_inputs.rs 1-2814

Header (:1-8): "Host-supplied structural entry inputs and their pre-execution validation and binding into Terminal Psi runtime storage." Range 1 starts at file top, no enclosing module. Top-level modules: `access_views` (:10), `byte_arrays` (:322, with nested `storage` :323), `case_membership` (:1715), `contents` (:2214), `initialization` (:2365), `scalar_fields` (:2462, with nested `entry` :2478), then top-level `TerminalEstablishedErasedField` (:2796) and `TerminalStructuralInputs` (:2803).

### B1. `access_views` (10-321)
- **Purpose.** Check and record that an access-view parameter of the entry machine (or of a callee) is backed by an established referent, with no aliasing between exclusive views and other arguments.
- **Shape.** `AccessViewOccurrence{ram: Option<AccessReference>, view, referent, structural_type, qualifications}` (:21). `exclusive` (:29), `overlaps` (:33), `input_parameter` (:38). `establish_access_view_inputs` (:46) for the entry. `TerminalExecution::bind_call_access_views` (:180) for calls.
- **Inputs/outputs.** Reads `module.access_view_inputs`, `module.access_view_plans`, the `establishments` the host supplied and `structural_arguments`. Produces `BTreeMap<TerminalAccessViewInput, AccessViewOccurrence>`; no mutation of execution.
- **How it decides.**
  - Entry: each establishment must be among the entry's rows (`entry_rows.contains`, :61) and unique (:69). `validate_access_view_referent` is called per establishment (:78). Every entry row must have an establishment, else `AccessViewInputsRequireCustody` (:94-99).
  - Aliasing (:101-111): if either row's access is exclusive and referents overlap (same `opaque_identity`, one path prefix of the other, :33-36), error.
  - Same-view consistency (:117-136): if a previous occurrence has the same view place, referent, type, qualifications and the same six identity fields (`view_identity`, `policy_identity`, `policy_plan_machine_identity`, `schema_identity`, `placement_commitment`, `placement_report_fingerprint`) must match, else aliasing error.
  - Against other formal arguments (:137-159): exclusive row, exclusive formal or `Owned` formal overlapping the referent gives an aliasing error.
  - Call: either the argument is a live access projection with a RAM (`resolve_live_access_projection`) and is validated against the plan whose `carrier == ram.carrier()` with the six identity fields compared by tuple equality (:208-234), or it is looked up among `self.access_view_occurrences` by view place plus the six identity fields plus an access compatibility match where a stored `MutableBorrow` covers `SharedBorrow | MutableBorrow | WriteOnlyBorrow` and everything else needs equality (:245-268). Not found: `AccessViewInputsRequireCustody`.
- **Cases by shape.** The access-compatibility `match` at :258-266 is a fixed list: only stored `MutableBorrow` is allowed to be presented as three kinds; other stored accesses must be equal. Also `ram.selected_field.is_some()` is refused in the RAM branch (:213).
- **Same job elsewhere.** The overlap test `left.opaque_identity == right.opaque_identity && prefix either way` (:33-36) is the same predicate as `validate_storage_loan_restoration_at`'s prefix tests in `boundary_effects.rs` (:3319-3327, only one direction there) and as `structural_paths_may_overlap` used in `initialization` (:2423). Suspected duplicated: `terminal_semantics::structural_paths_may_overlap` vs the local `overlaps`; I did not read the former.
  - The six-field identity tuple is compared three times in this module (:122-129, :218-231, :250-257).
- **Unused/one-caller (grepped).** `establish_access_view_inputs`: 1 call site (`engine_execution.rs:2152`). `bind_call_access_views`: 1 call site (`engine_calls.rs:459`).
- **Fail-open.** None seen. Every missing row or no-match is an error.

### B2. `byte_arrays::storage` (323-961)
- **Purpose.** Storage type for fixed byte arrays that are host-supplied or live in frames, with two backings (a local byte view, or packed scalar cells that may carry host-resident cells), plus reads/writes through an `ArrayStorage` home.
- **Shape.**
  - `StructuralByteArrayStorage{structural_type, backing, account}` (:340), private enum `StructuralByteArrayBacking{Local(ByteSequenceView), Cells(PackedScalarArrayValue)}` (:347). `PreparedStructuralByteArrayReplacement` (:352).
  - Methods (:368-582): `new`, `len`, `local_view(_mut)`, `packed`, `measure_result_custody`, `has_residents`, `prepare_replacement`, `read_view`, `read_window`, `read`, `copy_snapshot`, `relocated`, `replace_byte`, `capture_scalar`, `prepare_resident_install`, `install_resident`.
  - `byte_scalar` (:584), `ArrayStorageHome{Runtime, Frame{depth,place,structural_type}}` (:592), `ArrayStorage{referent, home}` (:602).
  - `impl TerminalExecution`: `stored_byte_array_read_view` (:617), `frame_array` (:652), `frame_byte_array_window` (:667), `stored_byte_array_length` (:731), `stored_byte_array_read` (:752), `stored_byte_array_write` (:778), `stored_byte_array_snapshot` (:817), `initialized_empty_byte_array` (:865). `scalar_byte` (:877). One `#[test]` (:915).
- **Inputs/outputs.** Reads `self.structural_byte_storage` (map keyed by `StructuralRuntimePlace`), `self.call_stack[depth].scalar_array_values`, `self.empty_array_storage`. `stored_byte_array_write` mutates the byte or the frame array cell and, on success, calls `invalidate_stored_provenance(&storage.referent)` (:811-813).
- **How it decides.**
  - `ArrayStorageHome` picks which table to read: `Runtime` uses `structural_byte_storage`, `Frame` uses `frame_array(depth, place)` where `depth == call_stack.len()` means the current frame (:657-659).
  - `frame_byte_array_window` (:667-729): requires `array.structural_type == structural_type`, `array.len() == count` and the leaf scalar to be unsigned 8-bit non-address (:688-695). It then walks `storage.referent.path` with `fixed_index_step`, multiplying index by the child element count to get an offset, and checks `end <= array.len()`.
  - A runtime array that is absent from `structural_byte_storage` is treated as valid only when `start == 0 && end == 0` and `initialized_empty_byte_array` is true (:631-636, :824-834). Otherwise error.
  - `prepare_replacement` (:428-456): only does work if the destination has a resident cell (`has_residents`); same type and length required; it installs the first resident found (:440-451), and `commit` writes each incoming byte via `replace_byte` (:361-363).
  - `scalar_byte` accepts only unsigned 8-bit non-address integers (:887-892).
- **Cases by shape.** `prepare_replacement` takes only the first resident cell: `find(|_, cell| matches!(cell, ScalarCell::Resident{..}))` (:440-447), then `prepare_resident_install(index, &resident, ...)` for that one index (:451). With several resident cells, others are not prepared here (inferred; the later `replace_byte` per byte goes through `cells.set_element`, which I did not read).
- **Same job elsewhere.** Four places read a byte array through `ArrayStorageHome` with the same `match` on `Runtime`/`Frame`: `stored_byte_array_read_view` (:627), `stored_byte_array_length` (:735), `stored_byte_array_read` (:757), `stored_byte_array_write` (:784), plus `stored_byte_array_snapshot` (:823). `Frame` arms each recompute `frame_byte_array_window`.
- **Unused/one-caller (grepped).** `stored_byte_array_(read_view|length|read|write|snapshot)\(|initialized_empty_byte_array\(` count mode: 14 hits in this file (definitions plus internal uses), 15 in `storage_bytes_and_views.rs`, 6 `storage_primitives.rs`, 4 `boundary_effects.rs`, 4 `boundary_storage_inputs.rs`, 1 `embedding.rs`. `scalar_byte(`: 4 hits in this file, 3 in `storage_bytes_and_views.rs`, 2 in `boundary_storage_inputs.rs`.
- **Fail-open.** None seen. Missing storage, wrong type or out-of-range is `malformed()` throughout.

### B3. `byte_arrays` body (963-1714)
- **Purpose.** Bind host-supplied byte-array contents into entry parameters, and construct the "borrowed byte view" bindings (descriptor + backing) when one machine passes a byte array or view to another.
- **Shape.**
  - `TerminalStructuralByteArrayValue{argument_index,path,bytes}` (:984). `byte_array_length` (:990), `array_path_type` (:1019).
  - `impl TerminalExecution`: `check_byte_view_handback` (:1029), `bind_byte_arrays` (:1056), `array_storage_root_staged` (:1165), `byte_array_storage_staged` (:1210), `loaned_byte_array_frame` (:1259), `establish_byte_array_binding` (:1284), `prepare_array_view_argument_staged` (:1430).
- **Inputs/outputs.** `bind_byte_arrays` stages into local `RuntimeRows` and only on success calls `validate_append` then `append` on `structural_byte_storage` and `byte_sequence_values` (:1158-1161). The `*_staged` functions take `&self` and an `identity_cursor`, returning a presentation value and a `ByteSequenceBinding`; they do not mutate execution.
- **How it decides.**
  - `byte_array_length` (:990-1017): a `ByteSequence(BoundedView{capacity})` gives its capacity; a `FixedArray` with literal length of unsigned 8-bit non-address elements gives its length; anything else `None`.
  - `bind_byte_arrays` (:1063-1157): path segments must be `Field` or `FixedIndex` only (:1064-1071). The parameter must have access `Owned` or a borrowed pair, nonlinear multiplicity, empty qualifications and projected qualifications, and its place must not be claimed (:1076-1087). The root must have no qualifications and the parameter's type (:1089-1093). The byte count must equal the array length exactly (:1102-1104). Duplicate storage or descriptor keys are errors (:1137-1156).
  - `check_byte_view_handback` (:1029-1054): the original qualification predicates must imply the parameter's required predicates, and for `MutableBorrow` the reverse must also hold (:1047-1050).
  - `establish_byte_array_binding` (:1284-1428): branches on `collection.storage == Borrowed` (:1295), then on `SharedBorrow` plus `LiveLength` extent (:1367-1376), else backing array. Extent must match (`Fixed(capacity)` equals measured length; `StoredLength` accepted; anything else refused, :1307-1311). `MutableBorrow` requires an empty qualification list on the source (:1305-1306, :1389-1390).
  - `prepare_array_view_argument_staged` (:1430-1712): returns `Ok(None)` if the parameter is not a reading-borrow of a borrowed byte carrier (:1441-1450). Then four paths: (a) an existing `Immutable` binding at the argument place (:1459-1521); (b) an existing `MutableArray` binding (:1522-1597); (c) neither: derive from the underlying array type and path. In (c), a missing array type or length returns `Ok(None)` (:1603-1610); otherwise it finds the source parameter (or synthesizes one from an operation result with nonlinear multiplicity and no claims, :1618-1650) and calls `fixed_byte_array_loan` (:1655-1662).
- **Cases by shape (fixed lists).**
  - (a) If the presented type differs from the parameter type, only the pair (source `BoundedView` shared-borrow, parameter `BorrowedView`) is allowed (:1479-1495). Window must have empty `backing_path` and offset 0 (:1506-1511).
  - (b) Differing types allowed only for source `MutableBorrow`, parameter `SharedBorrow`, both `BoundedView` (:1539-1553). Same empty-backing-path, offset 0, no-projected-qualifications check (:1567-1572).
  - (c) a trailing `FixedRange` path segment is split off as the window (:1599-1602).
- **Same job elsewhere.**
  - Paths (a) and (b) in `prepare_array_view_argument_staged` repeat the same block: look up source parameter in `structural_parameters.chain(block_structural_parameters)`, `fixed_byte_array_window`, check `backing_path.is_empty()` and `offset == 0`, rebuild `presented` with `parameter.structural_type`/`qualifications` (:1472-1518 and :1531-1580). The source-parameter lookup is repeated a third time in (c) (:1612-1616).
  - Suspected: `terminal_semantics::fixed_byte_array_window` and `fixed_byte_array_loan` are called here at :1496, :1557, :1655 and in `boundary_storage_inputs.rs`/`boundary_effects.rs` byte code; not read.
- **Unused/one-caller (grepped).**
  - `bind_byte_arrays`: 1 call site, `embedding.rs:2802`.
  - `byte_array_length(`: 9 hits in this file, 8 in `storage_bytes_and_views.rs`, 5 `storage_primitives.rs`, 3 `boundary_storage_inputs.rs`, 2 `storage_records_and_arrays.rs`, 2 `psi/compilation/validation/src/access_views_and_borrows.rs`, 1 `embedding.rs`, 1 `boundary_effects.rs` (31 total including definition). It is used outside the interpreter crate (the validation crate), so it is shared.
- **Fail-open.**
  - `prepare_array_view_argument_staged` returns `Ok(None)` at :1449, :1606, :1609 (not a borrowed byte carrier / no array type at path / no byte length). Wrong program that gets through: a byte-view argument whose path does not resolve to a byte array yields "no view prepared", and what the caller does with `None` I did not trace (callers are in `boundary_effects.rs` `prepare_boundary_arguments` per unit_111 notes, not re-read). Status: open.
  - `bind_byte_arrays` accepts `Owned` or borrowed parameter with any `StructuralPathSegment::Field/FixedIndex` path; the root must be unqualified. No fail-open seen.

### B4. `case_membership` (1715-2213)
- **Purpose.** Ask which case of a sum/mixed type is stored at a place, walk "actual" storage types through cases, and decide whether a path through `Case(..)` segments is active.
- **Shape.**
  - `StructuralCaseContents{structural_type, case}` (:1733). `stored_case_at_projection` (:1738).
  - `impl TerminalExecution`: `active_field_declarations` (:1758), `active_structural_claims` (:1787), `active_claims_in_value` (:1801), `actual_storage_type_from_owner` (:1824), `actual_storage_type` (:1851), `actual_storage_projection` (:1859), private `walk_actual_storage_path` (:1871), `validate_case_projections` (:1946), `case_payload_projection` (:1969), `observe_structural_case` (:2072).
  - Free fns: `validate_case_projection` (:2097), `case_projection_is_active` (:2111), `case_projection_in_value_is_active` (:2128), `active_projected_qualifications` (:2139), `case_projection_with_membership_is_active` (:2154), `case_projection_with_checked_membership_is_active` (:2167).
- **Inputs/outputs.** Reads `structural_cases` (a `BTreeMap<StructuralRuntimePlace, StructuralCaseContents>`), the type declarations, `activation_storage.owners`, `live_structural_values()`, `borrowed_entry_storage`. Produces type ids, canonical path segments, or bools. No mutation.
- **How it decides.**
  - Case activity is keyed on the stored case id for the sum-typed place at each `Case` prefix: `stored != case` gives inactive (`Ok(false)`, :2207-2209). If no stored membership is recorded, `membership(prefix)?.ok_or_else(malformed)` (:2192): absence is an error. The doc comment says it: "A failed crossing remains an error; absence is never an inactive payload." (:2165-2166).
  - A stored case id not declared in the sum type gives `malformed()` (:2204-2206).
  - `actual_storage_type_from_owner` (:1824-1849): first looks in `activation_storage.owners` (:1829), else scans live values and borrowed entry storage whose identity and path prefix own the target, and requires all candidates to agree on the type (:1843-1845). No candidate: `malformed()`.
  - `walk_actual_storage_path` (:1871-1944): per segment, `Case(case)` requires the recorded actual case to equal it, and a `Case` must be followed by `Field`; `Field` uses `field_by_identity` and requires a structural child; `FixedIndex` requires literal-length array and `index < length`; anything else (`Referent`, `FixedRange`, ...) is `malformed` (:1936).
  - `case_payload_projection` (:1969-2070): input is canonical path segments; it requires at least one `Case` segment (`saw_case`) and no dangling case (:2066-2068). Erased fields are skipped (`!candidate.relevance.is_erased()`, :2039).
- **Cases by shape.** `walk_actual_storage_path` handles only `Case`, `Field`, `FixedIndex` (`_ => Err` at :1936). `case_payload_projection` handles `Referent`, `Case`, `Field`, `FixedIndex` (the four canonical segment kinds, :1984-2063).
- **Same job elsewhere.**
  - Two path walkers over declared types with the same `Case -> Field` pairing logic: `walk_actual_storage_path` (:1871) and `case_payload_projection` (:1969). Also `captured_scalar_restriction` in `boundary_effects.rs` (:3425-3443) selects the case fields by `find_structural_case` again.
  - `case_projection_with_membership_is_active` (:2154) is a thin wrapper that adapts `Option` membership into `Result<Option>` and calls the checked version (:2160-2162).
- **Unused/one-caller (grepped).** The four `case_projection_*_is_active` names in count mode: 10 in this file, 6 `storage_providers.rs`, 6 `engine_terminators.rs`, 3 `storage_records_and_arrays.rs`, 3 `boundary_storage_inputs.rs`, 2 `storage_references.rs`, 2 `engine_custody.rs`, 1 `embedding.rs` (33 total including definitions). So they are used widely.
- **Stated limits (quote).** "absence is never an inactive payload" (:2166).
- **Fail-open.**
  - `case_projection_is_active` returns `Ok(true)` when the path has no `Case` segment (:2118-2123; also `case_projection_with_checked_membership_is_active` :2186-2188). Intended: a path with no case is not conditional. Status: stated by function meaning (doc comment at :2165 covers the absence rule); traced: the only paths that get `true` are those with no case segment.
  - `stored_case_at_projection` (:1738-1755) returns `None` on any failure (projection error, no recorded case, type mismatch, undeclared case). It feeds the checked variant, where `None` becomes an error (:2192). Fail-closed.
  - `active_structural_claims` (:1787-1799) returns an empty list when the catalogue is empty (:1793-1795), meaning no claims. Reaches it: any call with an empty claim catalogue; expected.
  - Otherwise none seen.

### B5. `contents` (2214-2364)
- **Purpose.** Bind host-supplied complete contents (every active scalar and case plus byte-array elements) into the entry's structural parameters before execution.
- **Shape.** `TerminalStructuralContents{argument_index, values: Vec<TerminalBoundaryStorageValue>}` (:2222). `TerminalExecution::bind_structural_contents` (:2228).
- **Inputs/outputs.** Uses `contents::describe(self, &root, MutableBorrow, Some(&values))` (:2257) to get leaves; stages into `ConstructorStorage`; `staged.publish(self)` at the end (:2361). Nothing is published if any argument fails.
- **How it decides.**
  - Values must be strictly increasing by path (`pair[0].path >= pair[1].path` is an error, :2241-2249).
  - Each leaf kind is checked: `requires_erased_establishment` is refused (:2266-2270); `Opaque` or `ByteField` leaves are refused (:2272-2281).
  - Visible leaves (`Scalar` or `Case`) are sorted by path and their number must equal `values.len()` (:2282-2300); each is validated (`validate_value`) and paired with a `Scalar` or `Case` content; mismatched pairing is an error (:2325).
  - Scalars that lie under a byte array leaf are not staged individually (:2308-2311); the byte array is staged as a whole from the values whose path starts with the leaf path (:2328-2350). `EmptyScalarArray` leaves stage an empty array (:2351-2356).
- **Cases by shape.** Accepts only `Scalar`, `Case`, `ByteArray`, `EmptyScalarArray` leaf kinds; `Opaque` and `ByteField` refused; others (`_ => {}`, :2357) are silently ignored in the second loop.
- **Same job elsewhere.** `initialization::validate` (:2370) checks overlap between contents and other initializers; `scalar_fields::bind` (:2684) separately binds scalar fields. Three entry-input binders (contents, byte arrays, scalar fields) each stage and validate independently.
- **Unused/one-caller (grepped).** `bind_structural_contents`: 1 call site, `engine_execution.rs:2332`.
- **Fail-open.**
  - Second loop `_ => {}` (:2357): leaf kinds other than `ByteArray`/`EmptyScalarArray` are skipped. Wrong program that gets through: none that I can identify, because the first loop already refused `Opaque`/`ByteField` and required `Scalar`/`Case` leaves to be matched. Other leaf kinds that `describe` may return and that are neither visible nor byte arrays would be accepted without contents. Status: open (I did not read `StorageLeafKind`).

### B6. `initialization` (2365-2461)
- **Purpose.** Reject entry inputs where two initializers (zero-initialized argument, complete contents, scalar field, boolean field, primitive value, byte array, access-view establishment) target overlapping storage.
- **Shape.** `validate(inputs: TerminalStructuralInputs)` (:2370).
- **How it decides.**
  - Erased-field establishments need their argument to be zero-initialized and a non-empty `field_path` and `carrier_type_identity` (:2373-2384). Note `carrier_type_identity` is a `String` and `field_path` is `Vec<String>` (:2798-2799): keyed on spelling, though here only checked for emptiness.
  - For each initializer (zero-init or contents) it builds `overlaps(other_index, path)`: same `opaque_identity` and `structural_paths_may_overlap(root.path, other.path + path)` (:2413-2424). It errors if it overlaps an earlier initializer (`.take(position)`, :2426-2428), any scalar field, boolean field, primitive value, byte array, or access-view establishment referent (:2429-2455).
  - An argument index out of range gives the same error as overlap (:2409-2412).
- **Same job elsewhere.** Overlap is computed again in `access_views` (:33) with a different helper.
- **Unused/one-caller (grepped).** `initialization::validate`: 1 call site, `embedding.rs:2750`.
- **Fail-open.** One: `overlaps` returns `false` when the other argument index is out of range (`is_some_and`, :2416-2417). Reaches it: a scalar/boolean/byte-array/primitive input naming a nonexistent argument is not flagged as overlapping here. Status: open for this function (the other binders do `.ok_or_else(invalid)` on the index, e.g. `scalar_fields::bind` :2703-2706, `bind_byte_arrays` :1072-1075, so those inputs fail later).

### B7. `scalar_fields` and `entry` (2462-2786)
- **Purpose.** Bind host-supplied scalar and boolean field values into entry parameters, and check that every scalar field that needs evidence has been supplied and satisfies its field type.
- **Shape.** `entry::validate` (:2491), `validate_type` (:2518), `requires_contents` (:2601). `TerminalStructuralScalarFieldValue{argument_index,path,field,value}` (:2666) with `From<&TerminalStructuralBooleanFieldValue>` (:2673). `bind` (:2684).
- **Inputs/outputs.** `bind` takes the machine, types, root values, the supplied fields and a `ScalarStorageRows` map, inserts each field keyed by `StructuralScalarRuntimeField::member(parent, field)` and returns the rows. Errors on duplicates (:2725-2733).
- **How it decides.**
  - Per field: parent type found by `resolve_structural_path_type`; the field must be a direct scalar field of that parent whose `scalar_type()` equals the supplied value's scalar type, and an integer must satisfy `scalar_type.admits(value)` (:2711-2722).
  - `entry::validate` (:2491-2516): for every structural parameter not zero-initialized, walks the type recursively. `validate_type` returns early if `!requires_contents(types, current)` (:2528-2530); a recursion on the same type id returns `malformed()` (:2531-2533). For `Record` fields: non-erased fields where `scalar_field_requires_evidence(field_type)` must have a stored value that `satisfies_field` (:2543-2558); structural children are recursed. For `FixedArray` it iterates every literal index (:2578-2587). `Sum`/`Mixed`, `Reference` and `AtomicCell` shapes are `malformed()` (:2588-2592).
  - After insertion, `bind` loops over operations of kind `BooleanStructuralField` whose source is a structural parameter; for those whose carrier is not a case payload it requires the boolean field value to exist (:2742-2783). Missing: `StructuralBooleanFieldMissing`.
- **Cases by shape.**
  - `validate_type` handles `Record`, `FixedArray` (literal length only: `length.literal().ok_or(malformed())`, :2579), refuses the rest listed above; `PrimitiveScalar | ByteSequence | ElementView` are accepted without inspection (:2593-2595).
  - `requires_contents` (:2601-2662): true for `Reference`, `AtomicCell`, `BoundedInteger`, `RefinedScalar` fields, or an undeclared type id; zero-length arrays and `PrimitiveScalar`/`ByteSequence` are skipped.
- **Same job elsewhere.**
  - `requires_contents` vs `requires_materialized_host_custody` (`boundary_effects.rs:2615`): see A2.
  - `bind` skips `BooleanStructuralField` operations whose carrier is a case payload (:2760-2767); presence of case-payload boolean values is therefore checked elsewhere (suspected, not read).
  - Boolean fields are converted into scalar field values via `From` (:2673-2682), so one table holds both.
- **Unused/one-caller (grepped).** `scalar_fields::bind`: 1 call site, `engine_execution.rs:2357`. `entry::validate`: called from `bind` (:2735) only (grepped `entry::validate\(` : 1 hit).
- **Fail-open.**
  - `validate_type` accepts `PrimitiveScalar | ByteSequence | ElementView` without looking at contents (:2593-2595), and `requires_contents` early-returns `Ok` when a type has no evidence-requiring field. Wrong program that gets through: a type with no evidence-requiring scalar field has no values demanded; that is the stated purpose of the function.
  - `entry::validate` skips whole zero-initialized parameters (:2501-2503). A zero-initialized argument is trusted. Status: stated by the parameter name; overlap with other initializers is checked in `initialization::validate`.
  - `bind` skips boolean-field presence for `BooleanStructuralField` operations whose source is not an entry parameter (:2751-2756) and whose carrier is a case payload (:2760-2767). Status: open for case payloads.

### B8. Tail (2788-2814)
- `TerminalEstablishedErasedField{argument_index, field_path: Vec<String>, carrier_type_identity: String}` (:2796). `TerminalStructuralInputs<'input>` (:2803): borrowed slices `arguments, contents, zero_initialized_arguments, established_erased_fields, scalar_fields, boolean_fields, primitive_values, byte_arrays, access_view_establishments, private_entry_ram`. This is the single bundle the embedding passes into execution start (`embedding.rs:2750`, `engine_execution.rs:2332/2357`, per the grepped call sites). `private_entry_ram` (:2813) is not read by anything in this file (grepped within the file: only its declaration).

---

## Coverage
- Read in full: `boundary_effects.rs` 2301-3787 (1487 lines); `boundary_inputs.rs` 1-2814 (2814 lines). Total 4301.
- Skimmed: none.
- Not read: none.

## Five things a maintainer most needs to know about this unit
1. Host results are admitted by a six-arm `match` on (declared result, expected result, returned value) at `boundary_effects.rs:2681-2777`, each arm guarded by a `supports_*` predicate; everything else is `malformed()` (:2777). A new host-result carrier needs a new arm and a new predicate, and `preflight_boundary_result` (:2455-2480) must also list it, or the call is rejected before it runs.
2. Mutable host scalar loans skip qualification and restriction checks for storage with no single owner or non-`ScalarField` endpoints: `scalar_qualification_owner` returns `None` for `OwnedArrayElement`/`RepresentationByte`/`ViewByte` (`boundary_effects.rs:3501-3503`) and `captured_scalar_restriction` returns `Ok(None)` for non-`ScalarField` endpoints (:3420-3422). Whether those cases can carry qualifications is untraced.
3. Entry-input binding is split into independent binders (`bind_structural_contents` :2228, `bind_byte_arrays` :1056, `scalar_fields::bind` :2684, `establish_access_view_inputs` :46) whose mutual non-overlap is checked in one place, `initialization::validate` (:2370), which returns `false` for an out-of-range other-argument index (:2416-2417).
4. `prepare_array_view_argument_staged` (`boundary_inputs.rs:1430-1712`) has three copy-pasted paths for the same window/presentation construction, and returns `Ok(None)` for non-resolving paths (:1449, :1606, :1609); the `None` handling is in callers outside this range.
5. Case activity is decided only from recorded case membership, and absence of a record is an error, not inactivity (`boundary_inputs.rs:2165-2166, :2192`); this is the shared rule used by 33 call-site hits across 8 files.

## What you are unsure about
- Whether `None` from `prepare_array_view_argument_staged` or `Ok(None)` from `captured_scalar_restriction` is later treated as "no check" in a way that lets a restricted scalar or a byte view go unvalidated.
- Per-function call counts for `scalar_qualification_owner`, `captured_scalar_restoration_owner`, `validate_scalar_loan_restoration`, `validate_storage_loan_restoration` (I only have a combined count of 15) and for `restore_arithmetic_bits`/`snapshot`/`TerminalX86FlagsEnvironment::new` (combined count 16 across 8 files).
- The `satisfies_source_restriction` count in A3 mixes in-file and external hits; the list in that bullet (3 in-file, 2 external) is what I read from the content grep.
- Whether `StorageLeafKind` has variants beyond those named in `contents` (`boundary_inputs.rs:2357`).
- `private_entry_ram` (:2813): I saw no use in this file; usage elsewhere not grepped.
