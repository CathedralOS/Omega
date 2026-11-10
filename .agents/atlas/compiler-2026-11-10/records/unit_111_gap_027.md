Second pass for unit 111

Ranges covered (gap_027, paths under `omega-rust/psi/interpreter/terminal-interpreter/src/`):
- `embedding.rs` 1-3012 (3012 lines), read in full.

Enclosing context: the range is the whole file, so it starts at the file header comment (:1-10), not inside a function. The file is a set of nested inline modules: `adapters` (:12), `instance` (:256) with sub-modules `component_binding` (:257), `component_return` (:611), `storage_observation` (:772), `returned_loans` (:1063), and then `program` (:1876) with sub-module `declarations` (:1882). The earlier notes (`unit_111.md:10`) list this file as "NOT read", so there is nothing to contradict.

All greps below were run with the Grep tool across `<checkout>/omega-rust`.

---

## 1. `mod adapters` (embedding.rs:12-255)

**Purpose.** Lets a host program supply a Rust closure as the implementation of an import (a boundary machine the Terminal Psi program calls but does not define). The adapter carries the signature it implements. Binding code later compares that signature to the program's declaration.

**Shape.**
- `TerminalImportSignature` (:26): `completion`, `scalar_parameters`, `structural_parameters: Vec<TerminalImportStructuralParameter>` (:36), `result`, `result_reference_regions`, `normal_qualifications`. Built from a `BoundaryMachineDeclaration` by `From` (:44-65), which clones each field.
- `TerminalAdapterError` (:68): `ExternalCompletionUnsupported{kind}`, `StructuralParametersUnsupported`, `ResultKindUnsupported`.
- Trait `TerminalImportAdapter: TerminalEffectHandler` with one method, `implemented_signature` (:95).
- `TerminalScalarImport<F>` (:99, constructor `for_import` :109): closure `FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>`.
- `TerminalUnitImport<F>` (:172, `for_import` :181): closure returning `Result<(), _>`.
- Private helpers `require_normal_completion` (:227), `check_scalar_arguments` (:238).

**Inputs and outputs.** Reads a `BoundaryMachineDeclaration`; produces an adapter object. At call time reads a `TerminalEffect::BoundaryCall{arguments,..}` and returns `TerminalEffectResult::Scalar` or `()`.

**How it decides.**
- `for_import` accepts only when `completion == NormalReturn` (:230-235), there are no structural parameters (:115, :187), and the result is a scalar (:118-121) or unit (:190). Everything else is a `TerminalAdapterError`.
- `check_scalar_arguments` (:238-254) compares the argument count and each argument's `scalar_type()` to `signature.scalar_parameters`. It keys on scalar type only, not on value or range.
- `TerminalScalarImport::handle_effect_result` (:152-169) checks `value.scalar_type() != self.result_scalar_type` and rejects a mismatch (:163).

**Cases by shape.** Two adapter shapes only: scalar-parameters to scalar result, scalar-parameters to unit result. Quoted refusals: "import declares structural parameters outside this adapter's contract" (:83), "import result kind is outside this adapter's contract" (:87), "external completion {kind:?} is outside this normal-return adapter's contract" (:79).

**Same job elsewhere.** The signature built here is compared to the same declaration-derived signature in `TerminalComponentBinding::bind_import` (:398-403) and `TerminalInstance::bind_import` (:1470-1473). Because both adapters build their signature with `TerminalImportSignature::from(import)` (:113, :185), the comparison at bind time only fails if the host passes the declaration of a different import (read, inferred).

**Unused or one-caller.**
- `TerminalScalarImport|TerminalUnitImport`: 14 hits in 5 files: embedding.rs 8, `lib.rs` 1, `README.md` 1, `omega/compilation/compiler/tests/support/terminal_cleanup_resume.rs` 2, `psi/compilation/build-time-evaluation/src/machine_execution.rs` 2 (Grep count).
- `TerminalImportSignature`: 136 hits in 37 files, nearly all under `omega/interpreter/terminal-runtime/src/execution/host_imports*` and tests (Grep count). It is widely used.

**Stated limits.** The three "outside this adapter's contract" messages above.

**Fail-open risks.** None seen. `TerminalUnitImport` implements only `handle_effect` (:213), so its result goes through default trait methods I did not read (trait is in `boundary_effects.rs`, outside this range). Whether a unit import's result path via `handle_effect_result` default is correct is "open".

---

## 2. `mod instance::component_binding` (:257-608)

**Purpose.** Binds a checked *component provider* program (a second Terminal Psi program that implements a boundary of the receiver) to a receiver instance. It checks the provider matches the exact admitted supply, then runs one provider invocation at a time, keeps the provider's instance alive until the receiver collects its return, and hands the effects back.

**Shape.**
- `TerminalComponentBinding<'host,'artifact>` (:275): fields `selected: AdmittedComponentProvider`, `program`, `supplies`, `scalar_admission`, `required_imports`, `bindings` (host adapters by `BoundaryMachineId`), `storage_resolver`, `components` (nested bindings), `pending: Option<PendingComponent>`.
- `PendingComponent` (:288): `caller: ComponentCallScope`, `operation: OperationId`, `arguments`, `structural_operands`, `inputs: ComponentInputCustody`, `instance: Option<Box<TerminalInstance>>`, `completed: Option<ComponentOwnedReturn>`.
- `ComponentBindings` type alias (:298): `BTreeMap<BoundaryMachineId, TerminalComponentBinding>`.
- Methods: `new` (:302), `unbound_imports` (:349), `bind_storage_resolver` (:355), `import` (:368), `bind_import` (:378), `bind_component` (:413), `require_ready` (:434), `supply` (:447), `selected` (:454), `provider_program` (:457), `has_pending_invocation` (:460), `pending_at` (:464), `prepare` (:470), `invoke` (:531), `completed` (:585), `acknowledge` (:589).

**Inputs and outputs.** `new` reads the provider `TerminalProgram`, the runtime supplies and an optional `AdmittedScalarApplications`. `prepare` calls `program.instantiate(...)` (:487-502) and moves `bindings`, `storage_resolver` and `components` into the new instance with `std::mem::take`/clone (:516-518). `invoke` runs `instance.invoke(meter)` and on `Complete` converts the instance into a `ComponentOwnedReturn` (:570-579). `acknowledge` takes the effects out of the finished instance and moves `bindings`/`components` back into the binding (:599-605).

**How it decides.**
- `new` (:308-319) accepts only if `program.execution_identity() == portable(selected.provider())`, `supplies.code_identity == provider_identity`, and `program.resolve_entry(Some(&supplies)) == selected.candidate()`. Keyed on identity values (hash identities and `MachineId`), not names. It always compares against `TerminalExecutionIdentity::portable(..)` (:309), so a provider whose program carries a non-portable custody identity would not match (inferred from the comparison; the other constructors `borrow`/`retain` take `custody_identity()` at :2350, :2359).
- `new` rejects if `requirements.missing_providers` or `external_completions` is non-empty (:328-335): "component closure needs provider or per-instance external-completion authority".
- `bind_component` (:413-432): the nested binding's `receiving_boundary()` must be in `required_imports`, unused, the receiver identity must equal `self.program.execution_identity()` and the supply stored in `self.supplies.supplies` must equal `binding.supply()`.
- `bind_import` (:378-411): `supplies.supplies.get(&boundary)` must be `None | Some(HostImport)` (:392-397), and the adapter signature must equal the declaration's (:398-403).
- `prepare` (:480-486): if `caller.namespace.is_none()` and any structural operand has an access other than `Owned`, error "component lending requires its live caller activation".
- `invoke` (:543-558): the pending record must equal the call's `caller`, `operation`, `arguments`, `structural_operands`, and value counts of the inputs and optional source.

**Cases by shape.** None as a list. The lending test at :480-486 separates owned operands from lent ones.

**Same job elsewhere (same file).**
- `unbound_imports` (:349) and `TerminalInstance::unbound_imports` (:1598); `import` (:368) and `TerminalInstance::import` (:1403); `bind_storage_resolver` (:355) and `TerminalInstance::bind_storage_resolver` (:1390); `bind_import` (:378) and `TerminalInstance::bind_import` (:1413); `bind_component` (:413) and `TerminalInstance::bind_component` (:1483). The two sets differ in error type (`String` vs `TerminalBindingRejection`) and in checks. The instance version refuses a declaration whose `completion` is `External` (:1421-1438) and refuses slots held by a `ComponentProvider`/`DeferredComponent` supply (:1439-1449); the component version has no completion check (`new` instead rejects non-empty `external_completions` at :328-335) and requires the supply to be absent or `HostImport` (:392-397).
- The binding `prepare` and `acknowledge` move `bindings`, `components` and `storage_resolver` in and out of the instance (:516-518, :603-604). `TerminalInstance::invoke` hands the same maps to `resume_with_components` (:1633-1641).

**Unused or one-caller.** `TerminalComponentBinding`: 21 hits in 7 files, including `omega/interpreter/terminal-runtime/src/execution/components.rs` (4) and `omega/build-system/build-settings/src/activation/provider_installation.rs` (4) (Grep count). The `pub(crate)` methods `prepare`/`invoke`/`completed`/`acknowledge` are used from `engine_calls.rs` (`:1830`, `:1905` seen by grep of `returned_claims()`/`.acknowledge()`).

**Stated limits.** "component lending requires its live caller activation" (:485); "nested component differs from the exact unused receiving slot" (:426).

**Fail-open risks.**
- `bind_import` at :392-397 accepts a host adapter when `supplies.supplies` has no entry for the boundary (`None`). Which wrong program: a slot that is in `required_imports` but has no recorded supply gets a host adapter instead of a checked provider. "open": I did not trace how `requirements.imports` is built (`engine::capabilities::check` is outside this range).
- `.expect("borrowed above")` (:571), `.expect("completed invocation was authenticated")` (:593) and `.expect("completed return was authenticated")` (:595) panic rather than error if called out of order. `acknowledge` is reached from `engine_calls.rs:1905`; "open" whether a caller could reach it without `completed()` being `Some`.
- Otherwise none seen.

---

## 3. `mod instance::component_return` (:611-770)

**Purpose.** Turns a finished provider `TerminalInstance` into a `ComponentOwnedReturn`: the returned value plus the instance that still owns the storage the value refers to. Before doing so it checks that the instance finished normally and holds no unfinished activation or claims.

**Shape.**
- `ComponentOwnedReturn` (:619): `provider`, `candidate`, `instance_namespace: u64`, `value: TerminalExecutionResult`, `owner: TerminalInstance`. Accessors (:628-648), `returned_claims` (:641), `acknowledge` (:650, consumes self).
- `TerminalInstance::into_component_return` (:659-768): returns `Err((self, error))` on failure so the caller keeps the instance.

**How it decides.** (all in the closure at :664-743)
- The instance must not be closed, must have been invoked, and its `terminal_psi()` must equal `selected.provider()` (:665-668).
- `entry == selected.candidate()`, empty `call_stack`, empty `live_claims`, empty `live_affine_frontier` (:677-682).
- Result kind: `Unit` and `Scalar` pass (:689). `Structural` needs: every returned qualification has `selected.result_membership(domain)` (:695-699); no projected qualifications and empty path (:700-701); the declaration `owns_linear_claim` or `no_code_affine_cleanup` (:702-704). If linear, exactly one claim with empty path equal to `returned.claims` (:710-718); else no claims at all (:719-721). Finally a `HostStructuralPayload::Opaque(identity)` with `identity == returned.value.opaque_identity` must exist in `structural_host_atoms` (:722-731).
- `ScalarCase` and `ScalarArray` results are refused (:732-737).
- The returned namespace comes from `exposure_namespace` (:739-742).

**Cases by shape.** A fixed list of result kinds: unit, scalar, whole structural value. Quote: "component return requires a selected case or array storage transfer carrier" (:735). Structural returns must be a whole value with no path or projected qualifications.

**Same job elsewhere.** Result-kind dispatch on `TerminalExecutionResult` appears again in `observe_result` (:844-865) and `returned_loan` (:1138-1145). Each handles a different subset: components refuse `ScalarCase`/`ScalarArray`; `observe_result` handles all five; `returned_loan` handles only `Structural`.

**Unused or one-caller.** `into_component_return`: 1 call, `embedding.rs:572` (Grep over `psi/interpreter`). `ComponentOwnedReturn::returned_claims` has 1 outside call, `engine_calls.rs:1830`.

**Stated limits.** Quoted messages at :735, :706.

**Fail-open risks.** None seen: every failure arm returns `Err`. The `.expect("the completed instance was authenticated without mutation")` at :759 is safe by reading: `result` was verified `Some` at :684-687 with no mutation between (traced, same function).

---

## 4. `mod instance::storage_observation` (:772-1059)

**Purpose.** Lets a host read, after the program has returned, the contents of a borrowed entry argument or of the entry's result, by canonical structural path. It exposes read-only values (`TerminalStorageContents` or a byte view), never storage handles.

**Shape.**
- `TerminalStorageObservationValue<'storage>` (:785): `Storage(TerminalStorageContents)` or `Bytes(TerminalByteRead)`.
- `TerminalStorageObservationError` (:791): `Closed`, `NormalReturnRequired`, `NotBorrowedEntryArgument{argument_index}`, `NotStructuralResult`, `UnknownPath`, `OpaqueContents`, `Execution(..)`.
- `TerminalInstance::observe_borrowed_entry` (:808), `observe_result` (:831).
- Private `observe_leaf` (:869), `observe_case` (:941), `observe_array` (:985).
- A `#[cfg(test)]` module `architecture` (:1040-1058).

**Inputs and outputs.** Reads `execution.borrowed_entry_storage` (tuples of `(index, PlaceId, value)`), `execution.result`, `execution.structural_types()`, byte storage. Returns values; mutates nothing.

**How it decides.**
- `observe_borrowed_entry` requires `execution.result.is_some()` (:818) and an entry in `borrowed_entry_storage` whose index equals `argument_index` (:821-827).
- `observe_leaf` picks a leaf with `contents::select(execution, value, path)` (:876) and dispatches on `StorageLeafKind` (:878-938): `Scalar`/`Case` go through `contents::capture` with `SharedBorrow` (:880-889); `ByteField` reads the field, choosing the namespace `CasePayload` if the second-from-last path segment is `Path::Case(_)` else `Member` (:896-900); `EmptyScalarArray` yields an empty byte view only if the byte-array length is `Some(0)` and the array is initialized (:912-918); `ByteArray` yields empty if initialized-empty else the stored read view (:923-934); `Opaque` is `OpaqueContents` (:935).
- `observe_case` (:941-983): the selected case is `value.result_case`; an empty path returns the case; otherwise the path must be exactly `[Case(c), Field(f)]` with `c == selected` (:953-958).
- `observe_array` (:985-1038): path segments must all be `FixedIndex`; offset is computed with checked arithmetic across `FixedArray{element,length}` declarations with literal lengths; the final element must be `PrimitiveScalar` and equal the stored scalar type (:1022-1030).
- All keyed on `PlaceId`, `StructuralTypeId`, canonical path segments and identities. No source spelling.

**Cases by shape.** `observe_case`: only `[Case, Field]`. `observe_array`: only fixed-index paths into fixed arrays of primitive scalars. `observe_result` returns `NotStructuralResult` for `Unit`/`Scalar` (:861-864).

**Same job elsewhere.** Three observers for three result shapes (`observe_leaf`, `observe_case`, `observe_array`). The architecture test (:1043-1057) asserts `contents::select(` and `contents::capture(` each appear once in this module and `observe_leaf(execution,` twice, so the leaf path is deliberately shared between `observe_borrowed_entry` and `observe_result`.

**Unused or one-caller.**
- `observe_borrowed_entry|retain_returned_loans` files: 8 files (Grep files_with_matches): embedding.rs, `README.md`, `terminal_entry_writeback.rs`, `terminal_storage_budget.rs`, `terminal_mutable_boundary_provision.rs`, `terminal_build_bindings.rs`, `examples/terminal_loans.rs`, and the production file `omega/build-system/build-settings/src/settings.rs`.
- `TerminalStorageObservationValue|Error`: 72 hits in 10 files (Grep count), 35 in embedding.rs.

**Stated limits.** None as comments. The test at :1040-1058 reads the source of this very file with `include_str!("embedding.rs")` (:1044) and counts substrings, including asserting that the module does not contain "TypedTrees", "CheckedTrees" or "StructuralValue::minted" (:1054-1056). This is a source-text test of the file itself.

**Fail-open risks.** None seen. Unknown paths and opaque contents return errors.

---

## 5. `mod instance::returned_loans` (:1063-1228)

**Purpose.** Lets a host read scalars through a reference the program returned. The host must opt in before the run starts (`retain_returned_loans`), because the interpreter must keep the borrowed storage alive.

**Shape.**
- `TerminalReturnedLoanError` (:1074): `Closed`, `AdmissionAfterStart`, `AdmissionAlreadyProvided`, `NoReferenceResult`, `AdmissionRequired`, `NormalReturnRequired`, `UnknownPath`, `UnreadableAccess`, `Execution(..)`.
- `TerminalReturnedLoan<'instance,'artifact>` (:1092): `execution`, `carrier`, `source: PlaceId`, `path`, `referent: StructuralTypeId`, `access`.
- `retain_returned_loans` (:1102), `returned_loan` (:1126), `access` (:1186), `structural_type` (:1189), `read_scalar` (:1193).

**How it decides.**
- `retain_returned_loans` requires `entry_storage_open` (:1107), that `entry_reference_backings` is not already set (:1110), and that the entry machine's structural result has non-empty `reference_sources` (:1113-1121). It then sets `entry_reference_backings = Some(default)` (:1122).
- `returned_loan` requires admission (:1135), a `Structural` result (:1138-1145), resolves the path tip via `canonical_structural_path_tip` and demands the tip be `StructuralTypeShape::Reference` (:1159-1161). The storage path must be one of the signature's `reference_sources` and `case_projection_in_value_is_active` (:1168-1173).
- `read_scalar` allows only `SharedBorrow` or `MutableBorrow` access (:1197-1202); other access kinds give `UnreadableAccess`. It appends `Path::Referent` to the stored path (:1203-1205) and reads a primitive projection.

**Cases by shape.** A `ScalarCase` result is `UnknownPath` (:1140-1142); any other non-structural result is `NoReferenceResult` (:1143). Only scalar reads through the loan (no byte or case reads).

**Same job elsewhere.** `read_scalar` (:1193) and `observe_leaf` (:869) both read scalar contents by canonical path but through different readers (`read_retained_reference_scalar` vs `contents::capture`).

**Unused or one-caller.** `TerminalReturnedLoan` family: 49 hits in 12 files (Grep count `observe_borrowed_entry|retain_returned_loans|returned_loan\(|TerminalReturnedLoan\b`): includes `psi/compilation/validation/src/reference_result_custody.rs` 4, `05_typed-trees-to-checked-trees/src/borrow.rs` 2 and `checks_contracts_calls.rs` 3 (these two may be unrelated name matches; I did not read them: "suspected").

**Stated limits.** None as comments.

**Fail-open risks.** None seen; every unmatched case errors.

---

## 6. `instance` top level: `TerminalInstance`, bindings, import dispatch (:1230-1875)

**Purpose.** The host-facing handle for one running instance of a program. The host binds imports (adapters, components, process-exit domain), then calls `invoke(meter)` repeatedly until finished, then observes the result and effects, then closes.

**Shape.**
- `SimulatedProcessDomain` (:1248): a unit struct. `TerminalProcessExitAdmission` (:1250): `program`, `domain: Arc<SimulatedProcessDomain>`, `demand`. `admit_isolated_simulated_domain` (:1257), `answers` (:1294).
- `TerminalBindingRejection` (:1311): nine variants. `TerminalInstanceError` (:1324): `Closed`, `UnboundImports`, `UnboundExternalCompletions`, `Execution`. `TerminalInstanceCloseError::Busy` (:1347).
- `TerminalInstance` (:1351): `program`, `entry`, `process_domain`, `execution: Option<TerminalExecution>`, `required_imports`, `bindings`, `storage_resolver`, `components`, `invoked`, `closed`.
- Methods (:1364-1738): `new`, `program`, `bind_storage_resolver`, `import`, `bind_import`, `bind_component`, `bind_external_completion`, `unbound_external_completions`, `external_completion`, `established_erased_fields`, `unbound_imports`, `invoke`, `observe_private_entry_ram`, `bind_provider_storage`, `take_returned_provider_storage`, `active_component_invocation_count`, `effects`, `result`, `observed_entry_result`, `is_finished`, `is_closed`, `close`.
- `ImportDispatch` (:1740) implements `TerminalEffectHandler` (:1778-1874) by forwarding each effect method to the adapter bound to the effect's boundary.

**Inputs and outputs.** `new` receives a started `TerminalExecution` and sets `execution.process_domain` to a fresh `Arc<SimulatedProcessDomain>` (:1370-1371). `invoke` calls `execution.resume_with_components(meter, &mut ImportDispatch{..}, &mut self.components)` (:1633-1641) and returns `TerminalExecutionStatus`. `close` drops execution and bindings (:1732-1735).

**How it decides.**
- `bind_import` (:1413-1481) rejects in this order: closed; the declaration's `completion` is `External` (:1421-1438); the boundary's supply is `ComponentProvider`/`DeferredComponent` (:1439-1449); the slot is not an `import` (then `ProviderRealized` if the supply is `CheckedProvider`/`ComponentProvider`, else `NotRequiredImport` if declared, else `UnknownImport`, :1450-1466); duplicate binding (:1467); signature inequality (:1471).
- `bind_component` (:1483-1533) requires receiver identity equality, the boundary in `required_imports`, no existing binding. If the supply is `DeferredComponent`, the exact-supply check is skipped and the supply is replaced by `expected` (:1509-1530). If the instance was already invoked, it demands the deferred supply, `awaiting_provider_readiness`, and no `failure` (:1511-1523).
- `bind_external_completion` (:1535-1567): the demand is found by `boundary`; `admission.answers(program, domain, demand)` compares program identity, the demand value and `Arc::ptr_eq` of the domain (:1300-1302). A unit struct inside an `Arc` still has its own allocation, so pointer equality identifies the instance (inferred).
- `invoke` (:1605-1643): refuses if any required import is unbound, except those whose supply is `DeferredComponent` (:1612-1620), and if any external completion is unbound.
- `close` refuses with `Busy` only when invoked and not finished (:1724-1730).
- `admit_isolated_simulated_domain` (:1257-1292) refuses if the execution or any call-stack frame has `access_view_occurrences` (:1268-1277). It then has a single-arm `match demand.kind { ProcessExit => {} }` (:1284-1286).
- Everything is keyed on `BoundaryMachineId` and identities. Strings appear only in error text.

**Cases by shape.** `ImportDispatch::adapter_for` (:1759-1775): only `BoundaryCall` reaches an adapter; `ComponentCall`, `MemoryFence` and `PortWrite` are rejected with messages: "a component completion receipt is not a host effect request" (:1766), "completed ordering observations are not host effect requests" (:1769), "port output requires an admitted target-environment provider" (:1772). External completion has one kind (`ProcessExit`, :1285).

**Same job elsewhere.** Duplicated bind/ready/unbound logic with `TerminalComponentBinding`, see section 2. The rule "all imports bound except deferred ones" appears at :1612-1620 (instance) and as `require_ready` (:434-445, no deferred exception) for components.

**Unused or one-caller (Grep counts).**
- `defer_component|admit_isolated_simulated_domain|bind_external_completion|SimulatedProcessDomain`: 16 hits in 7 files; outside embedding.rs: `omega/interpreter/terminal-runtime/src/execution.rs` 2, `tests/terminal_effect_readiness.rs` 2, `engine_execution.rs` 1, `README.md` 1, `tests/support/component_input_controls.rs` 1, `build-settings/src/activation.rs` 1.
- `is_compiler_primitive`: 1 external call, `omega/interpreter/terminal-runtime/src/execution/host_imports.rs:74`.
- `external_completion_requirement`: 2 external calls (`terminal-runtime/src/execution.rs:262`, `execution/components.rs:49`).
- `entry_installed`: external calls at `terminal-runtime/src/execution.rs:295`, `execution/entry.rs:40` and `:77`, `build-settings/src/calling_policy_execution.rs:145`, `tests/support/component_input_controls.rs:265`.
- `execution_requirements_with_private_entry_ram`: 1 production call (`terminal-runtime/src/execution.rs:306`) plus 2 uses in this file's own test (:2971, :3006).
- `active_component_invocation_count`: 1 production call (`terminal-runtime/src/execution.rs:411`) plus 3 in `tests/support/component_input_controls.rs`.
- `observe_private_entry_ram` on the instance: 1 production call (`terminal-runtime/src/execution.rs:421`).

**Stated limits.** Messages: "component installation requires its reserved slot and a checked readiness pause" (:1520), "provider storage resolver is already frozen or instance is closed" (:1396), "external completion requires separate domain admission" (:2530).

**Fail-open risks.**
- `ImportDispatch::resolve_provider_storage` returns `Ok(None)` when no resolver is bound (:1783-1786). What it admits: a request for provider storage with no resolver is answered "nothing supplied" rather than rejected. The caller (`engine_calls.rs`/`storage_providers.rs`) decides; not traced: "open".
- `ImportDispatch::storage_requirements` (:1808-1821) returns the default (empty) requirements and `accepts_reference_results` (:1802-1806) returns `false` when no adapter is bound for the boundary. For unbound slots `invoke` has already refused (:1612-1623), except `DeferredComponent` slots; "traced" for adapters, "open" for deferred slots.
- `is_finished` is `true` when `execution` is `None` (`is_none_or`, :1710-1714), and `unbound_imports` yields nothing when closed (:1599-1602). Both follow from a closed instance having no execution; `invoke` checks `closed` first (:1609). "traced".
- `bind_import` for an `External` completion slot is rejected, so the adapter path cannot supply an external completion (:1421-1438).
- Otherwise none seen.

---

## 7. `mod program::declarations` (:1882-1987)

**Purpose.** A lookup table over a sorted slice of declarations, with an optional second sorted slice of replacements that shadow the base rows by identity. It is how target-resolved structural types override the artifact's own.

**Shape.** Trait `ProgramDeclaration { type Identity: Ord; fn identity() }` (:1888). `ProgramDeclarations<'storage, D>` (:1893) holds `declarations` and `replacements`. Methods: `new` (:1899), `with_replacements` (:1906), `get` (:1916), `declaration` (:1933, errors with `malformed()`), `values` (:1940), `contains_key` (:1955). Implementations for `TerminalMachine`, `Block`, `StructuralTypeDeclaration`, `BoundaryMachineDeclaration` (:1960-1986).

**How it decides.** `get` does `binary_search_by` on `replacements` first, then on `declarations` (:1920-1930), keyed on the identity (`MachineId`, `BlockId`, `StructuralTypeId`, `BoundaryMachineId`). `values()` maps each base row to its replacement if one exists (:1947-1952).

**Observed asymmetry.** `get` can return a replacement whose identity has no base row (:1920-1925), but `values()` iterates base rows only (:1947), so such a replacement is visible to `get`/`contains_key` and invisible to `values()`.

**Fail-open risks.** Binary search assumes both slices are sorted by identity. An unsorted slice would silently miss a row. A miss returns `None`/`malformed()` (fail-closed, :1937). Whether the loader guarantees order I did not trace: "open".

**Unused or one-caller (Grep).**
- `ProgramDeclarations`: 89 hits in 10 files (Grep count). `with_replacements` has 1 caller (:2163). `ProgramDeclarations::new` is called at :2132, :2193 and 4 times outside (`engine_custody.rs:2572`, `:2743`; `engine_calls.rs:2276`, `:2278`).
- `impl ProgramDeclaration for Block` (:1967): I found no use of `ProgramDeclarations` instantiated for `Block` in three greps (`ProgramDeclarations<[^>]*Block`, `::new`, `with_replacements`). Marked "suspected" unused: usage could be through inferred types I did not trace.

---

## 8. `mod program`: program storage and admission (:1876-2261, :2263-2914)

**Purpose.** An immutable, shareable, admitted Terminal Psi program (`TerminalProgram`, an `Arc<AdmittedTerminalProgram>`). It resolves the entry machine, validates layout and supply selections, builds runtime supplies from a host request, and prepares an execution (`prepare_execution`) after checking everything the host must supply.

**Shape.**
- `AdmittedTerminalProgram<'a>` (:1999): `identity: TerminalExecutionIdentity`, `catalogs: Arc<ProgramStorage>`, `target_layouts: Option<Arc<AdmittedTargetLayouts>>`, `scalar_geometry: Option<Arc<[StructuralTypeDeclaration]>>`.
- `ProgramStorage` (:2007): `Borrowed(&TerminalModule)`, `Owned(TerminalModule)`, `Retained(RetainedTerminalCode)`; `Deref` to `TerminalModule` (:2013).
- `AdmittedTerminalProgram` methods: `resolve_entry` (:2042), `validate_layout_selection` (:2099), `machines` (:2131), `machine` (:2135), `verified_machine` (:2142), `verified_boundary_machine` (:2151), `structural_types` (:2160), `access_lifecycle_plans` (:2173), `structural_declaration` (:2183), `boundary_machines` (:2190), `has_provider` (:2196), `is_target_family_head` (:2203), `import_declaration` (:2217).
- `TerminalProgram` (:2232) public API: `module`, `entry`, `entry_installed`, `structural_type`, `execution_requirements`, `execution_requirements_with_private_entry_ram`, `load` (:2332), `borrow` (:2345), `retain` (:2357), `load_envelope` (:2366), `terminal_psi`, `execution_identity`, `admit_runtime_supplies` (:2388), `defer_component` (:2475), `select_external_supply` (:2491), `imports` (:2547), `import`, `is_compiler_primitive`, `external_completion_requirement`, `instantiate` (:2586), `from_admitted`, `start_execution` (:2604), `execution_program` (:2625), `prepare_execution` (:2642).
- `TerminalProgramEntry` (:2245): `machine`, `scalar_parameters`, `structural_parameters`, `result`, private `access_view_inputs`; `access_view_inputs()` filters by `row.machine == self.machine` (:2254-2260).
- `TerminalInstantiation` (:2814): `scalar_arguments`, `structural_inputs`, `installation`, `account`, `entry_results`, `scalar_application`, `scalar_admission`. `TerminalScalarApplicationRequest` (:2825).
- `admitted_scalar_inputs` (:2830), `scalar_application_inputs` (:2869), `compiler_owns_boundary_execution` (:2908).

**Inputs and outputs.** Reads the `TerminalModule` catalogs (machines, boundary machines, structural types, provider candidates, target-machine families, folded constants, scalar applications, layout applications, lifecycle plans) and the host's `TerminalRuntimeSupplyRequest`. Produces a `TerminalRuntimeSupplies`, a `TerminalExecutionRequirements` (via `engine::capabilities::check_with_private_entry_ram`, outside the range) and a started `TerminalExecution`.

**How it decides.**
- `resolve_entry` (:2042-2097): installation identity must equal program identity (:2046). If `folded_constants.target` is set, the supplies must carry the same target and the selected target machines must be for that target (:2050-2066). The entry is `installation.entry` else the artifact's entry (:2067-2070). With selected target machines it calls `selected.entry(...)` (:2073-2076). Without them, a `Machine` root that is a member of any target-machine family is refused as `TargetMachineSelectionMissing` (:2079-2085), and a `Boundary` root is always refused (:2088). Finally the machine must exist in `machines()` (:2093-2095).
- `validate_layout_selection` (:2099-2129): if layouts are present, `layouts.terminal_psi()` must equal the program's identity and target machines must be present, same program and same `target()` (:2104-2113). If no layouts: error if any `target_layout_applications` exists (:2114-2117), or any `access_lifecycle_plans` row has a `target` (:2118-2127). Otherwise `Ok`.
- `import_declaration` (:2217-2228): not a provider candidate (`has_provider` binary search on `provider_candidates` by boundary, :2196-2201), not a target-family head (:2203-2215), role `Callable`, and not compiler-owned.
- `compiler_owns_boundary_execution` (:2908-2914): true iff the declaration's `execution != CheckedBoundaryExecution::Provider`. If `declaration.declaration` is `None` the result is `false` (:2911), so a boundary machine with no checked declaration is treated as a host import.
- `admit_runtime_supplies` (:2388-2473): checks installation and target-machine identity (:2393-2403); x86 flags require an x86-64 target (`accepts_target_name`, :2420-2429); memory ordering requires `model.accepts_target(target)` (:2430-2438); native layouts require a target, `layouts.accepts(identity, target)`, and every layout machine to exist (:2439-2456); then each `host_imports` boundary goes through `select_external_supply` (:2465-2471).
- `select_external_supply` (:2491-2545): the boundary must be declared (:2505); role `Callable` (:2506); not compiler-owned (:2511); if a target-family head, the selected member must be a `Boundary` (:2516-2527); completion must be `NormalReturn` (:2528); and the boundary must not already have a supply or an `assumed_bindings` entry (:2533-2542).
- `prepare_execution` (:2642-2809): refuses mixing `scalar_application` and `scalar_admission` (:2661-2668); refuses ordinary execution when `scalar_applications.conditions` is non-empty and no scalar values were supplied (:2679-2693); requires the account's measurement identity to be `TerminalPsi` (:2694-2703); validates layout; runs capability check; every `requirements.scalar_applications` must have a value (:2715-2728); unless a scalar-application request is present, every `scalar_entry_conditions` must have a value (:2729-2744); refuses missing providers (:2745-2749); validates structural inputs; converts boolean fields (:2752-2762); starts the execution; requests entry checks or results (:2790-2798); binds byte arrays (:2800-2804); retains host structural payloads (:2805-2807); records `external_demands` (:2808).
- `admitted_scalar_inputs` (:2830-2867): the installation must be present; if the entry machine has a declaration, `primary.admits_machine(module, entry)` must hold (:2844-2851); program identity, target machines, provider selections, target layouts and `assumed_bindings` must equal the admission's, and every supply the admission expects must be present and equal in the installation (:2853-2864).
- `scalar_application_inputs` (:2869-2905): same shape of check but with `installation.supplies != expected_providers.supplies` (exact equality, :2890) and with the entry and an absent value for this application checked (:2893-2900).
- Keys are identities throughout (`MachineId`, `BoundaryMachineId`, `TerminalPsiIdentity`, target name via `target()`); target names are strings compared by `accepts_target_name`/`accepts_target`, which I did not read.

**Cases by shape.**
- `resolve_entry`: three entry shapes (machine; family member via selection; boundary refused), see above.
- `ProgramStorage`: three storage shapes.
- External completion: one kind (`ProcessExit`) in `admit_isolated_simulated_domain`.

**Same job elsewhere.**
- Four constructors `load` (:2332), `borrow` (:2345), `retain` (:2357), `load_envelope` (:2366) each build `AdmittedTerminalProgram{..., target_layouts: None, scalar_geometry: None}` inline.
- `admitted_scalar_inputs` (:2830) and `scalar_application_inputs` (:2869) both compare the installation to a set of admitted selections and return the scalar values. They differ: the first lets the installation hold extra supplies (checks only `expected.supplies` entries, :2859-2862), the second requires `installation.supplies == expected.supplies` (:2890). They return different error variants (`ScalarApplicationEvaluation(String)` at :2837 vs `UnsupportedSemanticVariant` at :2876). Call counts: `admitted_scalar_inputs` 2 (`embedding.rs:325`, `:2674`), `scalar_application_inputs` 1 (`:2670`) (Grep).
- `imports()` (:2547-2557) restates the filter of `import_declaration` (:2221-2227): Callable, not compiler-owned, no provider, not family head.
- `compiler_owns_boundary_execution` call sites: 4 in this file (:2226, :2511, :2553, :2567) and 1 in `engine_execution.rs:479` (Grep with content).

**Unused or one-caller (Grep counts).**
- `verified_machine` / `verified_boundary_machine`: about 60 call sites in `terminal-interpreter/src` (`engine_calls.rs`, `engine_structural.rs`, `engine_terminators.rs`, `engine_execution.rs`, `storage_*.rs`, `boundary_*.rs`). Not a one-caller.
- `start_execution`: 1 call, `lib.rs:163` (Grep `\.start_execution\(`).
- `TerminalProgram::(load|load_envelope|borrow|retain)(`: 85 hits in 33 files, 1 in embedding.rs (the test at :2928).
- `fixture_module_mut` (:2033, `cfg(test)`): not grepped.
- `is_compiler_primitive`, `external_completion_requirement`, `entry_installed`, `execution_requirements_with_private_entry_ram`: counts in section 6.

**Stated limits.** Quotes: "external completion requires separate domain admission" (:2530); "compiler primitive cannot be replaced by an external supply" (:2513); "x86 flags environment requires an admitted x86 target" (:2427); "whole-value lifecycle requires admitted target geometry" (:2125); "ordinary execution requires admission of retained scalar conditions" (:2689).

**Fail-open risks.**
- `resolve_entry` when `folded_constants.target` is `None`: no check that the installation's `target_machines` match anything (:2050). A program with no folded-constant target needs no closure, so this reads as intended; "traced" (the guard only matters when a target is recorded).
- `validate_layout_selection` returns `Ok` with no layouts when there are no layout applications and no targeted lifecycle plans (:2114-2128). Which wrong program: one that needs layouts but does not record applications. "open" (depends on the producer recording them).
- `compiler_owns_boundary_execution` (:2911) returns `false` for a boundary with no checked `declaration`, so such a boundary is classed as an importable host slot and `select_external_supply` can replace it. "open": I did not trace which producers leave `declaration` as `None`.
- `prepare_execution` skips the `scalar_entry_conditions` check whenever `scalar_application` is `Some` (:2729). The condition is then checked by `request_scalar_entry_checks` (:2790-2793, callee not in range). "stated" by structure only; callee unread.
- `admit_runtime_supplies` with `request.installation == None` starts from an empty supply set (:2407-2419). That is the ordinary "no providers" case; `execution_requirements` then reports missing providers (:2745). "traced".
- `ProgramDeclarations` lookups: see section 7.

---

## 9. `private_entry_target_tests` (:2916-3011, `#[cfg(test)]`)

**Purpose.** One `#[ignore]`d test that needs the environment variable `OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT` (:2921-2923). It loads real canonical output, admits a Linux x86-64 layout and private entry-RAM recipe, then asserts that (a) requirements with no installation fail (:2969-2973) and (b) with a macOS arm64 installation they fail with `TargetLayoutSelectionIdentityMismatch` (:3005-3008).

**Notes.** It hard-codes target names `"linux_x86_64"` and `"macos_arm64"` and layout facts (pointer size 8, case tag size 4) at :2943, :2975, :2949-2954. It reads a file outside the repository via an env var. It does not run by default (`#[ignore]`).

---

## Coverage

- Read in full: `embedding.rs` 1-3012 (3012 lines).
- Skimmed: none.
- Not read: none (0 lines).
- Grep counts (not full reads of other files): see each section. The files outside this range were not read; claims about them rest only on grep hits and are marked "suspected" or "open".

## Five things a maintainer most needs to know about this unit

1. `embedding.rs` is the only host entry to a loaded program. `TerminalProgram` (:2232) is an `Arc` over admitted storage, and `admit_runtime_supplies` (:2388) plus `select_external_supply` (:2491) are where a host import, provider or deferred component gets recorded as the supply for a boundary; `TerminalInstance::bind_import` (:1413) and `bind_component` (:1483) then check bindings against that record. Checks are on identities and exact signature equality, not names.
2. Binding logic for host adapters and nested components exists twice with different checks and error types: `TerminalComponentBinding` (:301-445) and `TerminalInstance` (:1364-1603). Notably only the instance version rejects `External` completion slots (:1421-1438), and only the instance version tolerates unbound `DeferredComponent` slots at invoke (:1612-1620).
3. The two scalar-application admission paths (`admitted_scalar_inputs` :2830, `scalar_application_inputs` :2869) differ on how strictly they compare `installation.supplies` (subset check vs exact equality, :2859-2862 vs :2890).
4. `compiler_owns_boundary_execution` (:2908) decides what counts as a host import (`imports()`, `import_declaration`, `select_external_supply`, and `engine_execution.rs:479`). A boundary with no checked `declaration` counts as not compiler-owned (:2911).
5. Post-return observation is deliberately narrow: `observe_*` and `returned_loan` expose only scalars, cases and byte views by canonical path, and component returns refuse `ScalarCase`/`ScalarArray` results (:732-737). The `architecture` test (:1040-1058) enforces structure by counting substrings in this file's own source.

## What I am unsure about

- How `requirements.imports` and `external_completions` are computed (`engine::capabilities::check`, outside this range), which decides whether the `None` branch in `TerminalComponentBinding::bind_import` (:392-397) can be reached with a boundary that should have a checked provider.
- Whether the loader guarantees that `catalogs.machines`, `boundary_machines`, `structural_types` and `provider_candidates` are sorted by identity, which `ProgramDeclarations::get` (:1920-1930) and `has_provider` (:2198-2200) assume.
- Whether `impl ProgramDeclaration for Block` (:1967) has any user.
- What `request_scalar_entry_checks` does with the entry conditions skipped at :2729.
- The `TerminalReturnedLoan` hits in `05_typed-trees-to-checked-trees` and `reference_result_custody.rs` (I did not read them; they may be name collisions).
