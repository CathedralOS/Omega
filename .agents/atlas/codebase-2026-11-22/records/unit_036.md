# Record: unit_036 (area 13-native-targets-driver)

Root `P/` = `source-rust/subsystems/compiler/psi-to-native/verification/`. Crates: `numeric-intrinsic-admission` (`P/numeric-intrinsics/`), `terminal-authority-receipts` (`P/provider-admission/authority/`), `provider-stack-contribution` (`P/provider-admission/same-stack/`), `provider-admission` (`P/provider-admission/src/`). All 59 files read in full. Counts come from Grep count mode over `source-rust/` unless said otherwise.

Word key: "identity text" = a string looked up through a `TerminalIdentityTable`/`ProviderIdentityTable` (`identities.text(index)`), compared with `==`.

---------------------------------------------------------------------------------------------------
## A. numeric-intrinsic-admission (5 files)

### A1 `numeric-intrinsics/catalog.rs` (267 lines)
1. Purpose: maps a selected numeric identity (`CompilerIntrinsicExecutionIdentity`) to a `NearestIeeeFloatArithmeticRealization` (operation + IEEE format). It is the one table the admission side and target lowering share for numeric builtins.
2. Shape: `catalog_realization(identity, boundary_format) -> Option<..>` (catalog.rs:161); `format_bearing_builtin(BuiltinFunction)` (:40) is a 46-arm match over directed-rounding add/sub/mul/div/sqrt/fma, classify, fused and multiply-then-add (:58-156); helpers `nearest_arithmetic` etc. (:5-38).
3. In/out: reads the identity enum from `native_intrinsics`, `numeric_literals::FloatFormat`, `target_vocabulary` op enums; returns an owned realization with `x86_scalar_fma: None` always (:36, :265).
4. Decides on: enum identity (`BuiltinFunction` variant, conversion source/target/domain). Integer carrier chosen by searching `Carrier::ALL` for equal signedness and `bits == integer.width()` (:227-230), so width is the key.
5. Cases by shape: `_ => return None` at :157 (any builtin not in the 46), at :202 (builtins other than min/max/sqrt/isnan/isinf/isfinite/isnormal/issubnormal), at :257 (conversions not float-format change, int-to-float exact, float-to-int exact/saturating/trapping), at :260 (identities not Primitive-float-binary/Builtin/NamedNegation/NamedConversion; this includes `PrimitiveIntegerComparison`, read; it is named in the caller `compiler_builtins/catalog.rs:99` and therefore resolves to `None` here). Comment :226: "Execution admits only an exact carrier in the target operation catalog."
6. Same job elsewhere: `terminal_authority_policy/classification.rs:187-303` enumerates the same builtin set for authority classes (different table, same enum). Suspected, not compared line by line.
7. Callers: no grep run for `catalog_realization`. Read uses: `compiler_builtins/catalog.rs:102`, `signature.rs:44`, `tests.rs:59`.
8. Stated limits: none quoted beyond :226.
9. Fail-open: none seen (unmatched returns `None`, which callers turn into refusal: `signature.rs:45`, `compiler_builtins.rs:376`).

### A2 `numeric-intrinsics/direct.rs` (122 lines)
1. Purpose: admits compiler-owned numeric primitives that constant evaluation reaches (not only the runtime entry) against the same per-target float table used for native lowering.
2. Shape: `direct_numeric_target(module, boundary, target: &str)` (direct.rs:7); `AdmittedTerminalIntrinsics::with_direct_numeric(self, artifact, demanded, profile, x86_fma)` (:35); `deployed_fma` (:85); one unit test (:105).
3. In/out: reads boundary declarations from the Terminal module, `TargetProfile::canonical_float_realization(requirement)`; mutates `self.operations` (pushes or replaces `(slot, operation)`), returns the realization.
4. Decides on: target by name string (`from_canonical_target_name(target)` :14), the requirement identity text of the boundary (`module.identities.text(boundary.identity)` :16-20) used as the table key. A selected operation that already exists is kept only when `format` and `operation` equal (:70).
5. Cases by shape: only boundaries whose declaration execution is `NumericPrimitive` are processed; others `continue` (:50-54). FMA needs `FloatIsaAdmission::X86AvxFma3` (:61-68).
6. Same job: `provider-admission/compiler_builtins.rs:310-331` and `admit_x86_fma_features` (:385-426) check x86 FMA admission for runtime-reached builtins; same rule, separate code (both compare `provider.profile()` and `admits(requirement, slot)`; direct.rs:97 vs compiler_builtins.rs:413).
7. Callers: `grep 'direct_numeric_target|with_direct_numeric'` = 7 hits in 5 files, 3 inside direct.rs; outside: `00_.../scalar_applications.rs` 1, `01_.../provider_rows.rs` 1, `numeric-intrinsics/admission.rs` 1, `provider-admission/lib.rs` 1 (re-export).
8. Stated limits: none.
9. Fail-open: `continue` at :50-54 skips non-numeric-primitive demands. Traced: the skipped demand is not a numeric one by declaration; a wrong program would need a boundary mislabelled non-numeric, which is not checked here. Classification: open.

### A3 `numeric-intrinsics/selection.rs` (129 lines)
1. Purpose: checks that each selected external provider row that the module actually retains has an exact declaring owner and a retained external implementation.
2. Shape: `validate_external_provider_owners(module, selected)` (selection.rs:11); `validate_retained_provider_owners(external_providers, identities, facts, retained)` (:46, `pub(crate)`).
3. In/out: reads `module.interfaces.boundary_machines`, `boundary_application_demands`, `external_providers`, `SelectedProviderPlanFacts`; returns `Result<(), String>`; no mutation.
4. Decides on identity text: the retained set is `BTreeSet<&str>` of boundary identity texts plus demand overloads (:15-37); a row "requires a declaration" if `facts.selected_row_matches_requirement(requirement_text, row_text, report_fingerprint)` for any retained text (:52-60); owner compare `identities.text(provider.declaration.identity()) != selected.provider_declaration` (:112).
5. Cases: rows whose binding is not `CompilerIntrinsic` are skipped unless `realization.is_some()` (error :68-69, else `continue` :71). Comment :5-10: "A selected external realization needs its exact declaring owner only where the module retains the requirement".
6. Same job: `provider-correspondence` `matches_external_selection` / `selected_sealed_native_provider` (imported :1) does the matching; this file only demands it.
7. Callers: `validate_external_provider_owners` re-exported (`compiler_builtins.rs:14`, `lib.rs:41`); I did not run a grep for it.
8. Stated limits: comment :5-10 (demand-only owner check).
9. Fail-open: :118 `if realization.is_some() && !found && requires_declaration(...)`: a row with a realization but no matching external implementation passes when no retained requirement text matches the row. Stated (comment :5-10: "a selected row nothing retains is never realized"); test `tests.rs:99-122` pins it.

### A4 `numeric-intrinsics/signature.rs` (108 lines)
1. Purpose: checks a numeric boundary declaration against its catalog realization and the selected service schema.
2. Shape: `boundary_contract_matches_selected_schema(module, boundary, plan, requirement) -> bool` (signature.rs:2); `numeric_boundary_realization` (:36, `pub(crate)`); `numeric_boundary_execution` (:47); `float_arithmetic_signature(realization, boundary, module)` (:58).
3. In/out: reads provider plan rows/schema methods, boundary completion/termination, `terminal_semantics::numeric_primitives::validate_boundary`; returns bool or `Result`.
4. Decides on identity text (`plan.identities.text(row.requirement_identity) == requirement` :13) and exact-one-row/one-method counts (:14, :27-33). Classification result must be a conventional float class layout for `Classify` (:73).
5. Cases: `Classify` has a special structural-result shape (:63-88); everything else must have a Scalar result (:89).
6. Same job: `provider-admission/settlements.rs:202` and `compiler_builtins.rs:118` call `boundary_contract_matches_selected_schema`; one function used by both. Grep `boundary_contract_matches_selected_schema` = 20 hits in 5 files (14 in `compiler_builtins.rs` mostly the test, 2 in `settlements.rs`, 2 in `windows_handle_contracts.rs`, 1 here, 1 in `admission.rs`).
7. Unused: none seen.
8. Stated limits: none.
9. Fail-open: `result_matches` is `true` when `result.qualifications.is_empty()` (:92-93), so an unqualified scalar result is accepted on shape alone. Traced: the following `signature_matches` (:102) still checks parameter and result scalar types.

### A5 `numeric-intrinsics/tests.rs` (163 lines)
Four tests: slot custody per artifact identity (:8), numeric meaning reconcile (:27), owner demanded only for retained requirements (:100), published boundary required for a demanded row (:125). Reads `admit_terminal_intrinsics` from the crate's `admission.rs` (not in this unit).

---------------------------------------------------------------------------------------------------
## B. terminal-authority-receipts (2 files)

### B1 `authority/mechanisms.rs` (256 lines)
1. Purpose: role-tagged identities of physical operations and their canonical byte encoding.
2. Shape: `TerminalMechanismIdentity` enum (mechanisms.rs:139: CompilerIntrinsic, NormalizedForeign, CheckedPhysical, Syscall, Table); `NormalizedForeignTerminalMechanismIdentity` (:13: target, locator digest, implementation contract, argument contract); `SyscallTerminalMechanismIdentity` (:78); `CheckedPhysicalTerminalMechanismIdentity` (:116; only `PortWrite{port}`); `terminal_mechanism_identity_bytes` (:177) with tag bytes 0 intrinsic, 1 foreign, 2 physical, 3 syscall, 4 table.
3. In/out: pure data; bytes feed leaf ordering and digests (`receipt.rs:460`).
4. Keyed on: target profile identity string (`target.identity().as_str().as_bytes()` :186,203,224,242), locator digest, plan commitment, port number, syscall number.
5. Cases: `CheckedPhysicalOperationIdentity` has one variant (`PortWrite`) (:111-113).
6. Same job: none.
7. Callers: `terminal_mechanism_identity_bytes` used in `receipt.rs`, `terminal_authority_policy.rs`; grep not run.
8-9. Stated limits: none. Fail-open: none seen. One `expect("target-profile identity length fits u32")` (:208,:229,:247).

### B2 `authority/receipt.rs` (837 lines)
1. Purpose: builds and validates a receipt of the authority leaves reviewed for one program/target, with a digest identity. A leaf records custody, mechanism, origin, exercised classes and permitted classes.
2. Shape: `TerminalAuthorityClosureLeaf` (receipt.rs:115) with constructors `new` (:124), `native_binding` (:150), `program_root` (:176) and private `validate` (:193); `TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves` (:340); `TerminalAuthorityClosureCustody` (:96: SelectedRequirement, NativeBinding, ProgramRoot); `TerminalAuthorityClosureLeafOrigin` (:63); `TerminalAuthorityClosureReviewBuildError` (:429); digest fn `terminal_authority_closure_review_identity` (:539, domain `omega.terminal-authority.closure-review.v6`); policy identity structs (:17, :40).
3. In/out: takes sorted leaves + identity table, returns receipt. Mutates nothing external.
4. Decides on: identity indices and texts (empty/absent requirement -> `EmptyRequirement` :226-229, :364-369); `exercised`/`permitted` axes (`permitted.first_missing_class(exercised)` :254); sort key and duplicate key `same_closure_leaf_key` (:488-537: schema, requirement index, plan or receipt, origin).
5. Cases by shape: `CheckedBoundary` origin must pair with a `NormalizedForeign` mechanism carrying a nonzero checked argument contract and a non-`InvokeNoNormal` site (:194-207); `CheckedPhysical` mechanism iff `CheckedPhysical` origin (:208-216); `CheckedIntrinsic` origin must pair with `CompilerIntrinsic` mechanism (:217-225); ProgramRoot needs a checked site, an exercised set and no permitted set (:231-244).
6. Same job elsewhere: the two policy identity structs (:17-37 and :40-60) are copies of each other (same fields and methods).
7. Callers: the receipt types are consumed by `reviewer.rs`, `admission.rs`, `native-artifact`, `behavior-exclusions` (crate map claim); not grepped here.
8. Stated limits: test comment :797 "The published v5 digest" while the digest domain string says `v6` (:549): a naming mismatch; read.
9. Fail-open: `unwrap_or_default()` on identity text in the digest (:586, :600, :607) hashes an empty string for a missing identity; reached only after `from_reviewed_leaves` already refused empty texts (:364-369). Traced: not reachable through `from_reviewed_leaves`. Classification: traced.

### B3-B4 not in this unit (the crate has two files; both above).

---------------------------------------------------------------------------------------------------
## C. provider-stack-contribution (2 files)

### C1 `same-stack/src/lib.rs` (223 lines)
1. Purpose: binds an opaque provider's caller-stack contribution (bytes, alignment) to its selected plan, requirement and admission receipt.
2. Shape: `admit_same_stack_contribution(candidate, selected_report_identity, selected_commitment, selected_requirement)` (lib.rs:131); `AdmittedSameStackContribution` (:86); `SameStackContributionAdmissionCandidate` (:73); macro-made ids `normalized_id!`/`commitment_digest!` (:19-61).
3. In/out: pure; returns the sealed record with an FNV-1a `report_identity` (:193) and a domain digest `commitment` (:208, `omega.task.same-stack-contribution.v1`).
4. Decides on: equality of report identity (u64), plan commitment digest, `TerminalIdentityIndex` requirement, and a power-of-two alignment (:171). Zero identity/commitment refused (:137-146).
5. Cases: no bound on `bytes` (zero bytes accepted; test `tests.rs:16`).
6. Same job: none.
7. Callers: grep not run (crate map says 9 dependents).
8. Stated limits: field comment :74 "Historical compact report coordinate; not admission authority." yet it is compared at :147 and folded into the digest (:213).
9. Fail-open: none seen.

### C2 `same-stack/src/tests.rs` (92 lines)
Three tests: zero-byte leaf keeps contribution with pinned FNV and digest values (:16), changed plan/requirement refused (:51), zero receipt and bad alignment refused (:74).

---------------------------------------------------------------------------------------------------
## D. provider-admission `src/` (51 files, listed in order)

Crate entry: `lib.rs` (84 lines) declares modules and re-exports. Private modules: `access_lifecycles, adapters, admission, admission_source, callback_entries, compiler_builtins, lowering_facts, model, native_binding_settlements, settlements, source_evaluated_bindings, static_interrupts, task_activations, terminal_authority_permission_policy, terminal_authority_policy, terminal_authority_review`; public: `installed_interrupt_helpers`, `provider_installation`, `terminal_authority_permissions` (re-export mod, lib.rs:78-84). `#![forbid(unsafe_code)]` (lib.rs:4).

### D1 `access_lifecycles.rs` (603)
1. Purpose: settles native access-lifecycle boundaries (initialize, move-out, view, relocate) and `EmptyRunRelease` into admitted boundary settlements, after checking the value image and the profile provenance.
2. Shape: `settle_native_lifecycles(input, installation, policy)` (:34); `settle_empty_releases(input, policy)` (:168); `require_image_contract` (:238); `require_runtime_value` (:297); `require_fields` (:331); `bind_profile` (:356); `selected_profile_call` (:475); `argument_query`, `argument_storage` (:493, :514).
3. In/out: reads the Terminal module, abstract plan functions, `AdmittedProviderInstallation::installed_calls()`, descriptor graph (`terminal_semantics::descriptor_results`); returns `(settlements, mechanisms)`.
4. Decides on: boundary id of `CompilerPrimitive` lifecycle plans (BTreeMap :46-61), call position (block, ordinal), exact selected call per `(caller, InvocationSite::Operation(producer))` and boundary id (:481-488), descriptor payload target identity (`supplied != consumed` :460).
5. Cases by shape: `require_runtime_value` recurses into Record, Sum, Mixed and non-empty FixedArray; `_ => {}` accepts every other shape (:325). `require_image_contract`: refuses reference-bearing records (:251), non-record schemas (:248-250), and for `Initialize` refuses integer stored width different from the scalar's full byte width (:279-290). Comment :289: "native lifecycle narrowing requires checked concrete-value fitting evidence".
6. Same job: `settle_empty_releases` and the lifecycle branch both mint a `SettledCompilerBuiltin` + `AdmittedBoundarySettlement` + `AdmittedTerminalMechanism` triple; same shape also in `static_interrupts.rs:461-484`, `compiler_builtins.rs:333-341`, `native_binding_settlements.rs:149-160`.
7. Unused: none; both functions called in `admission.rs:196,255`.
8. Stated limits: "native resident content requires retained loan forwarding for reference fields" (:256); "non-runtime fields require an authored custody agreement" (:339, :347).
9. Fail-open: (a) `primitive_plans.is_empty()` returns empty (:62-64): no lifecycle plans means nothing to settle. (b) `continue` when the lowered function has no operation for the Terminal call (:94-99) and when the boundary was already settled (:130-135): a lifecycle call the lowered plan dropped is not checked. Traced: the dropped call is not executed natively. (c) `_ => {}` at :325 accepts other structural shapes (scalar, byte sequence, reference...). Classification: open for (c); (a)(b) traced.

### D2 `adapters.rs` (194)
1. Purpose: carries an admitted program-wide provider context and builds the synchronous call graph that includes the receiver's admitted provider choices.
2. Shape: `AdmittedProgramProviderContext` (:21; psi, profile, selected-plan digest, providers, entry bridge) with `require` (:46); `selected_synchronous_call_graph` (:77); re-export `retain_selected_provider_adapters` (:17, defined in `target_bound_program`, not read).
3-4. Decides on: Terminal psi identity, target profile, 32-byte selected-plan digest equality (:52-60); `target_members.terminal_psi()`/`target()` vs profile name (:86-88).
5. Cases: none.
6. Same job: `scopes.rs:49-73` builds the same graph twice (with and without the program context).
7. Callers: `selected_synchronous_call_graph` grep = 6 hits in 4 files (adapters 2, lib 1, static_interrupts 1, scopes 2). `AdmittedProgramProviderContext::new` is `pub(crate)`.
8-9. Stated limits: none. Fail-open: none seen.

### D3 `admission.rs` (506)
1. Purpose: the entry. `admit_native_providers` runs callback-entry admission, provider installation, interrupt helper/scope admission and settlement, compiler builtins, native bindings, reached-provider check, lifecycle settlements, provider executions, builtin settlements, the Darwin wait classification, the authority policy extension and the closure review, then packages `AdmittedNativeProviders`.
2. Shape: `admit_native_providers` (:146); `AdmittedNativeProviders` (:84); `AdmittedNativeCallableAuthority` (:50, minted by `from_admission` only); `ProviderExecutionEvidence` trait (:40); `admit_checked_provider_installation` (:362); `require_reached_providers` (:422); `realization_error` (:33).
3. In/out: reads `AdmissionSource`, canonical artifact, `ProviderAdmissionRequest`; returns the admitted bundle (settlements, executions, interrupt/callback cohorts, review receipt, installation, program context).
4. Decides on: a loop at :212-251 refuses (i) an opaque foreign settlement (not `Component`) for a boundary that `requires_checked_non_observation`, (ii) any non-component settlement for a boundary that `requires_checked_result`. `require_reached_providers` keys on the boundary requirement identity text: `selected.selected_requirement_row(requirement).is_none() && no assumed_bindings row.required == requirement` (:475-480).
5. Cases by shape: Darwin wait classification only `request.profile == MacosArm64` (:258). `native_calling` collected only for `NativeBinding` executions (:313-318).
6. Same job: `require_reached_providers` filter (:438-499) and the review's `selected_requirement` (`terminal_authority_review/context.rs:287`) both ask "is a provider selected for this boundary requirement".
7. Unused: none seen.
8. Stated limits: header comment :4-10 (direct-compilation imports carry a foreign same-stack reserve).
9. Fail-open: in `require_reached_providers`, `filter_map` with `?`/`return None` yields no diagnostic when the boundary is missing from `plan.boundary_machines` (:439-442), when it is `EmptyRunRelease` (:443-447), a placement operation (:466-473), already in `native_settlements` (:474), or its identity text is missing (`plan.identities.text(declaration.identity)?` :475). A reached boundary with no declaration or no identity text gets no "no provider selected" error here. Traced: `AuthorityGraph::new`/review context separately refuse empty or duplicate boundary identity (`context.rs:72-79`) later in the same function. Classification: traced (second line of defence).

### D4 `admission_source.rs` (208)
1. Purpose: the read-only program facts admission consumes, and the check that an installed private entry still belongs to the current interrupt cohort.
2. Shape: `AdmissionSource` (:31; abstract plan, closed module, target context, program entry, private entry RAM recipes); `AdmittedProgramEntryCall` (:74); `ComponentCallFacts` (:95); `AdmittedInstalledEntryExecution` (:121) with `for_entry`, `require_current` and private `check_current` (:146).
3. In/out: no mutation.
4. Decides on: psi identity, machine/operation, and exact unique `placement_index` in the current cohort (:163-169); scope field-by-field equality (:170-197).
5. Cases: `entry.entry().target().architecture != X86_64` refuses (:154): the installed-entry execution check is x86-64 only.
6. Callers: `AdmittedInstalledEntryExecution` grep = 20 hits in 11 files (stages 04/05/15, installation-admission, here).
7-8. Stated limits: header :11-12 "That condition belongs to the private object rather than every occurrence of the same source machine".
9. Fail-open: `(!received.installation_assumptions().is_empty() && received.installation_assumptions() != scope.installation_assumptions())` (:193): a received scope with no assumptions matches a scope that carries some. Also `received.context_anchor().is_some_and(..)` (:190) accepts a received scope with no anchor. Classification: open (wrong program: an execution scope that gained assumptions or an anchor the received cohort lacks).

### D5 `callback_entries.rs` (335)
1. Purpose: projects native callback arguments from retained placements and admits inbound handler entries by rejoining the retained callback occurrence, canonical body, selected Calling application and formal order.
2. Shape: `project_native_callback_arguments` (:23); `admit_entries` (:79); `PreparedEntries` (:110) with `new` (:122), `occurrence` (:201), `selected` (:212), `admit` (:230).
3. In/out: reads module machines, request callback occurrences and `installed_calling`; produces `AdmittedCallbackEntry` (an `Arc<BoundCallbackEntry>`).
4. Decides on: placement index (binary search :206), application index, thunk identity, calling ABI identity + owner application string (`BTreeMap<(&CallingAbiIdentity, Option<&str>), ..>` :115-118), body declaration identity (`same_identity_as` :259), formal order.
5. Cases: `project_native_callback_arguments` returns `Ok(None)` for the whole call when any placement has no `private_materialization` (:41-43).
6. Same job: `static_interrupts.rs:240-257` re-checks the same occurrence fields.
7. Callers: `project_native_callback_arguments` 5 hits in 4 files (grep): `verified-optimization-input/native_proposal.rs` 1, `admission.rs` 1, `callback_entries.rs` 2, `lib.rs` 1.
8. Stated limits: none. Built with a dense identity-span table: `bodies.try_reserve_exact(length)` for `last-first+1` machine ids (:141-149).
9. Fail-open: `Ok(None)` at :41-43 (stated by the doc: "without validating the thunk body"); one placement lacking materialization drops projection for all. Open.

### D6 `compiler_builtins.rs` (922) and `compiler_builtins/catalog.rs` (133)
1. Purpose: admits selected compiler builtins reached by the body and turns each into a settled realization; `catalog.rs` is the sealed conversion `CompilerIntrinsicExecutionIdentity -> CompilerBuiltinExecution`.
2. Shape: `reached_builtins` (:38), `settle_compiler_builtins` (:80), `ResolvedBuiltin::for_provider` (:352), `admit_x86_fma_features` (:385), `compiler_builtin_completion` (:428), `float_arithmetic_realization` (:445), `validate_access_schema` (:467), `primitive_access_carrier` (:601), `require_access_backing_origins` (:630); catalog: `SettledCompilerBuiltin` (catalog.rs:20, private field, `compile_fail` doctest :14-18), `compiler_builtin_execution` (:36).
3. In/out: reads request builtins, plan boundary declarations, selected plans, authority policy; produces settlements + mechanisms.
4. Decides on: enum identity; boundary id membership (binary search :73); for placed scalar access: schema method names built as strings (`"ref"`, `"ref-mut"`, `"named(name(u32))"` etc. :494-503, :601-627) and compared with identity text (:510-531, 22 conjuncts :510-565). Source-spelling compares: `format!("{}::{name}", method.requirement_owner)` against trait name text (:516-520).
5. Cases by shape: per-variant match of `CompilerBuiltinExecution` (:185-332). Windows `HostedExitProcessI32` has its own branch requiring an import with the reviewed authority (:146-184). UEFI-only `ProgramImageRetainedSuffixStart` (:224). `Take` accessor refused (:264). `HostedReadByte` always refused with "retired" (:305-308). `StaticInterrupt`, `AccessLifecycle`, `EmptyRunRelease` refused through proposals (:186-196). Access placement complete only for single full-width fragment (:581-597). Catalog: `TaskRegisterActivate/Read`, `SegmentReload/Read` map to `None` (catalog.rs:42-45).
6. Same job: access-lifecycle triple (see D1); FMA admission (see A2); `boundary_contract_matches_selected_schema` shared with `settlements.rs:202`.
7. Unused: `numeric_boundary_execution` is re-exported (`compiler_builtins.rs:8`, `lib.rs:41`) but I did not grep its external callers.
8. Stated limits: "native placed scalar access requires a complete scalar container; fragmented or narrowed codecs require an explicit native realization" (:596). "local target catalog cannot realize hosted process exit" (:285).
9. Fail-open: `reached_builtins` silently drops builtins whose boundary is not in the plan (:73-75); traced: they are not reached. `compiler_builtin_completion` maps every non-exit variant to NormalReturn (:433-441); if a new variant is added it must be listed (exhaustive match). Stated none. Placed-access check `_ => return Err` for access other than shared/mutable borrow (:497). No fail-open seen beyond the first.

### D7 `installed_interrupt_helpers.rs` (339)
1. Purpose: prepares once the per-helper answers ("is this arrival helper associated with an admitted installation scope and entry") for stage 02/03 checks.
2. Shape: `Rows<T>` (:20), `Associations<T>` (:44), `PreparedHelperCohort` (:62), `PreparedInstalledInterruptHelpers::new/verify` (:195-321), `checked_helpers(cohort) -> Vec<BoundInterruptHelper>` (:324). Errors are `&'static str` stored per helper: `refusal: answer.err()` (:335).
3. In/out: reads cohort helpers/scopes/entries and the module's installed registrations/callback applications; outputs answers by `(machine, operation)`.
4. Decides on: `(MachineId, OperationId)` sorted keys and binary search; unique entry per `(installation, application_index)`; kind equality.
5. Cases: `Install` is not an arrival helper (:147, :182).
6. Callers: grep `checked_helpers|PreparedInstalledInterruptHelpers` = 8 hits in 5 files; outside the crate: stage 04 `installed_interrupts.rs` 2, image-emission 2, installation-admission `resources.rs` 1.
7-8. Stated limits: module doc :1-7 (stage 03 cannot depend on admission).
9. Fail-open: `PreparedInstalledInterruptHelpers::new` returns an empty answer set when `cohort.helpers().is_empty()` (:206-212); `verify` then answers "no admitted association" per operation (:187), i.e. refuses. Not fail-open.

### D8 `lowering_facts.rs` (66), `model.rs` (263)
`lowering_facts.rs`: two cohort carriers (`AdmittedNativeCallbackCohort` :11, `AdmittedInstalledInterruptCohort` :34) with `pub(crate)` fields and `Arc<[..]>`. `model.rs`: request and admitted types: `ProviderAdmissionRequest` (:62, 17 fields), `NativeBoundaryRealization` (:19; Builtin, NormalizedForeignCall, Component, SystemCall), `NativeCallbackArgumentProposal` (:92), `InstalledInterruptNativeEntryProposal` (:109), `AdmittedInstalledInterruptEntry/Scope/Helper`, `AdmittedCallbackEntry`, `AdmittedBoundarySettlement` (:249), `AdmittedBoundaryExecution` (:256). No decisions. Comment :8-9 says fields "are constructed only after admission checks succeed"; constructors are `pub(super)`.

### D9 `native_binding_settlements.rs` (163)
1. Purpose: turns each retained Build native binding (import or syscall) into a settlement.
2. Shape: `settle` (:6).
3. In/out: reads `request.native_bindings`, module, `installed_calling`, `profile`; outputs settlements + mechanisms.
4. Decides on: boundary id duplicates (:36); requirement identity text must have no selected provider row and no builtin on the same boundary (:48-58); unique calling application per `abi` (:66-73); import requires `HostedApplication` entry schema (:84-91); syscall plan equals `evaluate_call_plan(policy, signature)` (:114-120).
5. Cases: two binding kinds only (`Import`, `Syscall`) (:77-139).
6. Same job: `settlements.rs:127-137` (system-call rejoin) and `rejoin_system_call` (:507-612) do a parallel syscall policy check for the provider route.
7. Callers: called `admission.rs:186`.
8. Stated limits: error text :88 "foreign import needs admitted demand on a fixed or freestanding stack".
9. Fail-open: `continue` when `input.plan().boundary_machine(row.boundary).is_none()` (:62-64), stated: "The module may retain choices for other roots."

### D10 `provider_installation.rs` (299), `provider_installation_replay.rs` (643)
1. Purpose: admits the checked-provider installation: picks the one installed candidate per boundary from the selected adapter rows and replays every installed call against the lowered plan.
2. Shape: `admit_provider_installation_with_projection` (:167), `admit_terminal_provider_installation` (:196, `pub`), `select_provider_candidates` (:217), `AdmittedProviderInstallation` (:116), `AdmittedInstalledProviderCall` (:64), `ProviderInstallationError` (:28); replay: `replay_installed_provider_calls` (replay:48), `external_completion_provider_result` (:24), `claim_content_projections` (:390), `replays_result` (:417).
3. In/out: reads abstract plan, artifact view, selected adapters, assumed bindings; outputs installed candidates and call evidence.
4. Decides on identity text: candidate matches a selected row iff `provider_declaration`, `provider_identity` and `candidate_identity` texts equal the selected row's three strings (provider_installation.rs:264-276); selected rows grouped by requirement text (:240-250). Exactly one match or error (`SelectedProviderMismatch`/`AmbiguousSelectedProvider` :277-283).
5. Cases: replay handles two op kinds, `ExternalCompletion` and `BoundaryCall` (replay:80, :180), `continue` for others. Claims replay compares entry claims to completion receipts by `(argument_index, path)` (:296-359).
6. Same job: `admit_terminal_provider_installation` (:196) and `...with_projection` (:167) both call `select_provider_candidates`.
7. Unused: `admit_terminal_provider_installation`: grep = 2 hits (definition, `build/evaluation/admission/provider_execution.rs`): one external caller.
8. Stated limits: replay header :1-15 (does not require the consumed value to be an unchanged entry parameter).
9. Fail-open: `if installed.is_empty() { return Ok(Vec::new()) }` (replay:54); `continue` when no installed provider for a boundary (replay:90-92, :193-202) so calls to boundaries without an installed provider are not replayed here (those are the builtin/foreign boundaries). `or_insert` keeps the first of duplicate rows (replay:59,63,67,71,75). Traced: duplicates are refused upstream for installed candidates by `select_provider_candidates` (one per boundary). Open for the other maps.

### D11 `settlements.rs` (612), `settlements_source_imports.rs` (556), `source_evaluated_bindings.rs` (504)
1. Purpose: `settlements.rs` rejoins each provider execution's evidence to exactly one selected plan row and one retained external-binding row, builds the foreign/component/system-call realization and checks completion; `settlements_source_imports.rs` derives the physical mechanism identity of each demanded import/syscall/table binding and any Darwin clock policy row; `source_evaluated_bindings.rs` mints execution evidence and same-stack contributions from selected plans for direct compilation.
2. Shape: `settle_provider_executions` (settlements.rs:48), `settle_boundary` (:92), `rejoin_component_call` (:332), `rejoin_normalized_foreign_call` (:389), `rejoin_system_call` (:507), `external_calling_application` (:274), `selected_plan_from_exact_evidence` (:316); `derive_source_evaluated_mechanisms` (source_imports:50), `join_import_coverage_rows` (:244), `authenticate_callback_application` (:411), `require_exact_native_declaration_abi` (:24), `selected_darwin_clock_row` (:214); `prepare_source_evaluated_bindings` (sev:74), `mint_source_evaluated_imports` (:177), `prepare_system_calls` (:274), `SAME_STACK_BYTES = 256*1024` (:124).
3. In/out: reads request settlements, external binding rows, selected plan facts; outputs `AdmittedBoundarySettlement`, provider executions sorted by `(requirement text, plan report id, execution report id)` (settlements.rs:77-88), mechanisms, policy rows.
4. Decides on identity text throughout: `plan.identities.text(row.requirement_identity) == requirement` (settlements.rs:115, :414, :529; sev:292, :357), `row.requirement_identity == requirement` for external rows (settlements.rs:424, :544). Component calls compare plan digest, ABI, target, locator slot commitment and same-stack report ids (:350-371). Also report fingerprints are FNV hashes of requirement text (sev:206-226, :309-324).
5. Cases by shape: four mechanism families: Vtable/Table function, Syscall, Import, others `continue` (source_imports:95-200, `_ => continue` :199); realization match over `SystemCall | NormalizedForeignCall | Component | Builtin` (settlements.rs:126-174). Table rows only for `(VtableField, VtableField)` and `(TableFunction, TableFunction)` pairs (:438-454).
6. Same job elsewhere: `rejoin_system_call` (settlements.rs:507) vs `native_binding_settlements.rs:106-138` (syscall policy), vs `conservative_syscall_terminal_mechanism` (`external_mechanisms.rs:110`). The `Table` mechanism identity is built in `source_imports:128-139`; `exact_mechanism` in `context.rs:381-396` re-checks it.
7. Unused: `prepare_source_evaluated_bindings` grep 8 hits in 5 files (2 in `02_.../admit_providers.rs`, 2 in `tests/physical_publication/callback.rs`).
8. Stated limits: sev header :12-16 "unmeasured foreign entries receive the 256 KiB `SAME_STACK_BYTES` reserve unless a bodyless `via` leaf authored `stack N bytes`." (constant :124).
9. Fail-open: `require_exact_native_declaration_abi` returns `Ok` when `row.calling_signature` is `None` (source_imports:29-37). Stated at :22-23 ("Ordinary calls and direct imports are checked separately"). `external_calling_application` skips the schema commitment comparison when the plan declares no calling application for the requirement (settlements.rs:294-312, `if declared`). Open. `settlements_source_imports.rs:73-75`: rows with no selected external row `continue`. Settlements: `if input.plan().boundary_machine(boundary.id).is_none() { continue; }` (settlements.rs:212-214) but the schema contract check runs first (:202).

### D12 `static_interrupts.rs` (486), `static_interrupts/arrival_helpers.rs` (1038), `static_interrupts/scopes.rs` (798)
1. Purpose: admits x86-64 static interrupt installations: proves each arrival helper (acknowledgement, context view, begin/finish completion) belongs to a registered handler, admits each handler entry to its installation's private table slot, prepares the installation `Scope` (six loans, validator/work/completion bodies, gate offsets, platform assumptions), and settles the install/helper boundaries.
2. Shape: `require_helper_associations` (si:40), `admit` (:124), `settle` (:355); `arrival_helpers::validate` (ah:66), `check_closure` (:248), `Calls::new` (:799), `require_associated_calls` (:981), `selected_body` (:674), `acknowledgement_entry` (:703); `scopes::prepare` (sc:29), `ConfigurationLease` (:531) with `require`, `require_masked`, `changes_configuration` (:636), `context_anchor` (:493), `assumed_installation_sites` (:363).
3. In/out: reads installed-interrupt registrations, callback applications, abstract plan, call graphs, installation assumptions; outputs `AdmittedInstalledInterruptCohort` (entries, scopes, helpers) and settlements.
4. Decides on: `(MachineId, OperationId)` sites; unique matches via `(Some(x), None) = (iter.next(), iter.next())` (ah:685, sc:478); identity of values via `ValueIdentityAnalysis`; dominator order for begin-before-finish (`ExecutionOrder::precedes` ah:560); a body's callback by `same_identity_as` identity (ah:199, :692).
5. Cases by shape (fixed lists): installation must have exactly six structural operands (ah:156 and sc:184 "expected six structural formals"), with fixed access pattern mutable, mutable, shared, ?, ?, shared (sc:205-208) and operand 5 = context type (ah:162, sc:227), helper ordinals 1/2/3 = validator/work/completion (sc:214-219, ah:163), `Install` result must be Boolean (sc:190); Context view must be a `SharedBorrow` reference to the installation context type (ah:327-330); `context_anchor` requires x86-64 (sc:496). `ConfigurationLease::require` refuses `ActivateTaskRegister`, `ReloadSegmentDescriptors`, GDT `Load`, and register events in `changes_configuration` (GDT load, task-register activate, segment reload, `SystemWrite`, `ModelSpecificWrite`) (sc:558-567, :636-645); other register operations are not refused. A boundary in the closure that is neither a known interrupt helper nor `published_service_ceiling` containing the machine-control service passes (sc:605 `None => {}`).
6. Same job: `PreparedHelperCohort` (D7) re-derives checks over the same helpers; `entry_custody.admit` called in both `admit` (si:257) and `validate` (ah:198).
7. Callers: `selected_body`, `acknowledgement_entry` are `pub(super)` used by `scopes.rs`.
8. Stated limits: si header :11-17 "these rows grant neither root memory nor permission to publish IDTR until the native implementation executes its guarded installation".
9. Fail-open: `scopes::prepare` `continue`s a registration that has no admitted entries and no helper roots (sc:124-126): an installation neither reached nor entered is not given a scope. Traced: `admit` (si:155-163) returns an empty cohort only when nothing reached; `si:325-336` errors for a reached installation missing handler entries. `ConfigurationLease` fail-open for `None => {}` is open (an unlisted intrinsic, e.g. an IDT/CR load other than `SystemWrite`, depends on `changes_configuration`'s list).

### D13 `task_activations/{mod,arguments,carry,provider,stack,storage}.rs` (122+91+127+84+339+212) and `tests.rs` (157)
1. Purpose: validates published task-activation demands: selected TaskRuntime plan covers the start requirement, argument layout is canonical, stack frames compose into a fixed WCSU stack, suspension crossings match carried-live values and obligations.
2. Shape: `validate_task_activations(demands, identities, selected_plans, profile)` (mod:18); `TaskArgumentLayout` (arguments:11); `validate_crossings` (carry:38); `validate_selected_runtime` (provider:8); `compose_task_stack_demand` (stack:140), `project_wcsu_stack_plan` (:129), `TaskStorage` (storage:10).
3. In/out: reads demands + target layout applications (`profile.target_name()` rows), identity table; returns `Result<(), Vec<Diagnostic>>`; no admitted value is produced (test-only success).
4. Decides on: identity texts (`method.requirement_owner == requirement_owner_path` and schema text equals authored identity: provider:17-21); counts (exactly one plan, one method, one covering row: provider:40-81); operation name "start"/"try_start" only in messages (provider:48-51).
5. Cases: shapes of frames and crossings; limits: `MAX_TASK_ARGUMENT_ALIGNMENT = 1 << 16` (arguments:34); stack representation tag string "fixed-nonmoving-stack-v1" (mod:53).
6. Same job: none.
7. Callers: `validate_task_activations` grep = 7 hits in 4 files: 1 external, `verified-optimization-input/native_proposal.rs:1`; remaining are lib re-export, definition and 4 in `tests.rs`.
8. Stated limits: "an activation argument layout is not the canonical packed marshalling layout" (mod:62); alignment bound above.
9. Fail-open: `demands.demands.is_empty() -> Ok(())` (mod:24-26). Two checks are self-comparisons: `argument_layout.is_canonical()` recomputes the same packing from the layout's own fields (arguments:29-31, :77-91), and `projection.validate(...)` compares a fingerprint computed from the same `ComposedTaskStackDemand` (stack:114-118), so neither can fail for a layout/projection built by these functions. Classification: traced (read, inferred from code).

### D14 `terminal_authority_permission_policy.rs` (283)
1. Purpose: the receiving permission policy (sorted `(service_schema, requirement)` rows) and the package-permission custody checks.
2. Shape: `TerminalAuthorityPermissionPolicy` (:32), `permission_for` (:51), `terminal_authority_permission_policy_with_rows` (:89), `admit_package_terminal_authority_permissions` (:221), `validate_retained_package_terminal_authority_permissions` (:165), `validate_package_terminal_authority_permission_custody` (:232).
3-4. Keyed on `(ServiceSchemaDigest, requirement identity text)` (:56-62); text must be non-empty and have no control chars (:99-103). Deny-by-absence: `permission_for` returns an error when there is no row.
5-9. Same job: duplicate-coordinate checks at :116-129 and :236-248 (two copies). Callers: `admit_package_terminal_authority_permissions` exported at `lib.rs:78-84`; not grepped. Fail-open: none seen.

### D15 `terminal_authority_policy.rs` (657) and `terminal_authority_policy/*`
1. Purpose: the physical authority policy: which host-authority classes a mechanism exercises. Compiler intrinsics are a closed, reserved inventory; foreign/table/syscall/checked-physical rows are explicit and extended from reviewed host-operation tables.
2. Shape: `TerminalAuthorityPolicy` (:91) with `classify` (:142) and `with_reviewed_operations` (:117); `terminal_authority_policy_with_rows` (:190; version 12, :55); `reviewed_host_operation` (:282); `current_terminal_authority_policy` (:297); `classify_parameterized_intrinsic` (:310: integer comparison of any valid width, float conversion with valid signature, accessor scalar read/write with nonzero commitments classify to empty authority).
3. In/out: `with_reviewed_operations` adds reviewed rows only when the mechanism has no row yet ("The receiver's own rows take precedence" :115).
4. Decides on mechanism identity bytes; explicit rows for a compiler intrinsic are refused (:195-199).
5. Cases: `Syscall` rows only for `LinuxX64 | LinuxArm64` (:231-240); `CheckedPhysical` port write must have exactly `[PortIo]` (:251-262).
6. Same job: `classification.rs` and `numeric-intrinsics/catalog.rs` both enumerate `BuiltinFunction`; `reviewed_host_operation` dispatches to `windows_imports`, `darwin_poll`, `posix_operations`.
7. Callers: `time_host_permission_rows|filesystem_host_permission_rows` grep = 11 hits in 5 files: 3 in `package-manager/review/candidate/semantic_bindings.rs`.
8. Stated limits: none beyond comments. Test pins version 12 (:630).
9. Fail-open: none seen: unknown mechanisms return `UnclassifiedTerminalMechanism` (:163).

`terminal_authority_policy/classification.rs` (320): `CLOSED_POLICY_ROW_COUNT` starts from a literal `46` (:31) plus the non-special-form builtin count; `closed_policy_mechanisms` asserts equality at :110. `classify_builtin_function` (:187) is an exhaustive match (no wildcard): CLI/STI -> `InterruptControl`; hlt, restore flags, MSR, CR0-4, GDTR, cache ops and AArch64 system registers -> `MachineControl`; port in/out -> `PortIo`; fences, float, content and other -> empty. `PrimitiveFloatBinary`/`NamedFloatNegation` compute a tag then discard it (`_closed_coordinates` :175, `_closed_format` :183).

`terminal_authority_policy/darwin_poll.rs` (284): recognises macOS-arm64 `libSystem` `_poll` (:23-34) and grants an empty-authority row only when the checked program proves the descriptor count argument is a literal 0 at every selected call (`verify_calls` :120-178, `has_zero_count` :108). Keyed on locator digest, signature shape (`has_poll_signature` :36-75), constants. Fixed to `MacosArm64`.

`terminal_authority_policy/external_mechanisms.rs` (335): `normalized_foreign_terminal_mechanism(_with_callback_materializations)` (:30, :54) re-validate the entry plan and refuse non-canonical plans; `conservative_syscall_terminal_mechanism` (:110) hashes parameter order, scalar carriers and structural carrier identities; refusals: not Linux x64/arm64 (:120-128), boundary `requires` non-empty (:145-150), structural qualifications (:211-222), no checked call occurrence (:173-175). Stated limits: "does not yet support boundary structural-domain requirements" (:147), "does not yet support root structural qualifications" (:213), "projected structural qualifications" (:219).

`terminal_authority_policy/host_cohorts.rs` (190) and `host_cohorts/tests.rs` (118): the filesystem and time permission-row generators key on method NAME strings: `permitted_filesystem_facets(&method.name)` (:82) with a hard list of ~45 spellings (:47-72) and `settled_time_host_cohort(&method.name)` (:122) with six spellings (:104-114); Darwin clock keyed on trait text `"DarwinClock"` plus method name `clock_ns`/`mach_absolute_time` (:149-157). Unknown names return `UnknownRequirement` (:72, :123). The tests pin that a method name alone cannot classify an unreviewed import (tests:76-118).

`terminal_authority_policy/posix_operations.rs` (295): reviewed Darwin libSystem symbols (25) for macOS arm64/x64 and `__errno_location` for glibc (x64 `GLIBC_2.2.5`, arm64 `GLIBC_2.17`) (:63-127), and Linux syscalls through `target::LinuxFilesystemOperation::from_syscall` (:26-43). `Openat, Close, Pread64, Pwrite64` return `None` (unclassified) (:41); exit codes 60/231 (x64) and 93/94 (arm64) -> `ProcessTermination` (:27-32). Requires nonzero implementation contract, `AdmittedPlan` argument contract (:54-61) for imports and nonzero checked contract for syscalls (:141).

`terminal_authority_policy/windows_imports.rs` (290): 16 reviewed kernel32/ntdll imports keyed on the normalized PE locator digest (:46-104). `ReadFile` classes: content read + process input + process output; `WriteFile`: content write + process output; `CreateFileW`/`NtOpenFile`: all six filesystem facets (:60-61). `DuplicateHandle`/`CloseHandle` and CRT `_read/_write` stay unclassified (header :13-19).

### D16 `terminal_authority_review.rs` (264), `reachability.rs` (328), `context.rs` (406), `reviewer.rs` (556)
1. Purpose: walks every reachable boundary from the entry and records one leaf per terminal mechanism with its exercised class set and, when a permission policy is supplied, its permitted set; then builds the receipt.
2. Shape: `review_terminal_authority_closure` (review:70); `physical_authority_use(&AbstractOperation)` (:132: exhaustive match, `PortWrite` and register-class ops are the only non-`None`); `AuthorityGraph` (reach:38) with `reachable_authority_edges` (:122); `ReviewContext` (ctx:34) with `exact_mechanism` (:361), `checked_adapter` (:227), `selected_requirement` (:287), `missing_requirement` (:314); `Reviewer::expand_boundary` (rev:77), `admit_program_root` (:387), `leaf_permission` (:448).
3. In/out: reads admitted mechanisms, settlements, selected plans, installed interrupts, component authority; outputs a `TerminalAuthorityClosureReviewReceipt`.
4. Decides on: boundary id for mechanism rows; requirement identity text for selected rows (`selected.selected_requirement_row(requirement)`); checked adapter by three identity texts (supplied requirement, provider type, realization identity) (ctx:251-269); role match `(ProviderBinding, TerminalMechanismIdentity)` pairs (ctx:369-398); target profile equality (ctx:304, :379, :396).
5. Cases by shape: `ProviderBinding::VtableSlot` refused as unsupported (rev:378-382); x86 `PortWrite` on non-x86 target refused (reach:210-217); register ops must match target architecture (:242-262). Missing-requirement diagnostic lists at most 12 offers and stops at 16384 bytes (ctx:349).
6. Same job: `leaf_permission`/leaf admission duplicated in `admit_leaf` (rev:519) and `admit_native_leaf` (:484) (differ only in custody constructor and key).
7. Unused: `AuthorityGraph::new` used in `admission.rs:431` and `context.rs:67`.
8. Stated limits: ctx header "up to 12 selected rows"; review.rs:13-18 comment about exhaustive `AbstractOperation` matching.
9. Fail-open: `leaf_permission` (rev:461-470): with no permission policy, an unclassified mechanism gives `exercised = None` instead of an error (`Err(_) => None`). Stated by `review.rs:6-11` ("retained without claiming receiver admission") and by the context doc (ctx:41-45). `reachable_authority_edges` `continue`s a register access that needs no service (reach:271-273). `admit_program_root` refuses whenever a permission policy exists (rev:391-393).

### D17 `terminal_authority_review/windows_handle_*.rs` (in this unit: activations 327, aliases 804, arguments 689, calls 438, conditions 199, contracts 633, inputs 292, lifetimes 853, origins 562, paths 256; `windows_handle_replay.rs`, `_storage.rs`, `_scalar_table.rs`, `_values.rs` are not in this unit)
1. Purpose: Windows-only proof that every handle acquired by the reviewed `CreateFileW`/`DuplicateHandle` calls is released on every returning path with no use after release, no escape and no asynchronous I/O, so a narrower authority class set than the generic import row can be claimed.
2. Shape by file:
   - `windows_handle_contracts.rs`: `WindowsHandleContract` (:21), `reviewed_apis()` (:85; 8 rows), `no_handle_intervention` (:36; 7 locators), `reviewed_call` (:217), `reviewed_process_exit` (:264), `matches_declaration` (:418), `expected_call` (:507). Keyed on the normalized PE locator digest and exact parameter types/placement.
   - `windows_handle_calls.rs`: `HandleCallGraph::new` (:112), `Caller` (:22), `RuntimeCall` (:52), `boundary_target` (:322).
   - `windows_handle_inputs.rs`: `WindowsHandleInputs::prepare` (:113), `HandleProofProgress` (:30; progress print to stderr when env `OMEGA_WINDOWS_HANDLE_PROFILE == "1"` :37).
   - `windows_handle_origins.rs`: `HandleOrigins::derive` (:106) builds origin equations; `result_equation` (:341), `expression_equation` (:418).
   - `windows_handle_arguments.rs`: `HandleArgumentValues` (:72), fixed-point solver (`from_origins` :189) producing per-call receipts; `closed_argument_roots` (:122).
   - `windows_handle_activations.rs`: `ActivationPaths::derive` (:68) balanced call/return relation; `outer_contexts`, `inner_contexts`.
   - `windows_handle_aliases.rs`: `HandleWordAliases` (:18), `HandleAliases` (:247), byte-range tracking `HandleStorageRange` (:137).
   - `windows_handle_lifetimes.rs`: `HandleLifetimes` (:122) ledger: `acquire_file`, `duplicate_file`, `synchronous_io`, `release_file`, `returning_path`, `caller_continuation`, `merge_use_history`.
   - `windows_handle_paths.rs`: `HandlePathProgram::replay` (:66) replays every guarded path.
   - `windows_handle_conditions.rs`: `ScalarConditions` (:14) necessary guard facts.
3. In/out: reads the retained Terminal module, the context's mechanisms and settlements; produces `CheckedHandleCalls` (defined in `windows_handle_replay.rs`, not in this unit) that adds physical rows to the policy (`reviewer.rs:329-337`).
4. Decides on: locator digests; exact argument constants (e.g. `CreateFileW` argument 3 and 6 exactly 0, `DuplicateHandle` arguments 0 and 2 = current-process pseudo-handle, argument 5 = 0, argument 6 = 2 (`DUPLICATE_SAME_ACCESS`), query mode class 4 length 16 (arguments:359-436)); constants `0x0400_0000` (delete-on-close) and `0x4000_0000` (overlapped) in lifetimes:221, :299.
5. Cases by shape / fixed lists: the 8 reviewed APIs (contracts:93-187); read/write mapped to a 5-native-word plan with a 16-byte byte view (contracts:524-545); comment contracts:82-84: "The model is deliberately bounded to native calls in this customer's closure. Unsupported handle operations remain unknown". Origin derivation refuses nonnormal result publication (origins:356), cleanup-site storage (origins:322-339), mixed physical/body result (:404-409).
6. Same job: the `65_536` budget constant appears in `activations:56`, `calls:129,153,255`, `conditions:101`, `aliases:297`, `inputs:273`, `origins:118`, `paths:140`; `arguments:18-19,220` use 64, 1,024 and 1,048,576. Grep not run for count; read.
7. Unused: `observe_scalar`, `escape`, `attach_deletion`, `assign_scalars`, `write_partial_word`, `copy_region` are `#[cfg(test)]` (read); `windows_handle_activations`, `_conditions`, `_paths` are declared `#[cfg(test)]` modules in `terminal_authority_review.rs:29-31,38-40,49-51`, so in non-test builds the path-replay, activation-path and condition files are not compiled (read). `HandlePathProgram` is then not referenced outside tests in this unit; its production caller, if any, is in `windows_handle_replay.rs` (not read).
8. Stated limits: comments "finite analysis budget" (activations:58, origins:120, arguments:222, paths `AnalysisBudget`); "Unsupported handle operations remain unknown, never preserving by default" (contracts:84); "These mathematical conditions do not establish a native handle's origin" (conditions:6).
9. Fail-open: `windows_handle_conditions.rs:46-48` returns `true` for a storage-observation term in `require` (comment: "Live structural observations need their own version proof"); `proves` negates it, so it proves nothing (:58-61). Stated. `ArgumentValues::join` returns `false` (no change) for `Unestablished` incoming (:44-46); unseeded cycles are promoted to `Unknown` before use (:265-282). Traced. `reviewed_call` returns `Ok(None)` for unrecognised Windows imports (:221-240); those become `opaque_calls` in inputs:153-160, i.e. an "intervening call", not a pass.

---------------------------------------------------------------------------------------------------
## Cross-file notes

- Same job in several places: `AdmittedBoundarySettlement` + `AdmittedTerminalMechanism` + `SettledCompilerBuiltin::from_identity` triple constructed in `access_lifecycles.rs:136-161,221-233`, `static_interrupts.rs:461-484`, `compiler_builtins.rs:333-341`, `native_binding_settlements.rs:149-160`.
- Identity-text keying: the admission code compares requirement identity strings (not ids) in settlements.rs, settlements_source_imports.rs, source_evaluated_bindings.rs, selection.rs, context.rs, provider_installation.rs.
- Source spelling used for authority decisions: method names in `host_cohorts.rs:47-72,104-114,149-157`; schema-method name `"read"/"take"/"write"` and type name strings in `compiler_builtins.rs:494-531`; trait name `DarwinClock` in `host_cohorts.rs:18`.
- Platform scoping in code: Windows handle proof (Windows-only locators), Darwin poll (macos-arm64 only: `darwin_poll.rs:24`), static interrupts (x86-64: `scopes.rs:496`, `admission_source.rs:154`), syscall argument contract (Linux x64/arm64 only: `external_mechanisms.rs:120`).

---------------------------------------------------------------------------------------------------
## Coverage
Read in full (all 59 ranges of `units/unit_036.tsv`, each from line 1 to its last line, 21,460 lines in total by the unit's counts): numeric-intrinsics 5 files (789 lines), authority 2 (1,093), same-stack 2 (315), provider-admission/src 50 files (rest). Skimmed: none. Not read: none. Greps run: `admit_terminal_provider_installation`, `selected_synchronous_call_graph`, `direct_numeric_target|with_direct_numeric`, `checked_helpers|PreparedInstalledInterruptHelpers`, `AdmittedInstalledEntryExecution`, `time_host_permission_rows|filesystem_host_permission_rows` (source-rust and tools, *.rs), `validate_task_activations`, `prepare_source_evaluated_bindings`, `project_native_callback_arguments`, `boundary_contract_matches_selected_schema` (all over `source-rust/`, count mode). Counts for other functions were not grepped and are labelled so.

## Five things a maintainer most needs to know
1. Admission decides almost entirely on identity TEXT of requirements (settlements.rs:115,414,529; source_imports:333-337; sev:357; context.rs:251-269), not on ids; mechanism identities (`mechanisms.rs:177`) are the stable key for authority rows.
2. Authority classification is a closed inventory for compiler intrinsics (`classification.rs:30-112`, version 12 pinned at `terminal_authority_policy.rs:55` and test :630) plus reviewed per-platform tables keyed on normalized locator digests (posix, windows, darwin). Anything not listed is "unclassified", which with a permission policy is an error and without one is recorded as `exercised: None` (`reviewer.rs:461-470`).
3. Hard-coded shapes: static-interrupt installation has exactly six structural operands and helper ordinals 1/2/3 (`scopes.rs:184,214-219`, `arrival_helpers.rs:156,163`), x86-64 only; the Windows handle proof covers exactly 8 APIs with fixed protocol constants (`contracts.rs:85-187`, `arguments.rs:359-436`, `lifetimes.rs:221,299`).
4. Method-name spelling still decides permission rows for filesystem/time hosts (`host_cohorts.rs:47-72,104-114`) and the Darwin clock (`:149-157`), and the placed scalar access schema check compares composed type-name strings (`compiler_builtins.rs:494-531`).
5. Several checks cannot fail by construction: `TaskArgumentLayout::is_canonical` and `StackProjection::validate` recompute from their own inputs (`arguments.rs:29,77`; `stack.rs:114`); three `windows_handle_*` files (`activations`, `conditions`, `paths`) are `#[cfg(test)]` modules in `terminal_authority_review.rs:29-51`.

## What I am unsure about
- Whether `HandlePathProgram::replay` and `ActivationPaths` have a production caller: they sit in `#[cfg(test)]` modules here, and the production replay is in `windows_handle_replay.rs`/`_storage.rs`/`_values.rs`, outside my unit and unread.
- `catalog_realization` and `numeric_boundary_execution` external caller counts were not grepped.
- Whether `PrimitiveIntegerComparison` is meant to have no float catalog entry (`catalog.rs:260` returns `None`; `compiler_builtins/catalog.rs:97-108` routes it there; `classification.rs` classifies it as empty authority). I recorded behaviour, not intent.
- Receipt digest naming: test comment says "v5" (`receipt.rs:797`), domain string says `v6` (`:549`).
- I did not read `admission.rs` of numeric-intrinsics (not in this unit), so `AdmittedTerminalIntrinsics` fields are only seen through uses.
