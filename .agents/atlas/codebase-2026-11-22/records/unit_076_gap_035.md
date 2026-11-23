# Record: unit 076, gap 035 (second pass), area 14-interpreter

File: `source-rust/subsystems/interpreter/embedding/program.rs`, lines 2130-4059 (1930 lines), all read in full.
Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. The range starts in the middle of `TerminalEntryCalls` construction inside the inline `instance` module (which runs to line 2705) and ends with the inline `program` module (2706-4059). Types defined before 2130 (`TerminalInstance`, `EntryCallWatch` at 1507) are only referenced here.

## 1. Tail of `mod instance`, lines 2130-2705

### 1a. End of `TerminalEntryCalls` construction (2130-2146)
- Purpose: finishes building a set of alternative suspended-call paths. If the paths disagree on a projected suffix it returns an error; otherwise it keeps the positions where the flags say "omitted" (2133-2137) or the single path's `projected` (2139). Returns `TerminalEntryCalls { paths, by_leaf, projected }` (2141).
- Decides on: the per-position `omitted` flag, keyed on program counter `pc` (2135). Read.

### 1b. Entry-call watching (2148-2196)
- `check_selected_entry` (2148): refuses a `TerminalEntryCall` whose `program` identity or `entry` machine differs from the instance's (2152). Keyed on identity, `execution_identity()`.
- `invoke_until_entry_call` (2163) and `invoke_until_entry_calls` (2181): build an `EntryCallWatch` (type at 1507, not read here) and call `invoke_to`. The plural form checks only `calls.paths[0]` (2186), so it assumes every path shares one entry. Not checked against the other paths in this range (inferred; line 2081 calls the same check per path during construction).
- Callers: `invoke_until_entry_call` 17 matching lines, 2 files; `invoke_until_entry_calls` 5 lines, 2 files (grep `-rn --include=*.rs` over `source-rust`, includes the definitions and doc text).

### 1c. Call-stack observers (2198-2274)
- `caller_sites` (2200): iterator over active callers, immediate caller outward, yielding `(MachineId, CallSite)`. Cases by shape, match on `SuspendedCallResult` (2209-2229): `NoNormal` and `ExternalCompletion` give `CallSite::Edge`; `OwnedCleanups` is skipped (2214, comment: "cleanup has no authored call operation", 2199); `Scalar`, `Unit`, `Structural` look up the operation at `next_operation - 1` and keep it only if `declared_invocation()` is some (2226). A missing machine, block or operation silently drops that caller via `?` inside `filter_map` (2218-2225). Callers outside the file: 1 (`build/evaluation/activation.rs:912`).
- `completed_call_path_index` (2235): returns the index half of `completed_entry_call`. Outside callers: 2 files, 6 lines.
- `completed_call_stack` (2241): returns `Vec<PrefixCall>` for each suspended caller plus the leaf recorded in `completed_entry_call` (2244, 2272). Errors as `String` if no completed suspension, a closed instance, a caller with no return position, or a caller not pointing past a call (`declared_invocation().is_some()`, 2263). Used by `continue_checked_entry` (2288) and elsewhere (12 lines, 2 files).

### 1d. `continue_checked_entry` (2279-2395)
- Purpose: moves a parked entry frame from the `previous` instance into the `successor` instance (compiled from an extended program), so a paused Build resumes in the new code. Takes the actual registers, owned storage and reference descriptors (comment 2276-2278).
- Judgments, all refuse by returning `Err(String)`:
  - 2286-2302: `prefix.accepts(previous.module, successor.module)`, the previous call stack equals `prefix.stack()` sites, both entries match the prefix's first site machines, and neither instance is closed; the successor must not have been invoked.
  - 2311-2318: both frames' accounts must be `same_invocation`; a missing account on either side also refuses (`is_some_and` over the zipped options).
  - 2319-2333: the frame must sit at `(old.machine, old.block, old.index + 1)` with no pending effect, failure, failed return or failed call result, no returned byte sequence/element view/claims.
  - 2334-2373: a long list of "holds execution authority that requires a separate handoff" conditions. If either frame has external bindings or demands, target layouts, scalar geometry, native code layouts, call observations (frame only), scalar applications, scalar entry checks, entry results, assumed bindings, x86 flags, memory ordering, a storage resolver, private entry RAM, provider storage (allocations, residents, descriptors, completed profiles) or `!activation_storage.can_continue_entry()`, it refuses. This is a fixed list of shapes the resume path supports: a plain entry with no host authority. Note call observations are checked on `frame` only (2344), whereas the other fields check both frames.
  - Then `component_binding::continue_bindings` (2374), `frame.relocate_continuation_storage` (2381), and it swaps `frame.program` and `boundary_supplies` for the successor's (2386-2387), sets the resume position `next.index + 1` (2390).
- Keyed on: machine, block and operation identities from `prefix`, not display names. Read.
- Stated limit: error text at 2370 "requires a separate handoff".
- Fail-open risks: none seen. The checks refuse by default; the only silent acceptance is `frame.call_observations` not being checked on `entry` (2344), classified "open": a successor carrying `call_observations` would have them dropped when `frame.program = entry.program` and the successor's `execution` is replaced by `frame` (2392). Whether `entry.call_observations` can be set before `invoke` was not traced.

### 1e. `invoke_to` (2397-2473)
- Purpose: the single place an instance runs: refuses closed instances, pending completions, an unguarded projection, unbound imports and unbound external completions, then calls `execution.resume_with_components` with an `ImportDispatch` (2446-2455).
- Judgments:
  - 2410-2420: an active projection must be repeated (same `Arc` or equal) by the next watch, otherwise it refuses. Message: "a projected invocation must keep guarding its omitted suffixes until a checked successor".
  - 2422-2433: unbound imports are those without a binding, except boundaries whose supply is `AdmittedBoundarySupply::DeferredComponent` (2428), which are allowed to stay unbound (keyed on the supply kind in `boundary_supplies`).
  - 2434-2437: unbound external completions refuse.
  - 2461-2471: after the run, `call_observations` are finished on `Complete`, left in place on `None` or `SponsorExhausted`, and dropped for every other result (`_ => ... = None`, 2469).
- Same job elsewhere: `close` (2550) and `is_finished` (2540) rely on the same `execution.is_finished()`.
- Fail-open risks: 2469 drops observations on any non-complete status without recording why. Effect: observations vanish instead of being reported; that does not accept a program, classified "traced" as non-acceptance. Otherwise none seen.

### 1f. Small accessors (2475-2567)
`observe_private_entry_ram` (2475), `bind_provider_storage` (2485, returns the input back inside a rejection when closed, reason "provider storage cannot bind a closed instance"), `take_returned_provider_storage` (2498), `active_component_invocation_count` (2508), `effects` (2515, empty slice when no execution), `result` (2522), `observed_entry_result` (2528), `is_finished` (2540: `is_none_or`, so a closed instance with no execution reports finished), `is_closed` (2546), `close` (2550: refuses with `Busy` when invoked and unfinished, else drops execution, bindings and components).
- Outside callers (grep, lines/files): `take_returned_provider_storage` 3 lines, 1 file outside (`memory/providers.rs:232` is the lower-level method); `active_component_invocation_count` 2 lines (def and `host/src/execution/hosted_invocation.rs:147`); `observe_private_entry_ram` 4 lines, 2 files; `observed_entry_result` 10 lines, 2 files.

### 1g. `ImportDispatch` (2570-2704)
- Purpose: adapter that routes engine effect requests to the import adapter bound to the effect's boundary. Fields: `bindings: &mut BTreeMap<BoundaryMachineId, TerminalImportBinding>` and an optional `storage_resolver` (2570-2573).
- `adapter_at` (2576) errors "boundary call reached an unbound import slot" when no binding; `adapter_for` (2589) accepts only `TerminalEffect::BoundaryCall`; `ComponentCall`, `MemoryFence`, `PortWrite` all return rejections with their own messages (2595-2603). Keyed on effect variant and `site.boundary` id.
- `impl TerminalEffectHandler` (2608-2704): forwards every handler method to the adapter. Not forwarded to an adapter: `resolve_provider_storage` returns `Ok(None)` when no resolver is supplied (2615), which means "no storage supplied" and not acceptance; `accepts_reference_results` returns `false` with no binding (2632-2636); `storage_requirements` returns the default (empty) requirements with no binding (2643-2644). Classified "traced" as fail-closed: missing binding yields no extra capability. `prepare_original_storage` errors with no binding (2661-2665).
- Not forwarded: nothing in this impl is a duplicate of the other, but `handle_effect*` has four near-identical forwards (2669-2703), a pattern repeat of one job (suspected from reading, no other site grepped).

## 2. `mod program`, lines 2706-4059

### 2a. `declarations` (2712-2817)
- `ProgramDeclaration` trait (2718) gives each declaration kind an `Identity`; `ProgramDeclarations<'storage, D>` (2723) is a sorted slice plus a sorted `replacements` slice. `get` (2746) binary-searches replacements first, then declarations; `values` (2770) yields each base row replaced by its replacement when one has the same identity; `declaration` (2763) returns `malformed()` when absent. Impls for `TerminalMachine`, `Block`, `StructuralTypeDeclaration`, `BoundaryMachineDeclaration` (2790-2816).
- How it decides: by identity, requiring sorted-by-identity slices (`binary_search_by`). Assumed, not checked, that the rows are sorted (inferred; no check seen in this range).
- Cases by shape: `values` replaces only rows that already exist in `declarations`; a replacement with an identity absent from `declarations` is reachable via `get` but not via `values` (2777-2782 vs 2750-2755). Read.
- Fail-open: none seen.

### 2b. `AdmittedTerminalProgram` and `ProgramStorage` (2829-2919)
- `AdmittedTerminalProgram` (2829): `identity: TerminalExecutionIdentity`, `catalogs: Arc<ProgramStorage>`, `target_layouts: Option<Arc<AdmittedTargetLayouts>>`, `scalar_geometry: Option<Arc<[StructuralTypeDeclaration]>>`.
- `ProgramStorage` (2837): `Borrowed(view)`, `Owned(ClosedTerminalModule)`, `Retained(RetainedTerminalCode)`, plus `#[cfg(test)] Fixture(TerminalModule)` (comment 2840-2841: "Existing engine fixtures intentionally edit unsealed rows"). `Deref` to `TerminalModule` (2847).
- `pending_normal_completions` (2862): `Owned` and `Fixture` return an empty slice (2871, 2873) while `Borrowed` and `Retained` return the view's list. So a loaded-by-bytes program (`load`, `load_envelope`) reports no pending normal completions. Classified "open": no comment says why; whether an owned module can carry them was not traced.
- `closed_types` (2877) and `closed_module` (2891): `Fixture` errors; `Borrowed` and `Retained` `closed_module` error with "an unfinished program has no closed module" when the view has none.
- `fixture_module_mut` (2923, test only).

### 2c. Entry resolution and layout validation (2932-3011)
- `resolve_entry` (2932): refuses a supply installation whose `code_identity` differs (2936). If the catalogue declares a folded-constant `target` (2940), it requires an installation whose `folded_constant_target` equals it and whose `target_machines` target equals it, else `FoldedConstantClosure` errors (2943-2955). Picks root from `installation.entry`, else `catalogs.entry`, else `VerifiedEntryMachineMissing` (2957-2960); resolves via `psi_target_layouts::TerminalTargetContext::resolve_entry` (2963) and confirms the machine exists (2975).
- `validate_layout_selection` (2981): if layouts are supplied, their Terminal Psi identity and target must equal the target machines' and the program's (2988-2993). With no layouts it refuses if the catalogue has any `target_layout_applications` (2996) or any access lifecycle plan with `target: Some` (3000-3008, message "whole-value lifecycle requires admitted target geometry").
- Keyed on identity values and target names (string compare `Some(target)` at 2942, 2949: target name strings). Read.
- Fail-open: `resolve_entry` with `installation = None` and no folded-constant target passes straight through (2940 `if let` falls through). This is intended for artifacts without a target-closed constant; refuse happens only when the catalogue declares one. Classified "stated" by the surrounding error text ("missing or incompatible realization closure").

### 2d. Lookup helpers (3013-3096)
`machines` (3013), `machine` (3017, `malformed()`), `verified_machine` (3024, `VerifiedCallTargetMissing`), `verified_boundary_machine` (3033), `structural_types` (3042: base catalogue with replacements from `scalar_geometry`, else from `target_layouts.resolved_records()`, else none), `access_lifecycle_plans` (3055: from target layouts when present, else the catalogue), `structural_declaration` (3065), `boundary_machines` (3072), `has_provider` (3078: binary search of `provider_candidates` by boundary), `import_declaration` (3085: none when a provider exists, else only `BoundaryDeclarationRole::Callable` and not interpreter-owned).
- Grep counts (outside this file): `.verified_machine(` 45 lines in 11 other files plus this one; `.verified_boundary_machine(` 10 lines, 3 files outside; `.structural_declaration(` 81 lines, 13 files outside; `.access_lifecycle_plans()` 5 lines, 2 files outside. `fn verified_machine` appears once (this file).
- Same job elsewhere: `machine` (3017) and `verified_machine` (3024) are the same lookup with two different errors; both used. Suspected: `import_declaration` (3085) and `imports` (3475-3484) apply the same predicate (Callable, not interpreter-owned, no provider), written twice in this file.

### 2e. `TerminalProgram`, `TerminalProgramEntry` (3099-3163)
- `TerminalProgram { program: Arc<AdmittedTerminalProgram> }` (3100). `TerminalProgramEntry` (3113): `machine`, `scalar_parameters`, `structural_parameters`, `access_view_inputs`, `result`, `requires`.
- `unconditional_precondition` (3129 area, body ends 3130: calls `precondition_with(&[])`) and `precondition_with` (3135): evaluates each `requires` clause with `closed_predicates::closed_guard` and a `ClosedIntegerEvaluator`; `Some(true)` continues, `Some(false)` returns `Some(false)` at once, `None` marks undecided; returns `Some(true)` only when none is undecided, else `None` (3153). Clauses in `established` are skipped by value equality (3142). Judgment keyed on the proposition value (`established.contains(clause)`).
- Fail-open: skipping `established` clauses accepts whatever the caller claims. Classified "stated": comment 3133-3134 "The caller has already established these exact propositions". Whether callers pass only truly established facts was not traced (the only outside caller of `unconditional_precondition` is `build/evaluation/activation.rs:235`, which passes none; `precondition_with` has no caller outside this file: grep `precondition_with` found 2 lines, both in this file).
- `access_view_inputs` (3156): rows of the program-wide table filtered to this entry's machine.
- `unconditional_precondition` doc (3123-3128, partly read through 3129): host invoking an entry owes every clause.

### 2f. Program loading and admission (3165-3394)
- `load` (3247): `ClosedTerminalModule::from_semantic_bytes`, identity `portable`. `borrow` (3259), `retain` (3271), `load_envelope` (3280, via `CanonicalTerminalArtifact::from_bytes`). All set `target_layouts: None`, `scalar_geometry: None`.
- `entry` (3182), `entry_installed` (3186), `describe_entry` (3194), `structural_type` (3213), `execution_requirements` (3220) and `..._with_private_entry_ram` (3230) call `validate_layout_selection` then `engine::capabilities::check*`.
- `admit_runtime_supplies` (3302): refusals in order: installation identity mismatch (3307), target machines identity mismatch (3312), x86 flags without an x86 target (3334; `accepts_target_name`), memory ordering without a matching target (3344), native code layouts without target, for another program/target, or naming an unknown machine (3353-3368), then layout validation and entry resolution (3376-3378), then each host import and external completion goes through `select_external_supply` (3379-3392). Keyed on identity values, and target names (string).
- Same job as 1d: both check identity before moving authority.
- `defer_component` (3396): same as above with `DeferredComponent`; refuses on identity mismatch.
- `select_external_supply` (3412): refuses when the boundary is undeclared (3426), not `Callable` (3427), compiler-owned except `NumericPrimitive` with `HostImport` (3439-3446, comment 3436-3438: "A component or checked provider may not replace that declaration"), completion mismatch (3447-3460: `ExternalCompletion` needs `BoundaryCompletion::External`, others need `NormalReturn`), or a duplicate/competing selected supply or assumed binding (3461-3470). Keyed on boundary id and declared completion kind.
- `imports` (3475), `import` (3486), `executes_boundary_internally` (3490), `external_completion_requirement` (3497).
- Callers: `executes_boundary_internally` 2 lines (def and `host/src/execution/host_imports.rs:100`); `external_completion_requirement` 7 lines, 6 files; `defer_component` 2 lines outside this file at `build/evaluation/activation.rs:469`; `load_envelope` has no caller outside this file other than the test at 3952 and the README mention (`README.md:89`): grep `load_envelope` over `source-rust` and `tools` returns the definition and one test call. `TerminalProgram::retain` 7 lines, 4 files outside this one.

### 2g. `instantiate_compiler_evaluation` (3526-3606)
- Purpose: starts the one compiler invocation admitted for this artifact and target. Every failure collapses into one error "compiler invocation requires its admitted artifact, entry, installation and target" (3533-3539, `refusal`).
- Judgments: needs an installation; storage must be `Borrowed` or `Retained` (3541-3545, `Owned` is refused, so a `load`ed program cannot start compile-time evaluation); the entry root must exist; `admitted.require_invocation` and `installation.require_compiler_evaluation` must pass. With an issued layout: target name equal to `facts.profile` (string, 3554), non-empty `indices`, installed layouts' `target_facts` equal to `facts`, and each indexed structural parameter must be owned, `Opaque`, with declaration identity text exactly `"toolchain::LayoutTarget"` (3579-3585, a source-spelling compare), matching the supplied input's type and not duplicated (3586-3587). Sets `float_evaluation = CompileTime` (3600).
- Cases by shape: the only structural-argument kind accepted is an opaque owned `toolchain::LayoutTarget` (3585).
- Fail-open risks: none seen. Out-of-range `position` refuses via `ok_or_else(refusal)` (3569, 3574).

### 2h. Execution preparation (3614-3823)
- `start_execution` (3614, `pub(crate)`) builds a `TerminalInstantiation` with defaults and calls `prepare_execution`.
- `execution_program` (3636): returns the shared program unless the installation has target layouts, in which case it makes a new `AdmittedTerminalProgram` carrying `target_layouts` and, if scalars are admitted, their geometry.
- `prepare_execution` (3653): refuses in this order: mixing provisional `scalar_application` and completed `scalar_admission` (3674); ordinary execution with retained scalar conditions but no admission (3691-3705); an account whose measurement is not `TerminalPsi` (3706-3715); layout selection; the capability check with private entry RAM (3720); any required scalar application without an evaluated value (3727-3740); any entry condition (when not a provisional scalar request) without a value (3741-3756); the first missing provider (3757); structural initialization validation (3762). Then converts boolean field inputs into scalar field values (3764-3774), starts the engine (3775), rewrites a scalar-field error into the boolean-field error if boolean fields were supplied (3786-3801), and requests scalar entry checks or entry results (3803-3811), binds byte arrays and host payloads, and records `external_demands` (3813-3821).
- Fail-open: `execution.scalar_applications = scalar_values` is set from `None` when neither input was supplied (3812); that is only reached when no retained conditions exist (3691), so a program without conditions passes. Classified "traced".
- `TerminalInstantiation` (3827, `#[derive(Default)]`): `float_evaluation`, `scalar_arguments`, `structural_inputs`, `installation`, `account`, `entry_results`, `scalar_application`, `scalar_admission`. `TerminalScalarApplicationRequest` (3839).

### 2i. Scalar input admission (3843-3915)
- `admitted_scalar_inputs` (3844): requires an installation; if the entry machine has a `declaration`, the primary admission must `admits_machine`; compares program identity, `target_machines`, `provider_selections`, `target_layouts`, `assumed_bindings` and each expected supply against the installation (3866-3877); else returns a single "completed scalar inputs differ" error. Keyed on identity and structural equality of the selections.
- `scalar_application_inputs` (3882): the provisional counterpart. Compares the same fields against `observations` (3897-3902) and requires that the application's entry equals the resolved entry and that its value is not yet evaluated (`.is_some()` refuses, 3907-3911).
- Same job: the two functions check the same five fields with slightly different comparisons (3869-3875 versus 3898-3902): `admitted_scalar_inputs` checks each expected supply is present in the installation (installation may hold extra supplies); `scalar_application_inputs` requires `installation.supplies == expected_providers.supplies` (exact equality). Read.

### 2j. `interpreter_owns_boundary_execution` (3918-3928)
- Returns true when the declared callable's `execution` is neither `Provider` nor `NumericPrimitive`; a boundary with no `declaration` returns false (`is_some_and`, 3921). Effect: an undeclared boundary is treated as not interpreter-owned, so it counts as a host import in `import_declaration` (3092-3095) and `imports` (3479-3483) if its role is Callable. Classified "traced" as consistent with `select_external_supply`, which rejects any boundary that lacks a declared callable only through `compiler_owned()` of `None` (3439: `execution.is_some_and`, so `None` passes).
- Callers: this function is `pub(crate)`; used at 3094, 3481, 3494 (grep within the file).

### 2k. Tests (3930-4058)
- `envelope_load_tests` (3931): one test, `loading_an_envelope_keeps_identity_with_its_owned_semantics` (3935), builds an artifact, loads it with `load_envelope`, drops the bytes, asserts identity and identity text survive.
- `private_entry_target_tests` (3965): one `#[ignore]` test (3969), needing env var `OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT` from a real public CLI output. Checks the private-entry-RAM recipe is refused with no installation and with another target's layouts (`TargetLayoutSelectionIdentityMismatch`, 4052). The `macos_arm64` and `linux_x86_64` layout facts are hard-coded in the test (3994, 4021).

## Stated limits (under 20 words each)
- 2199: "cleanup has no authored call operation".
- 2370: "requires a separate handoff".
- 2840-2841: "Existing engine fixtures intentionally edit unsealed rows."
- 3436-3438: "A component or checked provider may not replace that declaration."
- 3969: `#[ignore]` "explicit qualification consumes OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT".

## Coverage
- Read in full: `source-rust/subsystems/interpreter/embedding/program.rs` lines 2130-4059 (1930 lines).
- Skimmed: none. Not read: none.
- Counts came from `grep -rn --include=*.rs` and `grep -rln` over `source-rust`, run once per name; a "lines" figure counts matching lines including definitions and doc comments, not distinct call sites.

## Five things a maintainer most needs to know about this range
1. `continue_checked_entry` (2279) resumes a paused Build in a new program only for a bare entry frame: a long refuse-if-present list (2334-2368) rules out any host authority (layouts, providers, RAM, bindings, observations). `call_observations` is checked on the old frame only (2344).
2. `invoke_to` (2397) is the single run path for an instance. Unbound imports are tolerated only when their supply is `DeferredComponent` (2428), and call observations are dropped on any status other than complete, exhausted, or unfinished (2469).
3. `instantiate_compiler_evaluation` (3526) accepts only `Borrowed` or `Retained` program storage and collapses every refusal into one message (3533). The layout-target argument is recognised by the text `"toolchain::LayoutTarget"` (3585).
4. Declaration lookup (2746) depends on sorted slices and a replacement slice (target layouts or scalar geometry); `structural_types` (3042) is where replaced rows enter.
5. `select_external_supply` (3412) is the admission rule for host imports, external completions and deferred components: compiler primitives cannot be replaced, except numeric primitives as host imports (3439-3446); completion kind must match the supply kind (3447-3460).

## What I am unsure about
- Whether `ProgramStorage::Owned` returning an empty `pending_normal_completions` (2871) is right for every owned module; no comment says so and I did not read `terminal_codec`.
- Whether `EntryCallWatch`, `TerminalEntryCall` and `TerminalEntryCalls` constructors (before 2130) guarantee all paths in a set share one entry, which `invoke_until_entry_calls` assumes (2186).
- Whether `entry.call_observations` can be set on a successor before `continue_checked_entry` (2344).
- Whether the declaration slices are sorted by identity; no check appears in this range.
