# Record: unit_078 (area 14-interpreter)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths below are relative to `source-rust/subsystems/interpreter/` unless they begin with `source-rust/`. "read" means I read the lines in this unit; "grepped" means a Grep run over `source-rust/` (counts given); "inferred" means reasoned from names or neighbouring code I did not read.

Unit contents (14 files, 16,413 lines): six files in `host/src/execution/host_imports/` and `host/src/execution/`, `host/src/lib.rs`, `host/src/runtime_boundary.rs`, four engine files in `instructions/` (calls, execution, structural, terminators), and `interpreter.rs`.

Terminology used here. A "place" is a numbered slot a machine uses to name a structural value (`PlaceId`). A "frame" is the saved state of a caller while a callee runs (`SuspendedCall`). "Custody" is the interpreter's runtime record of who owns a value (live claims and the affine frontier). `malformed()` is the interpreter's generic "the verified artifact is inconsistent" error; it records the call site's file and line (read: test at `instructions/execution.rs:4613-4619` asserts the report contains `engine_execution.rs:`).

---

## 1. `host/src/execution/host_imports/native_provider/service_reach.rs` (197 lines)

1. Purpose. Decides whether a native (foreign-code) import may be given service reach beyond its own trait. It allows that only when the host explicitly consents (`MatchingHost`) and every reached service, with its parents, belongs to the same declaring package as the import's owning service (header comment, `service_reach.rs:1-4`).
2. Shape. `admit_additional_services` (`:32`, `pub(super)`) returns `Result<(), String>`. Helpers: `package_owner` (`:12`), `identity_path` (`:18`, `pub(super)`), `describe` (`:22`). Constant `PACKAGE_OWNER = "package:"` (`:10`). Tests at `:87-196` (5 tests).
3. Inputs/outputs. Reads a `TerminalModule` (service table and identity text table), the owning service, the extra services, and a `TerminalNativeServiceDelegation`. Produces `Ok(())` or an error string naming the refused services. Mutates nothing.
4. How it decides. Keyed on source spelling of service identities. `package_owner` takes the identity text, splits at the first `::`, and accepts the left part only if it starts with `package:` (`:12-16`). Two services share an owner when those prefix strings are equal (`:71`). Display names come from the text after `::` (`:18-20`).
5. Cases by shape. None beyond the string shape `package:<hash>::<path>`; an identity without `::` or without the `package:` prefix has no owner and is refused (`:14-15`, `:54-63`).
6. Same job elsewhere. Not seen in this unit. Suspected: other consumers of service identity strings (not grepped for).
7. Unused/one-caller. `admit_additional_services`: grepped `admit_additional_services` over `source-rust/`: 3 matches = 1 production caller (`host_imports/native_provider.rs`) + definition + 1 test helper call. `identity_path`: grepped `identity_path\(`: only used inside this file (`:28`); `pub(super)` but no outside user.
8. Stated limits. Error text: "native-service delegation performs only reach declared by the import's own package" (`:82`).
9. Fail-open risks. None seen. Empty additional reach returns `Ok` (`:41-43`, trivially nothing to admit). A missing service row, a non-package owner, an unprovided delegation, or no owning service all return `Err` (`:44-63`, `:66-69`).

## 2. `host/src/execution/host_imports/native_provider/values.rs` (285 lines)

1. Purpose. Builds a codec that turns a record made only of scalar leaves (and 8-byte reference slots) into bytes for a native call and back. Byte positions come from the retained target layout; the semantic type declaration must agree on the complete leaf list and types (header `:1-4`).
2. Shape. `ScalarField` (`:17`), `ReferenceField` (`:24`), `StorageCodec` (`:31`: structural_type, extent, alignment, fields, references). Methods: `admit` (`:40`), `encode` (`:200`), `decode_fields` (`:242`), `aggregate` (`:270`).
3. Inputs/outputs. `admit` reads `TerminalProgram`, a slice of `NativeBorrowedStorageLayout`, a structural type id; uses `exact_layout`/`rejection` from `original_homes.rs`, `expand_coordinates` from `borrowed_storage::geometry`, and `record_fields::fields` (not read; in another file). Encode takes `TerminalBoundaryStorageValue` rows and returns bytes; `decode_fields` the inverse.
4. How it decides. By structural path equality between layout recipes and the semantic leaf list (`:105-113`), byte offset arithmetic and overlap (`:116-127`), reference slot position (`:153-170`), and count equality (`:178-190`). Not keyed on names.
5. Cases by shape. Quote: "native import needs a supported complete scalar record codec" (`:80`) when the shape is not `Record`, `FixedArray`, `PrimitiveScalar` or `Reference` (`:72-82`). Layouts with discriminators are refused (`:46-50`). Types with a quotient carrier are refused (`:54-56`). A reference slot must be exactly an 8-byte aligned opaque region (`:154-156`). `encode` and `aggregate` refuse any codec that has references (`:204-208`, `:274-278`).
6. Same job elsewhere. Scalar bytes conversion shares `scalar_codec.rs` (`encode`/`decode`/`scalar_extent`, `:7`). Field-and-offset checking of a C record is also done by `scalar_record.rs` (section 6) with a different fixed shape.
7. Unused/one-caller. `StorageCodec::admit`: grepped `StorageCodec::admit` over `source-rust/`: 8 call sites (`native_provider.rs:378,400,406,573`; `record_shape.rs:155,214,255`; `reference_outputs.rs:62`).
8. Stated limits. "native owned reference transport needs its exact value codec" (`:206`); "native owned reference result needs its exact value codec" (`:276`).
9. Fail-open risks. None seen. The `unwrap_or(usize::MAX)` at `:120` and `:160` turns an extent error into a forced overlap (refusal side); traced: earlier lines (`:115`, `:128-133`) already validated each stored scalar, so it cannot be reached with an error.

## 3. `host/src/execution/host_imports/native_word.rs` (116 lines)

1. Purpose. Checks that a native value is one integer word, possibly wrapped in single-field records at offset zero, and finds the path through the program's own record fields to the one scalar payload.
2. Shape. `NativeWordEnvelope` (`:9`: byte_size, alignment, contains_record). `native_word_envelope` (`:15`) walks the native signature graph. `scalar_payload_path` (`:62`) and recursive `scalar_payload` (`:72`) walk the semantic declaration.
3. Inputs/outputs. Reads `BoundarySignatureGeometry` shapes and fields, and `TerminalProgram` structural declarations. Returns an envelope or a path of `Field` segments, or an error string/`TerminalRuntimeError::InvalidProvision`.
4. How it decides. By shape class and byte geometry (`:33-35`, `:46-48`), by the count of non-erased fields (`:90-98`), and by field storage and atomic type (`:99-104`). Not keyed on names.
5. Cases by shape. Native side: only `Integer`, or `Record` with exactly one field (`field_count: 1`, `:38-41`); anything else is "native word envelope must end in an integer through single-field records" (`:53-55`). Semantic side: only a `PrimitiveScalar` equal to the wanted scalar, or a record with exactly one non-erased field (`:87-112`); atomic or encoded-storage fields refused.
6. Same job elsewhere. Used by five admission sites (below). `network_address.rs:220-226` repeats an extent-and-alignment check on top.
7. Unused/one-caller. Grepped `native_word_envelope|scalar_payload_path` over `source-rust/`: `native_word_envelope` 7 call sites (`absolute_value.rs:93,101`; `allocation/admission.rs:87,147,217`; `integer_absolute.rs:97`; `network_address.rs:221`); `scalar_payload_path` 5 (`absolute_value.rs:73`; `allocation/admission.rs:76,132,189`; `integer_absolute.rs:90`).
8. Stated limits. "native word payload cannot use atomic or encoded storage" (`:102`).
9. Fail-open risks. None seen. Erased fields are dropped from the count at `:90-93` (stated by the field's erased marker; erased fields carry no bytes). Loop bound `signature.shapes.len()` ends a cycle with an error (`:28`, `:59`).

## 4. `host/src/execution/host_imports/network_address.rs` (431 lines)

1. Purpose. Implements two C address functions, `inet_makeaddr` and `inet_lnaof`, directly in the host instead of calling libc. The C return value lives in an invocation-scoped store; the guest only holds a token (header `:1-13`).
2. Shape. `AddressOperation {Make, Host}` (`:35`); `OwnedAddress` (`:40`); `AddressContract` (`:46`); `AddressValues` (`:53`, a `Vec<Option<OwnedAddress>>`, slots never reused); `NetworkAddressImport` (`:81`) with `admit` (`:89`), `select` (`:109`), `validate` (`:128`); `TerminalImportAdapter` and `TerminalEffectHandler` impls (`:268`, `:274`); one test (`:378`).
3. Inputs/outputs. Reads a boundary declaration, a `NormalizedForeignCallBinding`, the calling application, and a shared `Arc<Mutex<AddressValues>>`. Produces a `TerminalEffectResult::Structural` minted token for `Make` and a `Scalar` u32 for `Host`.
4. How it decides. Selection is keyed on the normalized C symbol bytes `inet_makeaddr` / `inet_lnaof` plus platform library via `matches_platform_c_symbol` (`:109-126`; that function is in `host_imports.rs:215`, not read). Token identity is `u64::MAX - ordinal` (`:319-322`, `:353-354`). A stored occurrence must equal the admitted application commitment and multiplicity (`:69`); affine or linear occurrences are removed on use (`:73-75`).
5. Cases by shape. Only two operations. `Make` needs exactly two u32 scalars, no structural parameters, unqualified result, 4-byte aligned integer inputs (`:147-172`). `Host` needs one owned unqualified structural argument and a u32 result (`:183-198`). The carrier must be "a four-byte aligned C record ending in u32" (`:222-225`), with exactly one opaque application with non-zero commitments (`:227-235`).
6. Same job elsewhere. Native-signature classification and `admit_exact_foreign_call` call (`:236-260`) repeat the pattern in `scalar_record.rs:132-222`. Suspected (not read): `allocation/admission.rs`, `absolute_value.rs`.
7. Unused/one-caller. Grepped `NetworkAddressImport|AddressValues` over `source-rust/`: constructed once at `host_imports.rs:599`, store created once at `host_imports.rs:318`.
8. Stated limits. "native inet implementation is unavailable on this host" (`:325`, `:362`) when `HostInternetAddress::make`/`host_number` return `None`.
9. Fail-open risks. None seen. `AddressValues` grows with every `Make` and has no cap (`:318-332`); this is by design ("slots are never recycled", header `:6-8`), traced to avoid stale-token reuse.

## 5. `host/src/execution/host_imports/scalar_codec.rs` (260 lines)

1. Purpose. Converts logical scalar values to and from storage bytes using the producer's exact storage type. Never infers signedness from byte width (header `:1-5`).
2. Shape. `is_host_address_carrier` (`:20`), `boundary_scalar_extent` (`:24`), `scalar_extent` (`:31`), `closed_scalar_extent` (`:35`), `encode` (`:52`, returns `[u8;16]`), `decode` (`:93`), `integer_value_with_sign` (`:149`). Three tests (`:164-259`).
3. Inputs/outputs. Takes `TerminalScalarValue` plus storage and logical `ScalarType`; returns bytes or `TerminalEffectRejection`.
4. How it decides. On `ClosedScalarType` (concrete width) only; symbolic or parameterised widths fail the `try_from` at `:32`, `:56`, `:98-99`. Integer storage must be a whole number of bytes, up to 128 bits, not an address (`:40-47`). Logical width must be at least stored width (`:74`, `:124`). Address carrier: integer with `is_address()` and `bits() == Some(64)` (`:21`).
5. Cases by shape. Boolean, binary32, binary64, integer only (`:36-49`). Boolean decode accepts only `[0]` or `[1]` (`:104-108`). A logical type of a different kind than storage is refused (`:88`, `:145`).
6. Same job elsewhere. `values.rs` and (grepped, not read) `original_homes.rs`, `borrowed_storage.rs`, `provider_results.rs`, `native_provider.rs` call these functions.
7. Unused/one-caller. `is_host_address_carrier`: 3 callers outside this file (`native_provider.rs:172,746`; `original_homes.rs:843`) + own use at `:25`. `boundary_scalar_extent`: 2 callers (`native_provider.rs:463,538`) + test. Grepped `is_host_address_carrier|boundary_scalar_extent|scalar_codec::` over `source-rust/`.
8. Stated limits. Comment at `:17-19`: a host address "cannot reach interpreter storage or a record layout" because the codecs never admit it.
9. Fail-open risks. None seen.

## 6. `host/src/execution/host_imports/scalar_record.rs` (357 lines)

1. Purpose. Admits an import whose result is a C record of N ordinary 64-bit words (64-bit integers or `f64`), after checking the semantic type, the ordered native formals and the 8-byte field geometry (header `:1-3`).
2. Shape. `ScalarRecordAdmission<const FIELD_COUNT>` (`:17`), `admit_c_word_record` (`:23`), `word_class` (`:230`), `has_c_scalar_storage` (`:242`). Three tests (`:259-356`).
3. Inputs/outputs. Reads a `TerminalProgram`, declaration, binding, calling application, scalar parameter types, field types. Returns signature, structural type id and field ids.
4. How it decides. By structural shape (record, not quotient, not access view), by field types equal to the expected `ScalarType` (`:76-82`), by native shape geometry (size 8, align 8) and offset `ordinal * 8` (`:146`, `:191`), by the plan equality `application.plan.plan() != &binding.boundary_entry_plan` (`:96`), by target equality with the locator (`:98-100`).
5. Cases by shape. Fixed list: result must be a record whose multiplicity is `Unrestricted` or `Affine` (`:43-46`); records with `TargetLayout`, `AccessView` or `Accessor` storage refused (`:60-69`); field storage accepted only as none, `Scalar`, or `Direct` at the exact offset (`:248-255`); `Repeated` and `Fragmented` refused.
6. Same job elsewhere. Native-parameter and `CallSignature` classification (`:107-221`) repeat in `network_address.rs:236-260`.
7. Unused/one-caller. Grepped `admit_c_word_record` over `source-rust/`: 2 callers (`affine_transform.rs:50`, `integer_division.rs:37`). `word_class` and `has_c_scalar_storage` used only here.
8. Stated limits. "C word-record import requires its resolved target layout codecs" (`:62`).
9. Fail-open risks. `has_c_scalar_storage` returns `true` when a field has no storage row (`:249`). The wrong program that gets through is a packed-looking record whose field has no storage declaration but whose real position differs from `ordinal * 8`; the other `StructuralRecordStorage` variants (besides the three refused at `:60-69`) are not checked here. Classification: open (no comment at the site; I did not read the `StructuralRecordStorage` enum).

## 7. `host/src/execution/hosted_invocation.rs` (171 lines)

1. Purpose. The run loop for an installed program: validates provisions and requirements, builds the instance, binds imports, components and process-exit domain, then runs the interpreter in work-sized slices and collects the observation.
2. Shape. One function, `execute_installed` (`:12`), returning `TerminalRuntimeObservation` (stdout, stderr, effects, status, work usage, suspension counts, private entry RAM).
3. Inputs/outputs. Consumes `HostedTerminalProgram`, `TerminalRuntimeSupplies`, a quantum. Calls `program.entry_installed`, `execution_requirements_with_private_entry_ram`, `establish_host_entry`, `program.instantiate`, `HostContext::new`, `bind_instance`, `prepare_component`, `bind_component`, `TerminalProcessExitAdmission::admit_isolated_simulated_domain`, `instance.invoke`.
4. How it decides. Provisions' entry machine must equal the installed entry machine (`:34-41`). Any uninstalled provider slot is an error (`:51-59`). External completions need `domain_exit` not `Unprovided` (`:60-70`). The loop continues only on `SponsorExhausted` with budget left (`:143-153`).
5. Cases by shape. Imports exclude those taken by components (`:91-100`). None otherwise.
6. Same job elsewhere. Not seen in this unit.
7. Unused/one-caller. Grepped `execute_installed`: 2 call sites (`host/src/execution.rs:194`, `:215`).
8. Stated limits. None.
9. Fail-open risks. The entry-machine check is skipped when `request.provisions` is `None` (`:34-37`, `is_some_and`). Traced (inferred): with no provisions there are no component products (`:19-24` default empty) and no private entry RAM to disagree with; I did not read `establish_host_entry` for other uses of the entry.

## 8. `host/src/lib.rs` (47 lines)

1. Purpose. Crate root of `terminal-runtime`: `#![forbid(unsafe_code)]` (`:26`), declares `execution`, re-exports the public surface (`:34-47`).
2. Shape. Public `embedding` module with `HostContext`, `HostedTerminalProgram`, `bind_numeric_import_slot`, `establish_host_entry`, `execute_with_image_installation` (`:34-38`); `pub use` of delegations, request/observation types, `execute`, `execute_sliced`, `capture_host_x86_flags`, `run_native_provider_worker_if_requested`.
3. Inputs/outputs. Re-exports only.
4. How it decides. Not applicable.
5. Cases by shape. Not applicable.
6. Same job elsewhere. Not applicable.
7. Unused/one-caller. Not counted (re-exports).
8. Stated limits. Comment `:17` says the interpreter "lives in `psi/`"; the engine is at `source-rust/subsystems/interpreter/` (already in the crate map). Line `:36` is one long unwrapped line (read).
9. Fail-open risks. None seen.

## 9. `host/src/runtime_boundary.rs` (38 lines)

1. Purpose. Two `#[test]`s (compiled only under `cfg(test)`, `lib.rs:30-31`) that guard the crate's dependencies by reading source and manifests as text.
2. Shape. `runtime_borrows_the_produced_terminal_storage` (`:4`) asserts `execution.rs` contains `TerminalProgram::borrow(artifact)` and not `TerminalProgram::load(` or `semantic_bytes()`. `compilation_and_cli_do_not_own_runtime_integration` (`:12`) reads the compiler's `Cargo.toml`, the CLI's `Cargo.toml` and this crate's manifest.
3. Inputs/outputs. `include_str!` of five files.
4. How it decides. Keyed on source spelling: `str::contains`, `starts_with("compiler =")`, `contains("/pipeline/")`, and an exact line equality for `target-vocabulary` (`:25-31`).
5. Cases by shape. Only the one allowed `/pipeline/` dependency line is accepted (`:25-31`).
6. Same job elsewhere. Not seen.
7. Unused/one-caller. Test only.
8. Stated limits. None.
9. Fail-open risks. These are string tests: a dependency spelled with extra spaces or a different key form would not match `starts_with("compiler =")` (`:24`). Classification: open (inferred from reading the test).

## 10. `instructions/calls.rs` (3,958 lines)

File layout is a list of inline modules (`pub(crate) mod ...`) followed by `impl TerminalExecution` at top level (`:2783-3777`). Lines: `arguments` 9-127, `frames` 128-1137 (tests 844-1136), `boundary_calls` 1138-1159, `component_arguments` 1161-1565, `component_calls` 1566-1883, `component_contents` 1884-2372 (tests 2144-2371), `component_empty_arguments` 2373-2460, `component_scalar_arguments` 2461-2782, main impl 2783-3777, `family_actual_value` 3779-3803, `finite_family_tests` 3805-3958.

1. Purpose. Everything about calling: resolving call arguments, entering a callee by saving the caller's frame, the six call operations (unit, scalar, structural result, by machine value, dynamic dispatch, plain scalar call), boundary (host/provider/component) calls with effects, and passing owned values across a component boundary.
2. Shape.
   - `arguments`: `StructuralCallArguments` (`:21`: descriptor_parameters, values, aggregate_backings, byte_sequences, primitive_loans, scalar_arrays, element_views); `validate_carrier_partition` (`:35`); `resolve_call_arguments` (`:62`); `prepare_structural_call_arguments` (`:77`).
   - `frames`: `transferred_call_sources` (`:170`), `suspend_with` (`:207`), `bind_byte_sequence_arguments` (`:258`), `bind_element_view_arguments` (`:348`), `enter_call` (`:442`, the one place a frame is pushed for calls), `begin_unit_call` (`:587`), `begin_owned_cleanup_call` (`:611`), `begin_external_completion_call` (`:641`), `begin_no_normal_call` (`:667`), `begin_structural_scalar_call` (`:693`), `begin_structural_result_call` (`:718`).
   - `boundary_calls::completion_callee` (`:1143`).
   - `component_arguments`: `PreparedComponentArguments` (`:1178`), `ComponentInputCustody` (`:1186`), `ComponentCallScope` (`:1237`), `prepare_component_arguments` (`:1256`).
   - `component_calls::execute_component_call` (`:1582`).
   - `component_contents`: `capture_component_contents` (`:1922`), `ComponentContentsMapping` (`:1967`) with `rehome`, `rehome_path` (`:2010`), `map_identity` (`:2129`).
   - `component_empty_arguments`: `SharedEmptyArgument` (`:2381`). `component_scalar_arguments`: `PreparedScalarArgument` (`:2474`), `LentScalarArgument` (`:2491`).
   - Main impl: `PendingBoundaryCall` (`:2804`), `execute_call_unit` (`:2823`), `execute_call_structural_scalar` (`:2850`), `execute_select_implementation` (`:2871`), `execute_call_machine` (`:2894`), `execute_call_dynamic` (`:2954`), `execute_call_structural` (`:3047`), `execute_call_structural_with_scalar_arguments` (`:3073`), `execute_boundary_call` (`:3099`, about 410 lines), `boundary_effect_wait` (`:3512`), `complete_boundary_effect` (`:3559`), `execute_port_write` (`:3718`), `execute_call` (`:3743`).
3. Inputs/outputs. Reads `TerminalExecution` state (values, structural values, claims, affine frontier, supplies map, call stack) and `terminal_psi::Operation`. Mutates the same execution: pushes `SuspendedCall`, swaps in the callee's tables (`enter_call` `:560-583`), pushes `TerminalEffect`s, commits host results. Boundary calls talk to a `TerminalEffectHandler`.
4. How it decides.
   - Scalar-array vs opaque argument: `scalar_array_leaf_shape(...)` is `Some`, access `Owned`, no projected qualifications (`:93-106`).
   - Which call kind: the operation variant (`dispatched_operation!` macros) and the `OperationResult` variant (`:2924-2950`, `:3017-3043`).
   - Callee of `CallMachine`: the `Callable{contract, descriptor}` scalar, then catalog lookups by contract and descriptor (`:2903-2921`).
   - Dynamic dispatch: table index from the selector value; slot chosen by position, or when family actuals are present by scanning later slots for equal `requirement_identity` and equal `family_tuple` (`:2975-3013`).
   - Boundary call route: ordered cascade on the declaration's `execution` kind and the supply row in `boundary_supplies` (`:3124-3377`): empty-run release; static interrupt (refused, `:3133-3139`); access placement ops View/Initialize/MoveOut/Relocate (`:3140-3217`, plan found by `plan.boundary == boundary`, `:3145-3149`); `DeferredComponent` refused (`:3225-3232`); `ComponentProvider` (`:3233-3238`); `CheckedProvider` or provider present without a supply (`:3239-3370`); `execute_access_boundary` (`:3371`, in `memory/references.rs:1800`, not read); non-observation refusal (`:3379-3386`); else host effect.
   - Component calls: compares program identity, boundary id, and the supply row equal to the expected `ComponentProvider` (`:1616-1622`); completion provider/candidate/namespace (`:1685-1694`); type identity by TEXT equality of identity strings across two programs (`:1342-1354`, `:1743-1755`, and `map_identity` `:2129-2141`).
5. Cases by shape.
   - `bind_byte_sequence_arguments` accepts three forms then ends with `return Err(malformed())` (`:343`); last form only `BoundedOwned` carrier, owned, unrestricted (`:304-342`).
   - `execute_call` (plain scalar call, `:3743`): callee must have no structural parameters, no claims and a `Scalar` result, else `malformed` (`:3755-3761`).
   - Component call results: only `Unit`/`Unit`, `Scalar`/`Scalar` and `Structural`/`Structural`; everything else including `ScalarArray` and `ScalarCase` results hits `_ => reject("component completion disagrees...")` (`:1706-1861`).
   - Component inputs: access must be `Owned` or the reading-borrow pair (`:1326-1332`); projected qualifications must be empty (`:1334-1336`); a linear input needs exactly one whole claim (`:1385-1408`); a Unit lent argument kinds: scalar cells, shared empty record, or opaque host token (`:1466-1501`).
   - `component_contents::rehome_path`: `Case`, `Field`, `FixedIndex`; `Referent` refused ("component contents cannot cross a reference", `:2108-2112`); `Elements` contents refused (`:1986-1990`).
   - `SharedEmptyArgument` only for a record with zero fields (`:2386-2399`).
   - `family_actual_value` (`:3780`): integers and booleans only (`:3786-3799`).
6. Same job elsewhere.
   - Result dispatch `match operation.result {Unit, Scalar, Structural}` appears three times: `:2924-2950`, `:3017-3043`, `:3321-3368` (the third also adds profile capture).
   - `execute_call_structural` (`:3047`) and `execute_call_structural_with_scalar_arguments` (`:3073`) are the same except argument resolution.
   - Retiring a moved owned source: `retire_transferred_call_sources` (`:195-204`, removes silently after a precheck), component path (`:1649-1653`, `.expect(...)`), boundary result path (`:3668-3695`, errors on missing), `structural.rs:1700-1712` (skips if missing), `terminators.rs:105-128` (skips if missing). Five sites, different missing-value handling.
   - Capability walk in `execution.rs:241-730` re-derives which callees `CallMachine`, `CallDynamic`, `BoundaryCall` can reach (see section 11).
   - Suspected (not read): `boundary_effects.rs` and `embedding/instance` also bind arguments and supplies.
7. Unused/one-caller. `completion_callee`: grepped `completion_callee`: 2 callers (`instructions/execution.rs:612`, `instructions/terminators.rs:2859`). `execute_select_implementation`: called from the dispatch table at `execution.rs:3445` only (read). `PendingBoundaryCall`: used in `execution.rs:2214` and this file.
8. Stated limits. "component borrowed input needs its original materialized storage carrier" (`:1439`, `:2499`); "host reference results require an explicit captured-input interface" (`:3436`); "materialized host reference result has no invocation-owned public region" (`:3638`); "component contents cannot manufacture opaque backing or erased custody" (`:1940`); comment `:651-653` that the external-completion provider keeps its declared result and the saved continuation rejects any normal return.
9. Fail-open risks.
   - `bind_byte_sequence_arguments` `continue`s (skips binding) for borrowed-storage byte parameters with no reference ceiling (`:276-287`); `bind_element_view_arguments` does the same (`:368-378`). Wrong program: a borrowed byte or element view parameter that matches this test gets no binding row here. Classification: open (I did not trace where those bindings are created).
   - Component input that already forwards as a host atom and is owned adds no contents, scalar or empty carrier (`:1480-1500`: only `!forward_host_atom(value).is_none()` and `lent` push anything). Classification: open.
   - `execute_select_implementation` writes `Unsigned(table)` into the result integer type without checking the type admits it (`:2878-2890`). Classification: open.
   - `capabilities`-side counterpart in section 11.
   - `.expect("owned source was staged without parent mutation")` at `:1650-1652` panics instead of returning an error. Classification: open (a panic, not an accept).

## 11. `instructions/execution.rs` (4,839 lines)

Layout (inline modules): `atomic_operations` 10-200, `capabilities` 202-1906 (tests 732-1905), `content_transformations` 1908-2076, `execution` 2078-3609 (the engine state and loop), `native_code_layouts` 3611-3773, `scalar_operations` 3775-4839 with submodules `boolean_float` 3776-3914, `float_result_policy` 3915-4037, `integer` 4038-4621 (tests 4486-4620), `trapping` 4622-4716, and top-level helpers 4718-4775, test 4777-4838.

1. Purpose. The engine's state struct and dispatch loop; the pre-run capability check that walks the reachable call graph to find which imports, providers and external completions a program needs; atomic operations; native code layout records; scalar arithmetic, comparison, float policy and trapping.
2. Shape.
   - `TerminalExecution<'artifact>` (`:2139-2232`): about 70 fields (tables of values, structural values, byte/element/scalar-array bindings, claims, affine frontier, call stack, results, effects, failure). `SuspendedCall` (`:2234`), `LiveClaim` (`:2260`), `ResultClaimOrigin` (`:2293`), `SuspendedCallResult` (`:2340`), `CleanupContinuation` (`:2365`), `CompletedReturn` (`:2373`), `FailedReturn` (`:2395`). Macros `dispatch_operations!` (`:2401`), `operation_kind!` (`:2429`), `dispatched_operation!` (`:2439`), `frame_field_mut!` (`:2452`).
   - Entry points: `start_with_program` (`:2534`), `resume` (`:3162`), `resume_with_components` (`:3172`), `resume_operations` (`:3213`, the loop).
   - `capabilities::check` (`:241`) and `check_with_private_entry_ram` (`:248`); `TerminalUnsupportedCapability` (`:213`); `TerminalExecutionRequirements` (`:227`).
   - `execute_atomic_access` (`:23`), `content_parent_claims` (`:1917`), `execute_native_code_layout` (`:3639`), `FloatEvaluation` (`:3927`), `execute_float_result_policy` (`:3971`), `host_address_ordering` (`:4060`), integer executors (`:4154-4435`), `execute_trapping_integer` (`:4649`), `terminal_scalar_belongs_to_type` (`:4724`).
3. Inputs/outputs. `check` reads the admitted program catalogs and optional `TerminalRuntimeSupplies` and returns `TerminalExecutionRequirements` (imports, missing providers, external completions, scalar applications, scalar entry conditions). The loop reads the operation at `current_machine/current/next_operation`, charges the meter, dispatches, then runs the block terminator.
4. How it decides.
   - Dispatch loop: a match on `OperationKind` (variants named in `dispatch` table `:3475-3540` plus literal arms `:3310-3474`); the macro has no wildcard arm (`:2407-2415`), and the terminator `match` at `:3565-3592` lists every variant, so adding an enum variant forces an edit here (read).
   - Capability walk: per reached machine id and block id; operation kind; supply row for each boundary; `declaration.service_reach_is_installation_bound`; `program.has_provider(boundary)` (`:564-591`). Target check: `assembly_target_requirement.accepts_target_name(target)` where the target comes from the installation's `target_machines` or the program's `target_layouts` and both must agree (`:339-356`).
   - Register operations: Rflags read/write allowed only if an x86 flags environment is installed; `MemoryFence` only if the memory ordering model `supports(kind)`; all other register, task-register, segment and descriptor-table operations are refused with `UnsupportedCapability` (`:407-464`). The same list is repeated in the run loop (`:3366-3431`).
   - External completion: the smallest boundary id wins (`:540-542`) and is then refused (`:679-683`); demands from `Terminator::ExternalCompletion` are recorded when no checked provider answers (`:611-629`).
   - Access boundaries without a supply are removed from imports/missing providers only if the origin analysis resolves every such call (`:652-678`).
   - Atomic access: access taken from the arrival parameter or from the producing operation (`:32-43`); `WriteOnlyBorrow` or illegal ordering refused (`:44-46`).
   - Integer ops: result type from the operation's declared result; operand types must equal it (or the exact-add/subtract operand rule `:4376-4383`); host addresses compare only with host addresses or null (`:4060-4088`).
   - `FloatResultPolicy`: `Trapping` crashes if `apply_trapping_policy` errors (`:3988-3993`); `Saturating` replaces infinity with max finite when the `eligible` value is true (`:3996-4031`).
5. Cases by shape.
   - `execute_native_code_layout` expects a record of exactly `[entry: integer, regions: element view]`, whose element is a record of exactly `[base, length]` integers; quotes: `let [entry, regions] = fields.as_slice() else { return Err(malformed()) }` (`:3669-3671`) and `let [base, length]` (`:3692-3694`). Result must be affine, no claims or qualifications (`:3655-3663`).
   - `projected_empty_host_value` (`:2784`): path of `Field` segments only, record with packed storage, zero fields, no cleanup (`:2789-2816`).
   - Atomic read-modify-write: five operations (`:191-197`), integer only, not address (`:188`).
   - `execute_trapping_integer`: Add, Subtract, Multiply, Divide, Remainder, ShiftLeft, ShiftRight, Convert (`:4663-4701`).
   - Dynamic dispatch walk in `capabilities` follows the declared slot only: `table.rows.get(slot_index)` (`:517-527`).
   - `settle`: `ExternalCompletionKind` has one kind, handled as `match kind { ProcessExit => {} }` (`terminators.rs:2910`).
6. Same job elsewhere.
   - Register-operation refusal list: `execution.rs:429-464` and `:3396-3431` (same operations, two copies).
   - Reachability of callees: `capabilities::check` (`:372-598`) vs the call executors in `calls.rs`; for dynamic calls with family actuals the runtime picks a later slot (`calls.rs:2990-3012`) but the walk only follows `slot` (`execution.rs:505-529`). Inferred from reading both; I did not read how family slots are ordered in the catalog.
   - Staging an aggregate into storage (`ConstructorStorage::new` ... `stage_structural_result_contents` ... `publish` ... `commit_cursor`): `native_code_layouts` (`:3711-3758`), `structural.rs:144-155` and `:253-263`.
   - `store_scalar` (`:4475`) is the shared result writer used by most scalar executors.
   - Handler names do not match their scope: `execute_exact_integer_add` handles add, subtract, multiply, divide, remainder in exact, wrapping and saturating forms (`:4355-4429`, dispatch `:3534`); `execute_integer_bitwise_and` handles or/xor (`:4272`); `execute_wrapping_integer_shift_left` handles all four shifts (`:4307`).
7. Unused/one-caller. Grepped over `source-rust/`:
   - `structural_primitive_values` (`pub`, `:3099`): 0 call sites outside its own definition and its delegate (`final_structural_primitive_values`, `:3105`, also 0 other callers).
   - `live_affine_frontier()` (`pub`, `:3121`): grepped `\.live_affine_frontier\(\)`: 0 matches.
   - `assumed_bindings()` on `TerminalExecution` (`pub`, `:3018`): grepped `\.assumed_bindings\(\)`: matches belong to other types; I could not separate execution callers, so not counted.
   - `capabilities::check` (`:241`): 1 caller (`embedding/program.rs:3228`); `check_with_private_entry_ram`: 2 callers (`program.rs:3239`, `:3720`) + the wrapper.
   - `terminal_scalar_belongs_to_type`: grepped `terminal_scalar_belongs_to_type\(`: 27 matches = 1 definition + 26 call sites in 7 other files (`boundary_effects.rs` 2, `structural.rs` 2, `bytes_and_views.rs` 3, `custody.rs` 1, `primitives.rs` 12, `records_and_arrays.rs` 5, `references.rs` 1).
   - `frame_field_mut!`: grepped: 6 uses in 4 files (`boundary_inputs.rs` 1, `boundary_storage_inputs.rs` 2, `bytes_and_views.rs` 1, `primitives.rs` 2); none in this unit.
   - `unsigned_integer_value` (`:3048`): 5 callers (all `bytes_and_views.rs`).
   - `FloatEvaluation::CompileTime`: grepped: constructed in 6 files outside tests (Build `activation.rs:476`, `host.rs:102`, `scalars/execution.rs:159`; native stage 01 `execution.rs:145`; `embedding/program.rs:3600`; `numeric_imports.rs:230,436`).
8. Stated limits. `ScalarEntryQualificationUnsupported` for entry machines whose scalar parameters have qualifications (`:2587-2593`); "an unfinished program must arm every pending normal completion before running" (`:3156`); "assembly catalog target requirement is not established by the target environment" (`:353`); "non-copy replacement ..."/"moving an aggregate storage interpretation" are in `structural.rs`. Comment `:3571-3573`: the `EliminateEmpty` terminator "signals an invalid runtime input or engine invariant".
9. Fail-open risks.
   - `RegisterAccess` write of Rflags: `if let Some(Integer{Unsigned(bits)}) = self.values.get(&value)` does nothing when the value is absent or not an unsigned integer, and `u64::try_from(*bits).unwrap_or(u64::MAX)` saturates (`:3391-3393`). Wrong program: an Rflags write of a missing or wrong-typed value is silently accepted and does not change flags. Classification: open.
   - `OperationKind::AtomicFence` is `OperationFlow::Advance` with no effect (`:3462-3464`). Classification: open (no comment; interpreter runs one thread, inferred).
   - `capabilities::check` for `CallMachine`: when `checked_operation(...)` returns `Ok(None)` the loop does `continue`, so the callee of that call is not explored and its imports are not demanded (`:473-486`). Wrong program: a call the callable analysis does not consider reached under the entry's roots. Not accepted silently at run time (a missing provider later fails in `execute_boundary_call`, `calls.rs:3282-3288`), but the requirement list would under-report. Classification: open (did not read `callable_values`).
   - `CallDynamic` walk follows the declared slot only (see item 6). Classification: open.
   - `terminal_scalar_belongs_to_type` returns `true` for every `Callable` value without checking its contract or descriptor (`:4726`); `scalar_value` (`:3031-3044`) does check them. Wrong value: a callable with a bad descriptor stored through a path that skips `scalar_value`. Classification: open.
   - `closed_scalar` bypasses the retained-descriptor lookup for `Fixture` storage under `cfg(test)` only (`:2487-2496`; stated: "never enter production").

## 12. `instructions/structural.rs` (2,587 lines)

1. Purpose. Executors for operations that build, copy, move and read structured values: records, sum cases, zeroed values, locals, byte-sequence literals, field stores and moves, leaf copies, field reads, host reference results.
2. Shape (all inside `impl TerminalExecution`, line numbers of the first line): `execute_establish_scalar_case` (`:86`), `publish_aggregate` (`:122`), `publish_host_reference_result` (`:160`), `stage_host_referent` (`:214`, recursive), `establish_structural_case` (`:462`), `prepare_subtree_retirement` (`:598`), `prune_subtree` (`:612`), `execute_establish_byte_sequence_literal` (`:646`), `execute_establish_zeroed_structural` (`:712`), `materialize_zeroed_leaves` (`:768`), `execute_establish_structural_local` (`:943`), `execute_establish_trivial_affine_local` (`:971`), `execute_establish_record` (`:1005`, calls `establish_record` defined in `memory/records_and_arrays.rs:2537`), `execute_structural_scalar_field_store` (`:1014`), `execute_move_structural_value` (`:1163`), `execute_decompose_structural_value` (`:1269`), `execute_move_structural_field` (`:1305`), `execute_store_structural_field` (`:1373`, about 350 lines), `structural_window_requirements` (`:1720`), `structural_window_hole` (`:1771`), `transplant_subtree` (`:1905`), `execute_structural_leaf_copy` (`:1926`), `copy_scalar_array_payload` (`:2241`), `byte_carrier_leaf_copy` (`:2261`), `execute_structural_case_membership` (`:2334`), `observed_field_cell(_in)` (`:2343`, `:2353`), `execute_boolean_structural_field` (`:2402`), `execute_scalar_structural_field` (`:2412`), `read_observed_field` (`:2422`), `execute_byte_sequence_read` (`:2516`), `execute_byte_sequence_length` (`:2554`). Private `CheckedReferenceScalar` (`:45`) and `ResolvedStructuralWindow` (`:78`).
3. Inputs/outputs. Reads the operation, live structural values, claims, affine frontier, program type declarations. Writes `structural_values`, `structural_scalar_fields`, `structural_cases`, byte/element/scalar-array tables, `live_claims`, `live_affine_frontier`, `retired_structural_homes` via `ConstructorStorage` staging then `publish`.
4. How it decides. By place identity and declared multiplicity/access of the producing operation or arrival parameter (`machine.structural_parameter`, `machine.producing_operation`; e.g. `:1047-1096`, `:1179-1199`, `:1395-1409`); by structural type shape (`is_copyable_carrier` `:1975`, `scalar_array_leaf_shape` `:1993`); by case id and field id. Not by names.
5. Cases by shape.
   - Host reference payload: `Aggregate`, `Resident`, `Zeroed` only; resident supply must be a shared borrow with no nested references (`:271-275`). Comment `:165-167`: "Resident completion currently establishes one plain referent."
   - `materialize_zeroed_leaves` handles nine `ZeroedStructuralLeaf` variants (`:798-938`); a callable-typed leaf has no zero (`:776`, `malformed`).
   - `byte_carrier_leaf_copy` accepts Boolean and Integer members only (`:2284-2294`, `_ => malformed`).
   - `execute_move_structural_field`: aggregate-storage views refused ("moving an aggregate storage interpretation", `:1324`). `execute_store_structural_field`: "non-copy replacement of an aggregate storage interpretation" (`:1531`).
   - `execute_structural_leaf_copy`: an affine case-payload copy that is not a copyable carrier returns `AffineProjectionNotRepresentable` ("requires its owned frontier", `:1976-1983`).
   - `snapshot`/bounded byte copy: only `BoundedOwned` byte carrier with `Unrestricted` multiplicity (`:1931-1973`).
6. Same job elsewhere. Staging an aggregate: `publish_aggregate` (`:139-157`), `stage_host_referent` Aggregate arm (`:252-264`), `execution.rs:3711-3758`. The zero-value closure `zero` (`:774-789`) is a per-scalar-type zero; suspected duplicate elsewhere (not grepped). Source-retirement `if let Some(retired) = ... remove` at `:1701-1708` is one of the five sites listed in section 10.
7. Unused/one-caller. `execute_*` methods are reached from the dispatch table in `execution.rs:3475-3540` (read; one caller each). `projected_empty_host_value`/`retain_host_structural_payloads` (in `execution.rs`) are used by `embedding/program.rs` (grepped: `program.rs:937`, `:3819`, test file `storage_observation/tests.rs:1441`, `execution.rs:2932`). `snapshot_scalar_case` (`terminators.rs:2372`): 2 callers (`terminators.rs:2308`, `call_observations.rs:216`).
8. Stated limits. "The declaration owns validity on every observation of foreign storage" (`:43-44`); the quotes at item 5.
9. Fail-open risks.
   - `execute_establish_byte_sequence_literal` accepts a second execution of the same literal when the existing value matches in identity, type, path, qualifications and bytes (`:667-679`). Stated: repeated literals keep their home (test `calls.rs:919`, `repeated_literals_keep_their_home_but_nested_calls_get_distinct_storage`).
   - `execute_store_structural_field` silently skips retiring the moved source when `structural_values.remove(&value.place)` is `None` (`:1701`). Traced: scalar-array and byte values live in other tables and are removed at `:1709-1711`.
   - `execute_structural_leaf_copy` falls through to the general leaf-copy path when the scalar-array/primitive fast path finds a type mismatch (`:2033-2080`, no `else`). Classification: open (the later checks at `:2116-2127` refuse a type mismatch unless shared-descriptor/quotient).
   - Otherwise none seen.

## 13. `instructions/terminators.rs` (2,930 lines)

Layout: `block_bindings` 10-1500 (tests 1183-1499), `completion` 1501-1940 (tests 1797-1939), `structural_return` 1941-2422, top-level `impl TerminalExecution` 2446-2930.

1. Purpose. What happens at the end of a block: jump, conditional, structural case switch, return (scalar, unit, structural), crash, no-normal invoke, external completion. Includes simultaneous binding of the successor block's parameters, running cleanup actions (affine discards and owned cleanups), and installing return values into the caller's frame.
2. Shape.
   - `BlockBindings` (`:37`) with `commit` (`:100`) and `prepare_cleanup_actions` (`:181`); `present_arrival_memberships` (`:397`); `prepare_block_bindings` (`:431-1052`, about 620 lines); `borrowed_argument_source` (`:1054`); `block_parameter_is_declared` (`:1106`); `owned_source_for` (`:1128`); `install_presented_descriptor` (`:1146`); `owned_block_source` (`:1174`).
   - `completion`: `reject_no_normal_return` (`:1508`), `continue_cleanup_actions` (`:1529`), `finish_return` (`:1629`), `install_return` (`:1693`).
   - `structural_return`: `settle_return_structural` (`:1949`, about 420 lines), `snapshot_scalar_case` (`:2372`).
   - Top level: `SelectedSuccessor` (`:2446`), `settle_invoke_no_normal` (`:2455`), `settle_jump` (`:2511`), `settle_successor(_with_scalars)` (`:2540`, `:2550`), `settle_conditional` (`:2612`), `settle_structural_case` (`:2643`), `settle_return` (`:2726`), `settle_return_unit` (`:2762`), `settle_crash` (`:2793`), `settle_external_completion` (`:2826`).
3. Inputs/outputs. Reads the terminator and the live state; computes a `BlockBindings` (new scalar values, structural values, claims, backings) before changing anything, charges the meter for the edge, then commits via `continue_cleanup_actions`. Returns `TerminatorFlow::Continue` or `Yield(status)`. Failed returns and cleanups are parked in `failed_return` / `failed_call_result` so storage stays owned.
4. How it decides.
   - Successor arguments are classified one by one in a fixed order inside `prepare_block_bindings`: primitive loan (non-owned primitive scalar, `:478-500`); scalar array loan (`:501-522`); byte sequence loan (`:523-555`); packed scalar array (`:556-585`); then by `parameter.access`: `Owned` whole or projected (`:589-711`), view-descriptor reading borrow (`:712-732`), `borrowed_storage_parameter` (`:733-754`), shared borrow of borrowed byte carrier only (`:755-772`), mutable borrow of borrowed byte carrier only (`:773-784`), `_ => malformed` (`:785`).
   - Qualification change on an edge: binary search in the sorted catalog `structural_qualification_changes` on `(machine, edge, argument_ordinal)` (`:404-427`); exactly one row, with matching source and destination places, no path (`:416-424`).
   - Claim re-homing: each live claim on the argument place moves to the parameter place; with `block_entry_claims` declared, a claim must match an active binding path and multiplicity (`:934-960`).
   - `settle_structural_case`: edge chosen by `successor.case == active_case` (`:2683-2686`); each `Case` segment of the path is checked against the stored case first (`:2672-2678`).
   - `settle_crash`: `frontier_lower_bound` must equal exactly the sorted live claim keys (`:2808-2810`). `settle_external_completion`: bound must be strictly increasing and a subset of live claims (`:2846-2853`); exactly one scalar argument; return code must be a signed 32-bit integer (`:2896-2909`).
   - Structural return: signature checks (type, multiplicity, qualifications, projected qualifications) against the call frame's expected result (`:1983-2019`), a large `borrowed_descriptor` boolean (`:2122-2174`), carried frontier equality (`:2217-2235`), claim rebinding (`:2236-2263`).
5. Cases by shape.
   - The order list in item 4 is the case table. The `borrowed_descriptor` return is allowed only for an Affine signature with no qualifications whose source is a `MutableBorrow` view-descriptor parameter and whose backing is a mutable element view or mutable borrowed byte view (`:2122-2174`).
   - `snapshot_scalar_case` applies only to a `Sum` type with a stored case whose payload fields are all non-erased scalars and no qualifications; else it returns `None` and the general structural result is used (`:2379-2399`).
   - Quote: "A freshly formed shared reference can transfer its affine loan into a copyable result." (`:2185-2187`).
   - `settle_return` is scalar only (`:2741-2743`), `settle_return_unit` unit only (`:2775-2777`).
6. Same job elsewhere.
   - Block argument capture vs call argument capture: `prepare_block_bindings` (`:431`) and `prepare_structural_call_arguments` (`calls.rs:77`) share `bind_byte_sequence_arguments`/`bind_element_view_arguments` (`terminators.rs:819-841`, `:975-981`) but each does its own classification of loan kinds.
   - `install_return` checks `NoNormal` and `ExternalCompletion` twice: early returns at `:1699-1710` make the second pair of arms at `:1716-1727` unreachable (read).
   - Frontier handling at crash vs external completion differs (equality at `:2808`, subset at `:2848-2853`).
   - Retiring a moved owned source: see section 10 item 6 (`commit` `:105-128`, return `:2347-2356`, cleanup `:1540-1575`).
7. Unused/one-caller. `owned_source_for` is `pub(crate)`; grepped `owned_source_for` was not run separately (used 5 times inside this file, read). `reject_no_normal_return`: 3 callers in this file (`:2731`, `:2767`, `:1954`). `snapshot_scalar_case`: 2 callers (above). The env var `OMEGA_STATE_REFERENCE_DIAGNOSTICS` is read once here (`:899`) and prints on a failed argument bind only; grepped `OMEGA_TERMINAL_TRACE|OMEGA_STATE_REFERENCE_DIAGNOSTICS` shows one read each in the interpreter (`execution.rs:2471`, `terminators.rs:899`).
8. Stated limits. The `PendingNormalCompletion` terminator is refused in `execution.rs:3566-3569` ("an unfinished callee reached its missing result without a checked continuation"); in this file: the `StructuralReturn*` errors, and the comment at `:792-796` that "the verified module already admitted the change, its issuer and its proof" for explicit casts.
9. Fail-open risks.
   - `prepare_cleanup_actions` skips the runtime-index selection for actions other than `DiscardResidual` and `InvokeOwned` (`:192-196`); traced: other actions (`DiscardRoot`, `RetireHome`) carry no path in this code.
   - The structural-return claim check runs only when `structural_return` is `Some` (`:366`); all other callers pass `None` by design.
   - `present_arrival_memberships` leaves the value unchanged when no change row exists (`:412-428`); stated by the doc comment `:393-396` ("ordinary transfers retain the source's memberships").
   - None otherwise seen.

## 14. `interpreter.rs` (197 lines)

1. Purpose. Crate root of `terminal_interpreter`: module wiring by `#[path]` and the public re-export list.
2. Shape. 21 `#[path]` modules (`:34-70`); three alias modules `boundary` (`:72`), `engine` (`:81`), `storage` (`:88`) that rename flat modules; `pub use` list (`:99-179`); `TerminalExecution::start_artifact` (`:182`).
3. Inputs/outputs. `start_artifact` loads bytes with `TerminalProgram::load`, then `start_execution(..., None)`.
4. How it decides. Not applicable.
5. Cases by shape. Not applicable.
6. Same job elsewhere. Not applicable.
7. Unused/one-caller. `start_artifact` (`pub`): grepped `start_artifact\(` over the whole worktree: 1 definition in `source-rust/` and 0 callers there; the callers are in `_recycling/` (`scalar_applications.rs:2261`, tests). Counted by that grep (output cut at 40 lines but only 13 hits listed).
8. Stated limits. Comment `:12-19` says the engine lives "in `psi/`"; it lives here (already recorded in the crate map).
9. Fail-open risks. None seen.

---

## Coverage

Read in full (line counts):
- `host/src/execution/host_imports/native_provider/service_reach.rs` 1-197 (197)
- `host/src/execution/host_imports/native_provider/values.rs` 1-285 (285)
- `host/src/execution/host_imports/native_word.rs` 1-116 (116)
- `host/src/execution/host_imports/network_address.rs` 1-431 (431)
- `host/src/execution/host_imports/scalar_codec.rs` 1-260 (260)
- `host/src/execution/host_imports/scalar_record.rs` 1-357 (357)
- `host/src/execution/hosted_invocation.rs` 1-171 (171)
- `host/src/lib.rs` 1-47 (47)
- `host/src/runtime_boundary.rs` 1-38 (38)
- `instructions/calls.rs` 1-3958 (3,958)
- `instructions/execution.rs` 1-4839 (4,839)
- `instructions/structural.rs` 1-2587 (2,587)
- `instructions/terminators.rs` 1-2930 (2,930)
- `interpreter.rs` 1-197 (197)

Total read in full: 16,413 lines. Skimmed: none. Not read: none. Test modules were read in full but described briefly; I did not check what each test asserts beyond the names and the assertions quoted above.

Not read (outside the unit, cited above as "not read"): `host_imports.rs` (`matches_platform_c_symbol`, `admit_exact_foreign_call`, `call_operands`, `refusal`), `record_fields.rs`, `original_homes.rs`, `memory/references.rs` (`execute_access_boundary`), `memory/records_and_arrays.rs` (`establish_record`), `memory/custody.rs`.

## Five things a maintainer most needs to know about this unit

1. The engine loop (`execution.rs:3213-3603`) is a macro-driven `match` over every `OperationKind` and every `Terminator` with no wildcard arm (`:2407-2415`, `:3565-3592`). Adding an operation or terminator variant forces edits here, in the capability walk (`:372-598`), and often in `calls.rs`/`structural.rs`.
2. The pre-run capability walk (`execution.rs:241-730`) and the run-time call code (`calls.rs`) are two separate implementations of "which callees can this call reach". The walk follows the declared slot of a dynamic call only (`:505-529`), while the runtime may select a later slot by family tuple (`calls.rs:2990-3012`), and the register-operation refusal list is duplicated (`execution.rs:429-464` and `:3396-3431`).
3. Almost every executor is a check-then-commit transaction: it builds a staged `ConstructorStorage` / `BlockBindings` / `PreparedComponentArguments`, validates, then publishes. Failures park the half-built result in `failed_return` or `failed_call_result` so accounted storage is not lost (`calls.rs:207-245`, `terminators.rs:1629-1691`).
4. Cross-program type identity for components is decided by comparing identity TEXT (`calls.rs:1342-1354`, `:1743-1755`, `:2129-2141`) and for native service reach by the `package:` string prefix (`service_reach.rs:12-16`). Nothing here uses a numeric id across two programs.
5. In `host/`, native imports are admitted by fixed shape: one 4-byte word record (`network_address.rs`, `native_word.rs`), a record of N 8-byte words (`scalar_record.rs`), or a scalar-leaf record with a layout (`values.rs`). Anything outside those shapes is refused with a specific message; each admission also recomputes the native call signature and calls `admit_exact_foreign_call`.

## What you are unsure about

- Whether `capabilities::check` under-reports needs for `CallDynamic` with family actuals and for `CallMachine` when `checked_operation` returns `None` (`execution.rs:473-486`, `:505-529`). I did not read `terminal_semantics::callable_values` or the dynamic dispatch catalog ordering.
- Where bindings for borrowed-storage byte and element-view parameters are created when `bind_byte_sequence_arguments`/`bind_element_view_arguments` `continue` past them (`calls.rs:276-287`, `:368-378`).
- Whether the missing-storage case in `has_c_scalar_storage` (`scalar_record.rs:249`) is safe for the other `StructuralRecordStorage` variants.
- Which platform libraries `matches_platform_c_symbol` accepts for Linux (the call passes macOS and Windows libraries only, `network_address.rs:116-121`); the function is in `host_imports.rs:215`, not read.
- The `assumed_bindings()` method on `TerminalExecution` (`execution.rs:3018`): the grep matches many other types, so I have no call-site count for it.
