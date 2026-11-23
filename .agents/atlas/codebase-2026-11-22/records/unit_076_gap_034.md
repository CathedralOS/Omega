# Record: unit 076, second pass (gap_034), area 14-interpreter

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Paths are relative to `source-rust/subsystems/interpreter/embedding/`. All four ranges were read in full. Two of the ranges start mid-file inside a nested module, so their opening function bodies belong to code this pass did not read (see Coverage).

Counts below come from these commands: `grep -rn "\bNAME\b" --include=*.rs .` run in `source-rust/` (line counts), and `grep -c '#\[test\]'` or an `awk` line-range filter for test counts.

---

## 1. `boundary_storage_inputs.rs` lines 1568-2698 (end of file)

### Purpose
This range finishes the code that, when a guest program calls a host import with borrowed or owned structural storage, snapshots that storage before the call and checks and commits the host's changes afterwards. It holds the last part of `capture_graph` (originals that contain references), the module `reference_outputs` (stages "this host call stored a reference into a mutable argument"), `reference_result` (checks a reference the host returns), `structural_buffer` (the host-visible buffer types), and the top-level `BoundaryStorageInputs` struct with its capture, validate and commit steps.

### Shape
- `capture_graph` tail, `1568-1620`: for each reference leaf it resolves the live projection at `source_path + relative + Referent` (`1569-1578`), recurses into `capture_graph` (`1585`), records a `TerminalOriginalReferenceValue` on the request (`1602`), then pushes a `CapturedOriginal` (`1611`) and returns the loan. The head of this function is outside the range.
- `reference_outputs::source` `1635`: picks the one `CapturedReferenceInput` whose `argument_index` and `Arc` identity match a selection; zero or two matches is `malformed()` (`1639-1647`).
- `reference_outputs::stage` `1650-2002`: the staging step for host-written reference outputs (see section 4).
- `reference_result::BoundaryStorageInputs::require_result_captures` `2013` and `prepare_reference_result` `2043`: admission and preparation of a reference value the host returns.
- `structural_buffer` `2150-2270`: `TerminalBoundaryStructuralBuffer` `2159` (fields `original_storage`, `identity: Arc<()>`, `argument_index`, `access`, `structural_type`, `values`, `references`), `TerminalBoundaryReferenceSelection` `2170` (identity, argument index, path, access; `PartialEq` compares the `Arc` by pointer, `2177-2184`), `BoundaryReferenceReplacement` `2188`, enum `ReferenceOutputWrite {Descriptor, Address}` `2195`. Host-facing methods: `replace_reference` `2217`, `replace_reference_address` `2225`, `replace` `2257`.
- `TerminalBoundaryStorageBuffer` enum `2285` (`Scalar`, `Structural`, `ByteView`) with `select_reference` `2292`.
- `StructuralLoanBinding` `2336` (per-argument borrowed binding: `direct_original`, `identity`, `argument_index`, `source`, `source_path`, `access`, `referent`, `aggregate`, `normal_qualifications`, `original`, `leaves`, `completed_leaves`, `scalars`, `replacement`, `reference_replacement`, `stores`).
- `OwnedPayloadSnapshot` `2360`; `BoundaryStorageInputs` `2368` (`buffers`, `scalars`, `structural`, `owned`, `reference_inputs`, `byte_views`); `CapturedReferenceInput` `2377`.
- Methods on `BoundaryStorageInputs`: `is_empty` `2386`, `capture_scalar` `2395`, `capture_fields` `2416`, `capture_reference_inputs` `2546`, `validate_writeback` `2608`, `commit` `2679`.

### Inputs and outputs
- Reads: the live `TerminalExecution` (`structural_types()`, `resolve_live_projection`, `capture_reference`, `normal_qualifications_cover`, `capture_normal_qualifications`, `validate_storage_loan_restoration`, `structural_cases`, `reference_referents`), the `BoundaryMachineDeclaration` (reference outputs, result reference regions, structural parameters, normal qualifications), and storage helpers `contents::{describe, capture}`.
- `capture_fields` pushes one buffer into `self.buffers` and one `StructuralLoanBinding` into `self.structural` (borrowed) or one `OwnedPayloadSnapshot` into `self.owned` (owned). `validate_writeback` delegates to byte-view, scalar and `borrowed_completion::stage` checks, then `reference_outputs::stage`, which writes `bindings[i].reference_replacement` (`1995-2000`). `commit` consumes `self` and applies byte views, scalars, then `borrowed_completion::commit` per binding (`2679-2697`).
- `prepare_reference_result` returns a `PreparedReference` from `execution.capture_reference` (`2140`).

### How it decides (keys)
- Matching a host buffer to a captured input is keyed on identity: `argument_index` plus `Arc::ptr_eq` on the `identity` token (`1639-1642`, `1681-1685`, `2653`, `2179`). Not on a name.
- Whether a reference output slot is "active" is keyed on the structural case selected in the staged leaves, compared by canonical structural path (`1710-1745`, `1778-1787`, `2104-2113`). A path prefix lookup finds the case id.
- Whether the host's chosen source is allowed to feed the output is keyed on access coverage (`crate::storage::reference::access_covers`, `1756`, `1825`, `1837`, `2029`, `2063`) and, for the result path, on declared `result_reference_regions` matched by origin parameter index, access and `source_path` prefix of the canonical storage path (`2072-2088`).
- The lifetime-anchor check (`1794-1863`) is keyed on `BoundaryLifetimeDependency::{Referent, Stored}` argument indices and paths: at least one dependency must have the output's source argument index, cover the output access, and match either "no `Referent` step in the path and the capture is not Owned" or "path before the last `Referent` equals the stored anchor path" (`1847-1858`). If none match, `malformed()` (`1861`).
- Address-only writes (`ReferenceOutputWrite::Address`) compare old and new captured scalar storage and refuse a change; a primitive referent with no scalar loan on the old side also refuses (`1966-1988`).
- Qualification preservation is checked by `normal_qualifications_cover` only when the source access is not `SharedBorrow` (`1872-1887`, `2123-2139`).
- No decision here compares a display name or `format!` output. Errors are fixed strings or `malformed()`.

### Cases by shape
- `capture_fields` accepts only: an owned parameter whose argument access and type match (`2427-2431`), or a borrow where parameter and argument access and type match and `parameter.multiplicity == Unrestricted` (`2465-2469`); everything else is `malformed()`.
- Reference outputs: destination must be `BoundaryReferenceDestination::Parameter(index)` (the `let ... = output.destination` at `1666` is irrefutable, so this is the only variant); the destination binding must be `MutableBorrow` (`1736`); the endpoint must be a `StructuralTypeShape::Reference` (`1733`).
- A reference output on the descriptor itself cannot carry membership: required qualifications must start with `Referent` (`1901-1908`, message "reference output cannot establish membership on the descriptor itself").
- `require_result_captures` accepts only `TerminalDynamicReferenceOrigin::Parameter` origins with an empty `region.path` (`2018-2025`). Anything else is "host reference result requires captured input storage".
- `capture_reference_inputs` refuses `ByteView` buffers (`2565-2569`, "byte-view reference outputs require an exact descriptor lifetime contract").
- `select_reference` refuses `ByteView` buffers and accepts only `SharedBorrow`, `MutableBorrow`, `WriteOnlyBorrow` access (`2299-2326`).
- `stage_reference_replacement` accepts only a `MutableBorrow` buffer and one replacement per path (`2239-2248`).

### Same job elsewhere
- The "case is active at this path" lookup (`case_projection_with_membership_is_active`) appears three times with slightly different closures in this range (`1739`, `1778`, `2104`); the function lives in `boundary_inputs.rs:1726` (`case_membership` module, grepped, not read).
- The normal-qualification coverage block appears twice (`1872-1887` and `2123-2139`), differing only in the error string.
- The "find the one binding with this `argument_index`" pattern repeats (`1667-1671`, `1766-1769`, `1873-1876`, `2094-2098`, `2124-2128`).

### Unused or one-caller code
- `replace_reference_address`: 2 lines in `source-rust/` (definition `2225`; one caller `host/src/execution/host_imports/integer_parsing.rs:261`).
- `require_result_captures`: 2 lines (definition `2013`; one caller `instructions/calls.rs:3450`).
- `prepare_reference_result`: 4 lines in 3 files (grep).
- `select_reference`: 4 lines in 3 files.
- `TerminalBoundaryReferenceSelection`: 18 lines in 4 files.
- No comment in the range calls anything "the single/shared/canonical" mechanism.

### Stated limits
- "boundary has an undeclared reference output request" (`1660`).
- "byte-view reference outputs require an exact descriptor lifetime contract" (`2567`).
- "reference output selects storage invalidated by staged input case replacement" (`1790`).
- Most refusals use the generic `malformed()` with no message (about 40 sites in this range), so the reason is not recorded at the site.

### Fail-open risks
- `reference_outputs::stage` `1766-1793` and `reference_result` `2094-2117`: `if let Some(source_binding) = bindings.iter().find(...)` skips the case-membership check when the selected source has no structural binding. Input that reaches it: a reference output whose selected source argument was captured as a scalar buffer (no `StructuralLoanBinding`). Classification: traced in part. A scalar buffer has no sum cases, so the skip looks harmless, but no comment says so; `open` for the claim that every non-structural source lacks cases.
- `1873-1876` and `2124-2128`: `normal` falls back to `&[]` when no binding is found. This makes the following check stricter (empty promised list), so it fails closed. Traced.
- `1658-1663`: returns `Ok(())` with no staging when the declaration has no reference outputs, but only if no structural buffer carries reference requests; otherwise errors. Traced, closed.
- `1743-1745`: `if !active { continue; }` skips an output whose case is inactive. A request in the buffer for that path would be counted in `request_count` (`1694`) but not staged, and `request_count != staged.len()` returns `malformed()` (`1992`). Traced, closed. (A request for an inactive case therefore refuses.)
- `1706-1708`: `continue` for a binding with no declared outputs happens after its requests were counted (`1694`), so extra requests are caught by `1992`. Traced, closed.
- `2051`: delegates entirely to `execution.supports_captured_reference_result` (not read); refuses if false. Not traced.

### Tests
None in this range.

---

## 2. `boundary_values.rs` lines 1563-2764 (end of file)

### Purpose
This range finishes the `aggregates` module, which builds an `AggregatePlan` from a structural type declaration (records, fixed arrays and primitive scalars) and checks that a host-supplied aggregate value has exactly the declared scalar leaves in order. It then defines the runtime place and field-key types the interpreter uses for structural storage, and carries 12 tests.

### Shape
- Tail of `AggregatePlan::new` `1527`(head outside range)-`1681`: a work-stack walk that first pushes children, then builds a `Node` for each declaration (`Shape::{Scalar, Record, Array}`) with a leaf `count: Option<u64>` and `requires_erased_establishment`. Struct definitions are at `1487-1525` (just before the range).
- `AggregatePlan::count` `1683`, `requires_erased_establishment` `1688`, `is_scalar` `1692`, `scalar_fields` `1703` (iterative visitor with a `Visit` stack that produces each scalar leaf's path and type).
- `declared_scalar_fields` `1779`: handles records, fixed arrays and primitives (no case selected) and sums (case selected).
- `TerminalAggregateValue::set_declared_scalar_storage` `1831` and `validate_scalar_against` `1870`.
- `pub(crate) use aggregates::AggregatePlan` `2411`.
- Public value structs `TerminalStructuralBooleanFieldValue` `2414`, `TerminalStructuralPrimitiveValue` `2422`.
- `StructuralRuntimePlace` `2428` (`opaque_identity: u64`, `path: Vec<StructuralPathSegment>`), `structural_storage_path` `2433` (strips a trailing `Case` segment), `StructuralFieldNamespace {Member, CasePayload}` `2443`, `StructuralScalarRuntimeField` `2449` and `StructuralByteSequenceRuntimeField` `2485` (parent place, namespace, field id), `StructuralFieldNamespace::for_carrier` `2502`, `From<&TerminalStructuralValue> for StructuralRuntimePlace` `2511`, `scalar_view_value` `2520`.
- Tests: `aggregates::tests` `1909-2409` (8 tests) and `scalar_aggregate_tests` `2536-2763` (4 tests); `grep` count 12 `#[test]` in the range.

### Inputs and outputs
- `AggregatePlan::new(declaration, lookup)` reads declarations through a caller-supplied `lookup` closure and outputs a flat node table (`Vec<Node>` indexed by position, `1669-1680`).
- `set_declared_scalar_storage` mutates `field.contents` `stored_scalar` in a `TerminalAggregateValue`; `validate_scalar_against` only reads.
- `scalar_view_value` converts a `TerminalScalarValue` into `terminal_semantics::ScalarViewValue`.

### How it decides (keys)
- Validation is keyed on position and path: the nth leaf produced by the plan must equal the nth `field` of the payload in `path` (`field.path != path`, `1855`, `1893`) and in type and range (`value.scalar_type()`, `satisfies_field`, `scalar_fits_storage`, `1856-1859`, `1894-1898`). Order, duplicates and extras are rejected because the iterator must run out exactly (`1847`, `1885`, plus the `count` compare at `1801`, `1809`).
- Stored scalar type is keyed on the field's authored storage: `field.storage.stored_scalar()` falling back to the logical scalar (`1744-1748`, `1817-1821`).
- Sum vs record is keyed on the shape and whether a `result_case` is given: `(Record|FixedArray|PrimitiveScalar, None)` or `(Sum, Some(case))`; every other pair is `malformed()` (`1793-1826`). Test `record_and_sum_discriminators_cannot_be_interchanged` `2676` pins that.
- Case payload vs member is keyed on whether the last path segment is `Case(_)` (`2502-2508`); `structural_storage_path` removes only a trailing `Case` (`2433-2440`).
- Zero-length arrays: `count = Some(0)` regardless of the element's count (`1651-1657`), so an overflowing inner dimension under an outer zero is not an error, yet the element declaration is still walked (test `2364`).

### Cases by shape
- `AggregatePlan::new` accepts: `PrimitiveScalar` (with optional `scalar_restriction` that must have the same scalar type, `1598-1607`), shared-readable `ByteSequence` (`1608-1616`), `ElementView` with access `None` or `SharedBorrow` (`1617-1624`), `Record`, and `FixedArray` with a literal length (`1646-1666`); every other shape is `malformed()` (`1667`). The pre-walk accepts records with storage of `AccessView` or `Accessor` only to refuse them (`1563-1568`), and `FixedArray` and `_ => malformed()` (`1583-1584`).
- `declared_scalar_fields` refuses a declaration with `quotient_carrier` (`1790-1792`) and erased fields in a sum case (`1813-1815`).
- `is_scalar` is false if any record field is a non-scalar, non-structural leaf (a byte-view leaf) (`1692-1701`).
- Quote, `1578-1580`: a field that is neither `Structural` nor a plain leaf is `malformed()`.
- `scalar_scalar_view_value` refuses `Callable`, `HostAddress` and `UndeterminedNaN` (`2527-2532`).

### Same job elsewhere
- Leaf enumeration by declaration shape is also done by `contents::describe` (`crate::storage::structural_storage::contents`, referred to at `boundary_storage_inputs.rs:2433`); suspected same job (not read).
- `StructuralRuntimePlace` is converted from a value here (`2511`) and again as `StructuralRuntimePlace::from(&original)` in `boundary_storage_inputs.rs:1612`, `2497` (use, not duplication).
- `StructuralScalarRuntimeField` and `StructuralByteSequenceRuntimeField` have identical fields and `relocated` bodies (`2475-2481`, `2492-2498`), two near-copies for scalar and byte-sequence keys.

### Unused or one-caller code
- `set_declared_scalar_storage`: 7 lines in 3 files; `validate_scalar_against`: 26 lines in 2 files (mostly tests).
- `StructuralScalarRuntimeField::case_payload` `2464` and `relocated` `2475`: not grepped; skipped.
- `TerminalStructuralBooleanFieldValue`, `TerminalStructuralPrimitiveValue`: not grepped.

### Stated limits
- Test names assert the limits: "zero_array_dimensions_do_not_hide_missing_cyclic_or_unsupported_elements" (`2311`), "repeated_empty_records_publish_without_expanding_absent_storage" (`2339`). Cycles are rejected by the `active` stack (`1588` pops and checks the id).

### Fail-open risks
- `1717-1719` in `scalar_fields`: a node with `count == Some(0)` is skipped without visiting its fields. Input: a zero-length array or an empty record. Classification: traced; `AggregatePlan::new` has already walked and validated the element declaration (test `2364` proves a bad element under an outer zero still fails at construction), so skipping the visit does not accept an unexamined shape.
- `1642`: `requires_erased` is computed but no check in this range acts on it; its use is outside the range. Not traced.
- Otherwise none seen: all shape mismatches return `malformed()`.

---

## 3. `instance/storage_observation/tests.rs` lines 1-1538 (whole file)

### Purpose
Test module (`mod tests;` included from `program.rs:1343`) for host observation of borrowed entry storage, projected host atoms, and the checked "continue a parked entry in a successor program" path. It has 15 `#[test]` (grep count) and eight fixture builders.

### Shape (fixtures)
- `instance` `11`: a one-machine program with a `SharedBorrow` Boolean parameter, returns structural.
- `projected_fixture` `90`: a record "host carrier" with one field "facet" of type "empty host value", `MutableBorrow` parameter, plus a `TerminalProjectedHostAtom`.
- `projected_instance` `163` / `projected_accounted_instance` `170`: start an execution with `projected_host_atoms` and `zero_initialized_arguments`.
- `continuation_fixture` `213`: adds a helper machine and one suspending `CallUnit` with a `TerminalSuspensionCallPlan`; `shift` renumbers type ids and interns an extra name so the old and new publications differ.
- `continuation_sponsor` `308`: a `BuildEvaluationSponsor` with limit 1000 in every category.
- `nested_continuation_fixture` `620`, `branch_before_calls` `897`.

### Tests and what each pins (all `file:line` of the `fn`)
- `checked_entry_relocates_the_actual_issued_field_and_requires_one_invocation` `325`: continuing into a successor moves the issued host value to the new field path; a successor built on another invocation account fails with "same invocation".
- `a_rejoined_case_carries_its_original_scalar_payload_at_the_new_field` `388`: a sum-case payload is carried to the new case and field ids; an exposed address selector naming the old field refuses ("encoded in an exposed address").
- `a_live_byte_slice_keeps_its_original_allocation_after_publication` `530`: a subslice taken before continuation equals one taken after.
- `an_unfinished_helper_returns_its_scalar_only_in_the_checked_successor` `673`: a program with a `PendingNormalCompletion` terminator cannot run (resume, invoke, invoke_until_entry_call fail; error contains "missing result" for a wrong stack) until a successor completes it.
- `nested_continuation_resumes_the_second_caller_with_its_issued_field` `804`: a proof built for the first caller stack does not accept the second.
- `a_projected_helper_cannot_return_to_its_omitted_suffix_from_another_caller` `866`: error "a call reached an omitted continuation"; wrong flag length gives "projection flags".
- `alternative_preparations_select_the_actual_caller_before_guarding_its_shared_helper` `942`: duplicate, nested and unmatched alternative paths refuse; `completed_call_path_index` equals the flag; a finished entry "must keep guarding".
- `a_sealed_unit_entry_can_skip_preparation_but_cannot_return_through_an_open_tail` `1032`: "must arm every pending normal completion".
- `returning_from_a_branch_without_waiting_requires_a_complete_caller` `1082`.
- `a_suspended_helper_returns_a_borrow_of_the_same_authored_field` `1128`.
- `whole_borrowed_marker_copies_keep_the_issued_value_and_distinct_homes` `1284`: two marker copies with the same identity give `StructuralArgumentAliasing(100)`.
- `projected_host_atoms_require_exact_paths_types_memberships_and_fresh_identities` `1379`: ~11 bad atoms each refuse; identities 1 and 42 collide with static and argument identities.
- `projected_host_observation_preserves_absence_and_replacement` `1422`.
- `projected_host_tokens_are_reserved_before_local_allocation` `1487`: identities 0, 1, 42 are reserved.
- `entry_and_result_observe_typed_storage_and_reject_unknown_coordinates` `1500`.

### Inputs, outputs, keys
- Inputs are hand-built `TerminalModule` fixtures edited through `fixture_module_mut()`; outputs are asserts. The judgments tested are keyed on identity (opaque identity numbers, `StructuralTypeId`, `MachineId`/`OperationId` call sites, the same-account check), not on names. The fixtures intern display names ("facet", "host carrier") only to build identity tables.
- Several asserts match error text with `contains("...")` (`370`, `506`, `774`, `886`, `893`, `1014`, `1023`, `1062`, `1121`), so those tests depend on message wording. Other asserts compare error enum values (`1314`, `1346`, `1387`, `1505-1535`).

### Cases by shape
- The fixtures only cover: a Boolean primitive parameter, one empty-record child field, one `Sum` with one case and one Boolean payload, one `BorrowedView` byte sequence literal, a two-level helper chain, and a branch with two calls. No arrays, no multi-field cases, no references to non-record referents beyond one shared borrow. Not stated in a comment; this is the observed set.

### Unused/one-caller, limits, fail-open
- Test module; no `pub` functions to grep.
- `continue_checked_entry` (`program.rs:2279`): 11 lines in 3 files in `source-rust/`.
- Stated limits: none by comment.
- Fail-open risks: none seen (test code).

---

## 4. `program.rs` lines 1-2129

The file's top comment (`1-10`) says it holds the host embedding lifecycle: immutable `TerminalProgram` loading, `TerminalInstance` instantiation, import adapter binding, and post-return storage and loan observation. `TerminalProgram` itself is at `3247+`, outside this range.

### 4a. `adapters` module `12-257`

**Purpose.** Defines what a host import adapter must declare and two ready-made adapters for imports that take only scalars.

**Shape.** `TerminalImportSignature` `26` (completion, scalar parameter types, structural parameters, result, result reference regions, result lifetimes, normal qualifications), `TerminalImportStructuralParameter` `37`, `From<&BoundaryMachineDeclaration>` `45`, `TerminalAdapterError` `70`, trait `TerminalImportAdapter: TerminalEffectHandler` `97` (one method `implemented_signature`), `TerminalScalarImport<F>` `101`, `TerminalUnitImport<F>` `174`, helpers `require_normal_completion` `229`, `check_scalar_arguments` `240`.

**How it decides.** Whether an adapter fits an import is a full value comparison of `TerminalImportSignature` (derived `PartialEq`), used by the binders at `402`, `1809`; no name is used. `TerminalScalarImport::handle_effect_result` checks the argument count and each scalar type (`244-249`) and then the returned scalar's type against `result_scalar_type` (`165`).

**Cases by shape.** Both builders accept only `BoundaryCompletion::NormalReturn` (`232-237`), zero structural parameters (`117`, `189`), and a scalar result (`120`) or unit result (`192`). Others return `TerminalAdapterError` (message at `79-90`: "outside this adapter's contract"). `TerminalScalarImport::handle_effect` always refuses; only the result-aware path works (`145-152`).

**Same job elsewhere.** `interpreter-numeric-imports` `NumericImport` also implements `TerminalImportAdapter` (crate map `14-interpreter.md:56`, not re-read).

**Unused/one-caller.** `TerminalScalarImport` and `TerminalUnitImport`: 5 lines each in `source-rust/`, in two files: definitions at `program.rs` and the re-export at `interpreter.rs:148`. No constructor call in `source-rust/` (grep `for_import` not run across callers; the two grep hits per name are definition, impls, and the re-export only), so these look unreferenced outside the re-export; I did not search `tools/` or outside `source-rust/`.

**Fail-open.** None seen. `check_scalar_arguments` rejects any mismatch.

### 4b. `instance::component_binding` `259-713`

**Purpose.** A `TerminalComponentBinding` pairs a checked component provider program with the receiving slot it fills. It prepares, runs and completes provider calls on a nested `TerminalInstance`, and can be carried across a republished caller.

**Shape.** `TerminalComponentBinding` `278` (selected `AdmittedComponentProvider`, program, supplies, scalar admission, `required_imports`, `bindings`, `storage_resolver`, nested `components`, `pending`), `PendingComponent` `291`, methods `new` `305`, `unbound_imports` `352`, `bind_storage_resolver` `358`, `import` `371`, `bind_import` `381`, `bind_component` `416`, `same_provider_closure` `439`, `for_successor_receiver` `456`, `require_ready` `469`, `supply` `482`, `prepare` `505`, `invoke` `567`, `completed` `621`, `acknowledge` `625`; free fn `continue_bindings` `644`.

**How it decides.** Keyed on identities: provider identity equals the program's execution identity and the supplies' code identity, and the resolved entry equals `selected.candidate()` (`311-322`); receiver identity equals the receiver program's identity and the supply (provider, provider boundary, candidate) equals what the receiver recorded (`425-427`); host adapter vs checked provider is decided from `supplies.supplies.get(&boundary)` (`395-400`). In `invoke`, a pending call is accepted only if the caller scope, operation, scalar arguments, structural operands and input snapshot lengths equal what `prepare` stored (`579-594`). `continue_bindings` pairs old and new slots by the checked prefix's `boundary_pairs()` (`663`) and compares by `supply()` equality (`670`).

**Cases by shape.** `new` refuses a provider closure that still needs a provider or an external completion (`331-338`, "component closure needs provider or per-instance external-completion authority"). `prepare` refuses lending of non-Owned structural operands without a live caller activation (`515-521`). `continue_bindings` refuses an unused provider with two possible successor slots (`696-698`).

**Same job elsewhere.** `TerminalInstance::bind_component` `1821` repeats the checks of `TerminalComponentBinding::bind_component` `416`, once at top level and once nested, with different conditions (deferred readiness at `1847-1861` only at top level).

**Unused/one-caller.** `TerminalComponentBinding`: 8+ hits across several files (not counted; the type is re-exported). `continue_bindings` is `pub(super)`; its caller is `continue_checked_entry` (not in range).

**Stated limits.** `437-438`: "keep running the same provider program, even when its caller has been republished."

**Fail-open.** `651-653`: `continue_bindings` returns `Ok(())` if both maps are empty; with nothing bound on either side nothing is lost. Traced, closed. `687-701`: an unused previous binding (no call-site pairing) is matched to a unique successor slot with the same `supply()`; ambiguity or absence refuses. Traced, comment at `689`: "Both receivers were independently checked against this exact export." This relies on checks done by the receivers' construction (not read).

### 4c. `component_return` `716-875`

**Purpose.** Authenticates a finished provider instance and turns it into a `ComponentOwnedReturn` that keeps the provider instance alive as the owner of the returned storage until the caller acknowledges.

**Shape.** `ComponentOwnedReturn` `724` (provider, candidate, `instance_namespace`, `value`, `owner`), `into_component_return` `764`.

**How it decides.** Verification closure `769-848`, keyed on: instance not closed and invoked, program's Psi identity equals the selected provider (`770-773`); entry equals candidate, call stack, live claims and affine frontier all empty (`782-786`); a result exists (`789`). Structural results need: every result qualification is a `selected.result_membership` domain, no projected qualifications, empty path, and `owns_linear_claim` or `no_code_affine_cleanup` on the declaration (`800-809`); linear carriers need exactly one whole-path returned claim equal to `returned.claims` (`815-823`); nonlinear carriers need none (`824`); an `Opaque` host atom with the returned identity must exist (`827-836`).

**Cases by shape.** Accepts `Unit`, `Scalar`, `Structural` with the above; refuses `ScalarCase` and `ScalarArray` ("requires a selected case or array storage transfer carrier", `839`).

**Fail-open.** None seen. Failure returns the instance back with the error (`851-858`).

### 4d. `storage_observation` `877-1344`

**Purpose.** Read-only access for the host to storage after a normal return: borrowed entry arguments, the result, and live sequence length.

**Shape.** `TerminalStorageObservationValue` `892` (`Storage`, `Bytes`, `Opaque`), `TerminalStorageObservationError` `901`, methods on `TerminalInstance`: `borrowed_entry_host_atom` `921`, `observe_borrowed_entry` `951`, `observe_result_sequence_length` `977`, `observe_result` `1017`; private `selected_leaf` `1055`, `observe_leaf` `1066`, `observe_case` `1234`, `observe_array` `1287`; `mod tests;` `1343`.

**How it decides.** `observe_leaf` dispatches on `StorageLeafKind`: `Opaque` of `StructuralTypeShape::Opaque` (forwarded host atom), `Scalar` or `Case` (captured through `contents::capture`), `ByteField`, `EmptyScalarArray`, `ByteArray`, `Opaque` of a shared readable byte view (needs the retained descriptor home, else "observed byte field has no retained descriptor binding"), and any other `Opaque` is `OpaqueContents` (`1074-1231`). The `ByteField` namespace is decided by whether the path's second-to-last segment is `Case` (`1128-1132`). `observe_array` does index arithmetic through nested `FixedArray`s and checks the flat length divides evenly (`1309-1312`).

**Cases by shape.** `observe_case` takes only `[]` or `[Case(selected), Field(f)]` (`1241-1248`); `observe_array` takes only `FixedIndex` segments to a `PrimitiveScalar` (`1297`, `1324`); `observe_result_sequence_length` only for `ElementView` leaves (`1000-1005`).

**Unused/one-caller.** `observe_result_sequence_length`: 2 lines in `source-rust/` (definition `977`; one caller `compiler/build/evaluation/values/projection/values.rs:393`). `borrowed_entry_host_atom`: 13 lines in 2 files (mostly this unit's tests).

**Fail-open.** `939-947`: `borrowed_entry_host_atom` returns `Ok(None)` when no host atom is recorded at the home. Input: a borrowed empty record that the guest replaced or constructed. Stated in the doc comment `919-920`: "A record constructed by the guest has no issued value." A `Modified` payload errors (test `1451`). Stated.

### 4e. `returned_loans` `1348-1426`

**Purpose.** Lets the host read a scalar through a reference the guest returned.

**Shape.** `TerminalReturnedLoanError` `1357`, `TerminalReturnedLoan` `1375` (execution, carrier, source place, path, referent type, access), `access` `1385`, `structural_type` `1388`, `read_scalar` `1392`.

**How it decides.** Reads only for `SharedBorrow` or `MutableBorrow` (`1396-1401`); path goes through `canonical_structural_storage_path` then `primitive_read_projection_type` (`1406-1417`). Anything unresolved is `UnknownPath`.

**Unused/one-caller.** `TerminalReturnedLoan`: 4 lines in 2 files (definition, impl, `pub use` here, re-export `interpreter.rs:153`); no constructor in the range, so it is constructed elsewhere or not at all; I did not find the constructor (`grep` shows no `TerminalReturnedLoan {` other than the definition). Suspected unconstructed in `source-rust/`.

**Fail-open.** None seen.

### 4f. Entry call selection and `TerminalInstance` `1429-2129`

**Purpose.** The instance type, binding of imports, components and process-exit authority, and the selection of "entry calls" (checked suspension points where a run should stop so a successor program can continue it).

**Shape.** `SimulatedProcessDomain` `1447`; `TerminalEntryCall` `1462` (entry, program identity, `location`, `callers`, `projected`, `covers_pending`); `TerminalEntryCalls` `1501`; `EntryCallWatch` `1507` with `covers_pending` `1515` and `reached` `1522`; `TerminalProcessExitAdmission` `1555` with `admit_isolated_simulated_domain` `1562` and `answers` `1599`; `TerminalBindingRejection` `1616`; `TerminalInstanceError` `1629`; `TerminalInstance` `1656` (program, entry, `completed_entry_call`, `active_projection`, `process_domain`, `execution`, `required_imports`, `bindings`, `storage_resolver`, `components`, `invoked`, `closed`); methods `new` `1672`, `install_compiler_observations` `1699`, `take_call_observations` `1719`, `bind_storage_resolver` `1728`, `import` `1741`, `bind_import` `1751`, `bind_component` `1821`, `bind_external_completion` `1873`, `unbound_external_completions` `1907`, `external_completion` `1920`, `established_erased_fields` `1928`, `unbound_imports` `1936`, `invoke` `1943`, `select_entry_call` `1953`, `select_call_stack` `1963`, `select_projected_call_stack` `2035`, `select_entry_calls` `2065` (range ends inside it at `2129`).

**How it decides.**
- `bind_import` `1751-1819` order of checks: closed; the declaration's completion is External (refuse, must use domain admission); the supply is a component provider or deferred component (refuse `ProviderRealized`); the slot is not demanded (distinguishes `ProviderRealized`, `NotRequiredImport`, `UnknownImport`); duplicate; and finally exact signature equality (`1809`). Keyed on `BoundaryMachineId` and the signature value.
- `bind_external_completion` `1873-1905`: demand found by boundary id, then `admission.answers(program identity, this instance's process-domain `Arc` by pointer, demand)` (`1599-1608`, `ptr_eq` at `1607`).
- `admit_isolated_simulated_domain` `1562-1597`: refuses if any access-view occurrences are live on the execution or any call frame (`1573-1582`); the only external completion kind is `ProcessExit` (the `match` at `1589` has one arm).
- `select_call_stack` `1963-2031`: walks the chain outermost to innermost; at each depth it finds the operation by id inside the machine, and requires, for non-final steps, that the declared invocation's callee equals the next machine, and, for the final step, that the operation has a `suspension_crossing` (`1994-2002`). The first chain entry must be the instance's entry machine (`1980-1985`). `covers_pending` is true only if every pending normal completion site is in the chain (`2019-2025`).
- `EntryCallWatch::reached` `1522-1552`: compares the program counter `(machine, block, next_operation)` to the position after the call, and for one path also `matches_callers` against the whole call stack (`1480-1493`); for several paths it binary-searches `by_leaf` sorted by position (`1532-1537`). Reaching an omitted (projected) position without matching refuses (`1543-1549`).
- `select_entry_calls` `2065-2129`: each path must pass `check_selected_entry` (outside range), together they must cover every pending normal completion (`2083-2100`), duplicates or nested prefixes are refused (`2105-2113`), and the rest of the projected-flag consistency check at `2115-2129` is cut off by the range end.

**Cases by shape.**
- `1589-1591`: `match demand.kind { ProcessExit => {} }`, a one-variant match: process exit is the only external completion that can be admitted.
- `select_projected_call_stack` `2043-2049`: refuses unless `projected.len() == chain.len()`.
- `TerminalInstanceError::Display` `1640-1647` prints `Debug` for every variant except `Execution` (`write!(formatter, "{other:?}")`), so `Closed`, `UnboundImports` etc. show Rust debug text.

**Same job elsewhere.**
- Component slot checks are in both `bind_component` bodies (`416`, `1821`), see 4b.
- `TerminalInstance::bind_import` signature check `1808-1811` parallels `TerminalComponentBinding::bind_import` `401-406`; they differ in that the component version returns `String` errors and the instance version typed `TerminalBindingRejection`.

**Unused/one-caller.**
- `TerminalProcessExitAdmission`: 8 lines in 4 files.
- `select_projected_call_stack`: 5 lines in 3 files; `select_entry_calls`: 7 lines in 3 files; `take_call_observations`: 5 lines in 3 files (commands above).
- `TerminalInstance::established_erased_fields`, `external_completion`, `unbound_external_completions`: not grepped.

**Stated limits.**
- Doc `1461`: "An immutable program has already checked this call and its callers."
- Doc `2033-2034`: "A projected caller has no executable suffix in this publication." Only the selected stack may park; other returns to an omitted suffix refuse.
- Comment `1518`: "Selection checked the complete group together." for the `_ => true` arm of `covers_pending`.
- Error strings: "compiler call observation requires an invocation without host imports" (`1706`), "compiler observations require an unopened invocation" (`1711`).

**Fail-open risks.**
- `1515-1520` `EntryCallWatch::covers_pending` returns `true` for any group of two or more paths with a stated comment (`1518`): "Selection checked the complete group together." Traced: `select_entry_calls` checks pending-completion coverage at `2083-2100`, so the claim holds for groups built through it; a group created some other way is not reachable (`EntryCallWatch` is `pub(crate)`, constructor not seen in the range).
- `1522-1552` `reached`: `projected` is `None` for a path made by `select_call_stack` (`2018`) so the omitted-continuation refusal does not run unless projection flags were set. That is by design (documented `2033-2034`).
- `1719-1725` `take_call_observations`: returns an empty `Vec` (not an error) when observations are absent or incomplete (`filter(|o| o.complete)` then `unwrap_or_default`). A caller cannot tell "no calls" from "run did not finish". Input: any caller that takes observations before completion. Classification: open (no comment).
- `1928-1934` `established_erased_fields`: returns `&[]` if there is no execution (closed instance). Open, low impact.
- `1559`, `1599-1608`: `answers` returns false for `None` domain; closed.
- Otherwise none seen.

---

## Coverage

Read in full:
- `source-rust/subsystems/interpreter/embedding/boundary_storage_inputs.rs` lines 1568-2698 (1131 lines).
- `source-rust/subsystems/interpreter/embedding/boundary_values.rs` lines 1563-2764 (1202 lines).
- `source-rust/subsystems/interpreter/embedding/instance/storage_observation/tests.rs` lines 1-1538 (1538 lines).
- `source-rust/subsystems/interpreter/embedding/program.rs` lines 1-2129 (2129 lines).

Total read in full: 6000 lines. Skimmed: 0. Not read: none of the assigned ranges. Not read outside the ranges (the callee and head code these ranges depend on): the head of `capture_graph` and the `borrowed_completion`, `byte_views` modules of `boundary_storage_inputs.rs` (lines 1-1567); the head of `AggregatePlan::new` (`boundary_values.rs` ~1527-1562); `check_selected_entry`, `continue_checked_entry` (`program.rs:2279`), `TerminalProgram` (`program.rs:2130+`).

The ranges end at the end of `boundary_storage_inputs.rs` and `boundary_values.rs` (line counts 2698 and 2764 from the unit file match the last lines read). `program.rs` continues past 2129 (crate map says `3247` for `TerminalProgram::load`); `select_entry_calls` ends outside the range.

## Five things a maintainer most needs to know about this unit

1. `reference_outputs::stage` (`boundary_storage_inputs.rs:1650-2002`) is a single 350-line function that checks, for each host-stored reference, case activity, access coverage, lifetime anchors, normal qualifications, descriptor membership, and address-only scalar interpretation, then only at the end (`1995-2000`) writes the staged references. Nearly every refusal is the bare `malformed()` with no message (about 40 sites), so a refusal does not say which of those checks failed.
2. Host buffers are tied to captured storage by an `Arc<()>` token compared with `Arc::ptr_eq`, plus argument index (`1639-1642`, `1681-1685`, `2179`). A buffer cloned or rebuilt without that token is refused as `malformed()` rather than matched by content.
3. `AggregatePlan` (`boundary_values.rs:1500+`) is the checker that a host-supplied aggregate has exactly the declared scalar leaves in declaration order with the declared storage types. It handles only records, literal-length fixed arrays, primitives, shared byte views and shared element views; everything else, including `Reference` and `Sum` declarations inside an array, is refused (`1667`, test `2311`). Sums go through `declared_scalar_fields` only at the top level, with a selected case (`1806`).
4. `TerminalEntryCall`, `TerminalEntryCalls` and `EntryCallWatch` (`program.rs:1462-1553`) implement checked suspension for continuation: a run stops only at the exact program counter after a named call and with the exact caller stack. Projected positions (callers whose code is omitted in this publication) refuse any other arrival (`1543-1549`). The tests in `storage_observation/tests.rs` (`866`, `942`, `1032`, `1082`) are the behavior record for this.
5. `storage_observation/tests.rs` pins several behaviors by matching error message text (`contains("same invocation")`, `"encoded in an exposed address"`, `"must keep guarding"`, `"must arm every pending normal completion"`, `"a call reached an omitted continuation"`), so renaming those messages breaks tests whose checks are about identity and call-site logic.

## What I am unsure about

- Whether `TerminalScalarImport` and `TerminalUnitImport` have any user outside `source-rust/` (my grep covered `source-rust/` only; neither is constructed anywhere in `source-rust/` by the grep, which shows only definitions and the `interpreter.rs:148` re-export). Same for `TerminalReturnedLoan`, which no code in `source-rust/` appears to construct.
- Whether the missing-binding skip at `boundary_storage_inputs.rs:1766` and `2094` is safe for every non-structural source; I reasoned that scalar sources have no cases but did not read `case_projection_with_membership_is_active` or the scalar capture code.
- The part of `select_entry_calls` after line 2129 and `check_selected_entry`, so the completeness of the projected-flag conflict check is not recorded here.
- The use of `requires_erased_establishment` (`boundary_values.rs:1642`, `1688`): computed in the range, consumed outside it.
