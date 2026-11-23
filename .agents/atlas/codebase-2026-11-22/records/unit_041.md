# Record: unit_041 (areas 08-psi-program and 07-psi-semantics)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Paths are relative to `source-rust/subsystems/compiler/psi/` (written `psi/`) unless full. Everything below was read line by line with the Read tool; counts come from Grep (count or content mode) over `source-rust/`. "Not counted separately" means the pattern was grepped together with others.

The unit has 93 files, 20,300 lines. They belong to nine Rust crates: the root `terminal-psi` crate (identities, interfaces, layouts, machines, proofs, scalars, `psi.rs`), `application-identity`, `psi-program-identity`, `service-surface`, `psi-effect-contracts`, `proof-applications`, `layout-plans`, and from area 07 `closed-predicates`, `layout-planning` (default-integer-layouts) and `proof-admission` (bounded.rs only).

---

## A. `identities/` (2,929 lines)

### A1. `identities/applications/src/type_identity.rs` (2,068 lines; crate `application-identity`)

1. Purpose. Defines the text grammar of "type identities" (nested strings such as `ref-mut(ref(compiler-type(atom(i64)),region(...)))`), a reader that validates them, and a visitor trait that lets each consumer (package review, native, source) hang its own checks on that walk. It also has functions that compare a closed call's identity text against a calling-ABI record.
2. Shape. Header comment lists the submodules (`:1-7`). `compiler_type_name`/`compiler_type_identity` (`:10`, `:18`). `pub mod encoding` (`:30`): `CoordinateTag` enum of 23 tags (`:34-58`), `atom` (escapes `\ ( ) ,`, `:103`), `byte_atom` (length:hex, `:125`), `compound` (`:141`), `callable_declaration_path` (`:92`). `mod callable_display` (`:159`) with `callable_display_path`. `mod framing` (`:259`) with `Reader` (`:264`: `text`, `position`, `callable_regions`). `mod grammar` (`:488`): `type_identity` (`:510`), `declared_region` (`:591`), `callable_child` (`:621`), `canonical_const` (`:656`), `region` (`:675`), `semantic_name` (`:732`), `nominal_coordinates` (`:770`), `owner` (`:832`), `constraint` (`:855`). `mod index` (`:910`): `array_length`, `expression` (`:936`), `pattern`, `static_argument` (`:1225`), `embedded_operation`, `embedded_algebra`. `mod lifetime_partition` (`:1306`). `pub mod vocabulary` (`:1325`): trait `Spelled`, trait `Vocabulary` (`ALL`, `parse`), macro `vocabulary!` (`:1343`), 14 enums: `TypeForm` (13 spellings), `CallableForm`, `NameForm`, `OwnerForm`, `ConstraintForm`, `DomainFamily`, `ArrayLengthForm`, `IndexForm` (about 50), `RegionForm`, `PatternForm`, `FieldActionForm`, `Label` (about 39). Public functions: `package_declaration_identity` (`:1590`), `closed_requirement_matches_calling_abi` (`:1605`), `closed_callable_matches_calling_abi` (`:1676`), `visit_type_identity_package_owners` (`:1942`), `validate_source_type_description` (`:1954`), `visit_trait_application_package_owners` (`:1959`), `validate_trait_application` (`:1968`), `visit_static_argument_package_owners` (`:1975`), `malformed_identity_offset` (`:1984`). Trait `TypeIdentityPackageOwnerVisitor` (`:1879`) with 6 required and 11 defaulted methods. Error enum `TypeIdentityVisitError` (`:1850`).
3. Inputs and outputs. Input is identity text (`&str`) and, for the ABI matchers, `semantic_vocabulary::CallingAbiIdentity` (`:1610`, `:1680`) and `PackageKeyIdentity` (`:1591`). Output is `Result<(), TypeIdentityVisitError>`, `Result<bool, ..>`, an offset, or built strings. Nothing is mutated except visitor state.
4. How it decides. Every decision is on spelling. Tag recognition lexes a run of lowercase letters, `-` and `_` (`framing::peek_token`, `:307-314`) and matches it against the `vocabulary!` string tables (`:1363-1368`). `callable_display_path` searches the text for the literal `named-callable(path(` (`:163-164`) and strips a literal `nominal(` prefix and `,path(` separator (`:171-173`). Region admission is keyed on the `callable_regions` flag (`:525-529`, `:680`, `:1230-1236`) and on the owner atom equalling the literal `schema` (`:600`, `:683`). The Cast domain must be an `ArithmeticDomain` name or the `Exact` name (`:1046-1049`). `closed_callable_matches` compares text of the template path, owner and operand identities against fields of the ABI record (`:1748-1809`); operand kind is compared via a hard-coded kind-to-string table (`:1649-1656`).
5. Cases by shape. `index::expression` is an exhaustive `match` on `IndexForm` (`:940-1138`); `IndexForm::StaticApplication` at expression position is `MalformedIdentity` (`:1134`), and `UnsupportedConstSubstitution`/`UnboundMatchSubject` are `Unadmitted::InvalidIdentity` (`:1135-1137`). Grammar refuses `OwnerForm::Unresolved` (`:851`), `NameForm::InvalidDerivedOrigin` (`:762`), `ArrayLengthForm::InvalidExpression` (`:932`). Visitor `DeclarationIdentitySyntax` caps depth at 128 (`:1824`). A toolchain-owned nominal in the ABI matcher is refused with comment: "Do not guess an owner correspondence." (`:1763-1765`).
6. Same job elsewhere. Three further `TypeIdentityPackageOwnerVisitor` implementations that each count depth/accept owners: `DeclarationIdentitySyntax` (`:1814`), the local `Unbudgeted` (`:1985`), the test `Schema` (`:2031`); outside the file, `read_core_filesystem_declaration` in `interfaces/service_schemas.rs:296-360` implements its own with a depth cap of 128 (`:303`). The closed-call template/operand comparison is written twice inside this file: `closed_requirement_matches_calling_abi` (`:1634-1666`) and `closed_callable_matches` (`:1781-1798`), with the same ordinal loop. Suspected (not read): `source-to-psi/representations/typed-trees/type_records/type_identity.rs` also builds these strings (grep for `type_identity::encoding`: 33 matches in 15 files).
7. Unused or one-caller. Grepped `source-rust/`: `closed_requirement_matches_calling_abi` has 1 non-test caller (`interfaces/boundary_machines.rs:346`) and 2 test callers (`source-to-psi/pipeline/06_.../tests/boundary_calls.rs:1578,1597`); `closed_callable_matches_calling_abi` has 1 caller (`package-manager/review/evidence/src/record/contracts/expressions_operator_policy.rs:121`); `validate_source_type_description` 1 caller (`06_.../src/nominal_calls.rs:83`); `validate_trait_application` 1 caller (`expressions_operator_policy.rs:101`); `visit_trait_application_package_owners` 1 external caller (`package-manager/.../membership/visitor.rs:169`) plus its use in `validate_trait_application`; `visit_static_argument_package_owners` 2 external callers (`psi/semantics/src/structural_qualifications.rs:2323`, `visitor.rs:162`); `normalize_requirement_lifetime_partition` 3 callers (`build/providers/plans/provider_plan.rs:896`, `signature.rs:131`, `provider_policy/validation.rs:127`); `malformed_identity_offset` 1 non-test caller (`visitor.rs:90`); `callable_display_path` appears once in each of 4 other files (`calling_policy_plans/execution.rs`, `selected_application.rs`, `target_lowering/model.rs`, `provider-admission/src/admission.rs`). Comment "Canonical identity framing shared by source producers and receiving checks" (`:28`): `encoding::` is referenced in 33 matches over 15 files (grep `type_identity::encoding`).
8. Stated limits. "Source descriptions can name a toolchain declaration before its exact revision is joined." (`:1886`); "validates framing, not package authority" (`:1812`); "Schema regions describe a declaration's binder, never a live value's lifetime." (`:588`).
9. Fail-open risks.
   - `DeclarationIdentitySyntax::package_owner` returns `Ok` for any digest, `embedded_name` accepts any nonempty name, and `descriptive_toolchain_owner` is accepted (`:1835-1846`). A forged package digest or unknown owner passes `validate_trait_application`/`validate_source_type_description`. Stated: "This query validates framing, not package authority" (`:1812`) and "the caller still checks their exact identities" (`:1953`). Classification: stated.
   - `grammar::owner` accepts `ToolchainSource` with an all-zero digest (`:846`) while `Package` rejects zero (`:840-842`), and `ToolchainSource` never calls `visitor.package_owner`. Open (no comment).
   - `malformed_identity_offset` uses a visitor that never errors (`:1985-2000`) and maps only `MalformedIdentity`/`Unadmitted` to `Some` (`:2003`); an `AllocationFailed` yields `None`, which a caller reads as "not malformed". Open (caller at `visitor.rs:90` not read).
   - `closed_callable_matches` uses `abi.operands.get(..).is_some_and` (`:1791`), so a missing operand is a mismatch, not acceptance. No fail-open there.

### A2. `identities/declarations.rs` (201 lines)

1. Purpose. Holds the checked-machine record (`CheckedMachineDeclaration`) and a nominal-declaration newtype, and decides whether two such records from two different identity tables name the same declaration.
2. Shape. `NominalDeclarationIdentity(TerminalIdentityIndex)` (`:3`) with `same_identity_as` (`:14`). `CheckedMachineDeclaration` (`:36`: `machine`, `state`, `semantic_declaration`, `parameter_types`, `callable`, `result_type`, `contract: [u8;32]`, `published_may_suspend`, `published_may_block`, `specialization`), `callable_evidence_identity` (`:50`), `same_identity_as` (`:71`). `CheckedMachineSpecialization` (`:138`), `CheckedMachineCapture` (`:151`) with `same_identity_as` (`:158`).
3. Inputs/outputs. Reads `TerminalIdentityTable` text for each index; returns `bool`/`String`; no mutation.
4. How it decides. Keyed on identity text, not index: `identities.text(a) == other_identities.text(b)` (`:31`, `:83-92`). Invocation-local identities go through `same_invocation_declaration` instead (`:20-24`). `callable_evidence_identity` builds `format!("specialized-machine|callable={}:{}|application={}"..)` (`:63-68`), a string key.
5. Cases by shape. Specialization compared as both-`None` or both-`Some` (`:99-133`); capture likewise (`:103-109`).
6. Same job elsewhere. The "text equal and nonempty" test is repeated in `interfaces/reach_applications.rs:196-199` (`ClosedReachArgument::same_identity_as`) and `layouts`-free; `CheckedMachineCapture::same_identity_as` (`:166-169`) is the same expression.
7. One-caller code. Not counted separately.
8. Stated limits. Comment at `:141-143`: caller descriptions "must retain this link rather than infer an entry from the machine's spelling".
9. Fail-open risks. `NominalDeclarationIdentity::same_identity_as` refuses empty or missing text (`:25-31`), but `CheckedMachineDeclaration::same_identity_as` compares `semantic_declaration`, each `parameter_types` entry, `callable` and `result_type` with plain `text(a) != text(b)` (`:83-92`). Two records whose indices are both absent (text `""`) or both out of range (`None`) therefore compare equal on those fields. The specialization branch is stricter (`:110-120` require nonempty). Open (traced only inside this file; the callers decide what an absent index means).

### A3. `identities/mod.rs` (16 lines)
Module list and re-exports (`:3-16`), including `psi_program_identity::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker}` (`:13`). No decisions. Cases/limits/fail-open: none seen.

### A4. `identities/private_inputs.rs` (61 lines)
1. Purpose. Facts inferred from private callers during one invocation (Build) check: required-input clauses and the calls that proved them.
2. Shape. `InvocationPrivateInputs { clauses: Vec<(MachineId,u32,Proposition)>, calls: Vec<(MachineId,InvocationSite,MachineId,u32,ObligationId)> }` (`:5-8`); `from_checked` (`:12`), `is_empty`, `clause` (binary search, `:38`), `proved` (binary search, `:47`).
3. I/O. Sorts and stores supplied rows; lookups return `Option<&Proposition>`/`bool`.
4. Decides on. Ids and ordinals: clause key `(callee, ordinal)` (`:42`), call key full tuple (`:57`).
5. Cases. None.
6. Same job. Companion `InvocationPrivateResult` (A5).
7. Callers: group grep `rejoins_machine|rejoined_record|retain_invocation_*|retain_checked_private_*|proved_private_result|inferred_input_clause|proved_inferred_input` gives 38 matches in 8 files (not separately counted).
9. Fail-open. `from_checked` duplicate test for `calls` sorts by the whole tuple (`:23`) and then compares only caller, site and clause of adjacent rows (`:24-27`); two rows with the same caller/site/clause but different callee may not be adjacent. A missed duplicate is kept, not refused. Open.

### A5. `identities/private_results.rs` (26 lines)
`InvocationPrivateResult { machine, ordinal, clause, returns }` (`:6-11`); `matches` (`:14`) requires the machine's `ensures[ordinal]` to equal `clause`, `returns` nonempty, and the machine's `Terminator::Return` edges/values to equal `returns` in order (`:15-24`). Only `Return` terminators are read; `ReturnUnit`/`ReturnStructural` are ignored (`:20-22`). Decides on ids and clause equality. Fail-open: none seen.

### A6. `identities/program/identity.rs` (56 lines; crate `psi-program-identity`)
1. Purpose. A one-value vocabulary marker and a 32-byte program fingerprint.
2. Shape. `VocabularyMarker` unit struct (`:5`), `CURRENT` (`:8`), `new(raw)` (`:10`), `get()` returns 142 (`:18-21`, comment "Combined callable/source identities, typed conversions and owned cleanup."). `SemanticFingerprint([u8;32])` (`:25`), `TerminalPsiIdentity { vocabulary_marker, program_fingerprint }` (`:53`).
4. Decides. `new` accepts only raw `142` (`:11`).
9. Fail-open: none seen. Note: the number 142 here differs from the wire `FORMAT_MARKER = 290` named in the crate map; they are different markers and I did not read the wire file.

### A7. `identities/table.rs` (501 lines)
1. Purpose. The interned table of identity strings every declaration points into, plus the retained Build-invocation evidence that lets an anonymous Build declaration later "rejoin" its package-owned twin.
2. Shape. `TerminalIdentityIndex(pub u32)`, 0 = `ABSENT` (`:7-21`). `TerminalIdentityTable` (`:24`: `entries: Vec<String>`, `invocation`, `invocation_local`, `rejoin`, `records`, `private_inputs`, `private_results`, `positions` memo). `InvocationDeclarationOrigin` (`:37`), `InvocationRecordOrigin` (`:46`), `InvocationCodeScope(Arc<()>)` (`:78`, equality by pointer, `:80-84`). Methods: `same_invocation_declaration` (`:103`), `rejoins_machine` (`:123`), `rejoined_record` (`:155`), `inferred_input_clause` (`:181`), `matches_inferred_input_clause` (`:187`), `proved_inferred_input` (`:200`), `proved_private_result` (`:215`), `retain_checked_private_results` (`:228`), `retain_checked_private_inputs` (`:249`), `is_invocation_local` (`:262`), `index_of` (`:278`), `text` (`:289`), `into_builder` (`:293`). `TerminalIdentityTableBuilder` (`:309`): `for_invocation` (`:324`), `retain_invocation_machine` (`:338`), `retain_invocation_record` (`:376`), `intern_invocation_local` (`:403`), `intern` (`:423`), `build` (`:451`). `Record` impl (`:472`).
3. I/O. Builds from strings; `Record::read` rebuilds through the builder (`:491-498`); `Record::write` refuses an invocation table (`:477-481`).
4. How it decides. Text identity: `HashMap<String,u32>` position memo (`:54`, `:311`). Invocation-locality by linear `Vec::contains` (`:263`). `rejoins_machine` keys on machine/state identity plus stored `occurrence` and `contract` bytes (`:139-148`). `rejoined_record` keys on the 32-byte `occurrence` and demands exactly one successor (`:170-177`).
5. Cases. `rejoined_record` returns `Ok(None)` for unknown record or for a record that already has a package (`:160-168`).
6. Same job. The `HashMap<String,u32>` duplicate is built twice, in `index_of` (`:279-285`) and `into_builder` (`:294-299`).
7. One-caller: see A4 grep (38 matches in 8 files).
8. Stated limits. "Ordinary declaration identity and portable publication never consult this invocation evidence." (`:121-122`). Data-oriented ratchet note: string-keyed `HashMap` (not a dense-ID table) at `:54`, `:311`, plus `as u32` narrowing at `:283`/`:297` and an `expect` on overflow at `:427-428`.
9. Fail-open risks. `rejoins_machine`: `a.package.is_none_or(|owner| b.package == Some(owner))` (`:147`) accepts any package owner for a previously ownerless declaration; stated at `:119-122` ("A previously anonymous Build declaration may acquire its package owner"). `proved_private_result` returns false when rows are absent (`:216-220`). `retain_checked_private_inputs` drops an empty input set silently (`:256`). Otherwise none seen.

---

## B. `interfaces/` (4,873 lines)

### B1. `interfaces/application_demands.rs` (122 lines)
Purpose: a set of boundary-application demands bound to one `TerminalPsiIdentity`. Shape: `TerminalBoundaryApplicationDemands { terminal_psi, rows: Arc<[..]> }` (`:7`), `new` (`:13`), `validate_for_terminal` (`:30`). `new` sorts by operation id and rejects repeats (`:17-23`). Decides on `OperationId` and `TerminalPsiIdentity` equality. Fail-open: none seen. Callers: grep `TerminalBoundaryApplicationDemands|boundary_application_demands_for_machines|core_filesystem_service` gives 46 matches in 16 files (not separately counted).

### B2. `interfaces/applications.rs` (217 lines)
1. Purpose. Records how a boundary operator requirement is applied (type and const arguments) and lets a module list which demands belong to a chosen machine set.
2. Shape. `BoundaryTypeIdentity` (`:6`), `BoundaryOperatorRequirement` (`:19`, `new` refuses empty overload `:31`), `BoundaryApplicationCandidate` (`:55`), `BoundaryApplication::{Empty, Exact}` (`:65`, `exact` `:71`), `BoundaryApplicationArgument::{Type, Const}` (`:94`), `TerminalBoundaryApplicationDemand` (`:142`), `TerminalModule::boundary_application_demands_for_machines` (`:177`).
4. Decides. `exact` requires nonempty and arguments in binder ordinal order `0..n` (`:72-80`); `const_argument` refuses empty strings (`:121`). `boundary_application_demands_for_machines` demands a sorted machine list (`:181`) and a strictly ordered demand catalog (`:184-188`), then finds each demand's owner by binary search over operation ids (`:198-205`).
5. Cases. Const value identity is two `String`s (`value_type`, `value_encoding`, `:103-104`).
9. Fail-open: none seen. A demand whose operation is in no machine returns `Err` (`:210`); a demand owned twice returns `Err` (`:202-204`); comment `:193-194` says dormant code is still visited. Empty catalog returns `Ok(vec![])` before any owner check (`:190-192`), traced: nothing to attribute.

### B3. `interfaces/boundary_machines.rs` (799 lines)
1. Purpose. Declares a boundary machine (a foreign/provider/compiler-owned requirement with its full contract), plus ways to compare its result with a provider's result and to decide which obligations the checker must still discharge.
2. Shape. `StructuralDomainRequirement` (`:13`, `declared_roots` `:21`), `ScalarDomainRequirement` (`:36`), `BoundaryStructuralResultDeclaration` (`:41`), `BoundaryMachineResult::{Unit, Scalar, Structural}` (`:49`) with `accepts_provider_result` (`:56`), `matches_machine_result` (`:100`), `matches_structural_result` (`:113`), `matches_operation_result` (`:126`). `BoundaryMachineDeclaration` (`:200`, about 30 fields), `CheckedBoundaryCallable` (`:243`), `CheckedBoundaryExecution` (13 variants, `:251-271`), `matches_calling_abi` (`:326`), `same_identity_as` (`:403`), `external_completion_requirement` (`:465`), `reach_beyond_owning_supply` (`:485`), `requires_checked_result` (`:493`), `has_valid_parameter_order` (`:609`), `structural_contract_result` (`:630`), `scalar_contract_result` (`:643`), `scalar_contract_parameters` (`:655`). `find_boundary_machine` (`:695`), `program_local_root_introduction_compatibility_report_identity` (`:702`).
3. I/O. Reads domain declarations (`StructuralDomainDeclaration`, `ScalarDomainDeclaration`) and identity tables; returns booleans, derived `ValueDeclaration`s, a `u64`.
4. How it decides. Keyed on ids (`StructuralTypeId`, `StructuralDomainId`, `MachineId`), on equality of qualification lists (`:122`, `:135`), and on content of the identity table for the calling-ABI comparison (`:331-336`, `:372`). `matches_calling_abi` rebuilds a commitment with `requirement_application_commitment(text(template_callable), &arguments)` and compares to the stored commitment (`:395-400`).
5. Cases by shape. `CheckedBoundaryExecution::HostedPrimitive` equality requires both requirements to be `BoundaryRequirementKind::Trait` with `Provider` execution (`:422-444`). `matches_calling_abi` handles only `ClosedReachArgument::Type`/`Const` against `Type`/`Const|Value` operands (`:364-371`); other kinds produce `Err("boundary calling needs a complete type or value tuple")` (`:389-391`). `requires_checked_result` walks scalar membership then structural result domains (`:500-606`).
6. Same job elsewhere. Four result comparators in one file repeat the same four-field structural comparison (`:120-123`, `:133-136`, and in `accepts_provider_result` `:65-66`). `Psi::boundary_machine` (`interfaces/queries.rs:72-79`) is the same linear lookup as `find_boundary_machine` (`:695-700`); grep `find_boundary_machine`: 20 matches in 14 files. `program_local_root_introduction_compatibility_report_identity` hand-rolls 64-bit FNV-1a with offset basis `14_695_981_039_346_656_037` and prime `1_099_511_628_211` (`:711`, `:762`); other FNV-1a copies exist at `foundation/wire/src/fnv1a.rs:4`, `psi-to-native/targets/calling-conventions/src/plans/plan_identity.rs:18`, `targets/target/src/compatibility_fingerprint.rs:9`, `verification/provider-admission/src/source_evaluated_bindings.rs:332`; this unit's `conformances.rs:184` and `layout_reports.rs:207` use `wire::fnv1a::Fnv1a`.
7. One-caller: see B-group grep above; `requires_checked_result` etc. were counted together (28 matches, 16 files, with `ReferenceResultShape`, `requires_checked_non_observation`, `validate_numeric_intrinsic_signature`).
8. Stated limits. "This is a host-permission projection, never a ceiling" (`:482`); "A promise can forward an owned input claim or require a checked realization." (`:498-499`).
9. Fail-open risks. `requires_checked_result` uses `is_none_or(..)` on a missing domain declaration or missing closure (`:507-509`, `:588-591`), which yields `true` ("requires checking"), so a missing row makes the check stricter. `accepts_provider_result` returns `self.matches_machine_result(result)` for every non-structural shape (`:96`). The only acceptance of a provider result with a looser rule is the structural arm: provider qualifications may be a subset of the expected set when the omitted domains' missing memberships have no establishment routes or authorize the requirement (`:67-90`). Traced inside this file only; the authorization predicate `authorizes_boundary_requirement` is elsewhere (not read).

### B4. `interfaces/callback_application.rs` (13 lines)
`TerminalCallbackApplication` record (`:2-13`): registrar machine/operation, `requirement: String`, `calling_owner_application: String`, `published_contract: [u8;32]`, `entry_admitted`, `body: CheckedMachineDeclaration`. Data only; string-typed identities at `:7-8`. Fail-open: none seen.

### B5. `interfaces/callback_declarations.rs` (119 lines)
Purpose: arena roster of callback uses and requirements with `Result<_, String>` fields recording why a fact is unavailable. `CallbackDeclarationRoster { uses, requirements }` (`:14`), `CallbackRequirementDeclaration` (`:20`), `CallbackContractEnvelope`, `CallbackContractRefinement`, `CallbackEntryResources` (`:51`), `CallbackUseDeclaration` (`:76`), `declaration` (`:111`). Defaults for every record are `Err("... has no ...")` (`:27-36`, `:61-73`, `:91-108`). Decides nothing. Fail-open: none seen (absent facts are `Err`).

### B6. `interfaces/callback_declarations_prepared.rs` (559 lines)
1. Purpose. Keeps the projected checked callback records so native selection can match each emitted callback registration to exactly one checked use, and reports why a match failed.
2. Shape. `PreparedCallbackDeclarations` (`:30`): `declarations`, `requirements`, `occurrences`, `installed_uses`. `from_projection` (`:41`), `calling_requirements` (`:77`), `corresponding_uses` (`:101`, the matcher), `selected_roots` (`:258`). Test module (`:282-559`, 3 tests).
3. I/O. Inputs are the emitted `TerminalCallbackApplication`s with two registrar identities and an `installed` flag; two identity tables. Output is one `Vec<CallbackUseId>` (always length 1 on success) or a bounded diagnostic `String`.
4. How it decides. Candidates are the uses with the same receiving application text and `static_machine_ordinal` (`:158-172`). A candidate passes only if six checks all hold (`:177-190`): requirement overload string equal, published commitment equal, `entry.is_some() == entry_admitted`, installed flag equal, `selected_declaration.same_identity_as(...)`, registration declaration equals one of two registrar identities. Several passing candidates are collapsed only if every field except `application_index` is equal (`:215-228`); otherwise `Err("checked callback uses disagree on policy ...")` (`:220`). The lowest `(arena_index, generation)` is kept (`:229-231`).
5. Cases. Diagnostic keeps at most four candidate rows (`:198`, `detail_rows = [None; 4]` `:171`) and truncates to 1,024 bytes (`:249-253`).
6. Same job. None in unit.
8. Stated limits. "Retain at most four fixed-width rows" (`:196`); "No arbitrary identity strings enter this message." (`:247`).
9. Fail-open risks. Receiving application is read as `Ok` text only when the requirement has exactly one row (`:142-145`); otherwise it is `None` and sorts before every real application, so those uses never match (`:158`, `:162-168` compare `Option<&str>` ordering with `Some`). That refuses, not accepts. None seen.

### B7. `interfaces/calling_policies.rs` (135 lines)
Records for calling-ABI declarations: `CallingAbiDeclaration` (`:8`), `CallingStaticOperand`, `CallingAbiStorage`, `CallingPolicyRequest`, `CallingSignatureTelescope` and children. `CallingAbiDeclaration::identity` (`:27-60`) builds a `CallingAbiIdentity` from table text, refuses empty text (`:34`), validates, and requires `identity.id() == self.id` (`:54-58`). Decides on id equality of a recomputed key. Fail-open: none seen.

### B8. `interfaces/conformances.rs` (438 lines)
1. Purpose. The closed-conformance application record (a trait applied to a carrier with its rows, schemas and static applications) and its two digests.
2. Shape. `ClosedConformanceApplication` (`:5`), `ClosedConformanceSchemaKind::{Nongeneric, Finite, Open}` (`:21`), `ClosedConformanceSchema` (`:28`), `ClosedConformanceStaticApplication` (`:42`), `DeclaredResultLifetime` (`:56`), `ClosedConformanceApplicationCommitment([u8;32])` (`:62`), `ClosedConformanceRow` (`:94`), `closed_conformance_application_bytes` (`:118`, private), `closed_conformance_application_report_fingerprint` (`:180`), `closed_conformance_application_commitment` (`:335`).
3. I/O. Reads identity text and writes length-prefixed bytes into a `Vec<u8>`; fingerprint is 64-bit FNV-1a (`:184-189`); commitment is a domain digest with tag `omega.psi.terminal.closed-conformance-application.v8\0` (`:339-346`).
4. Decides. No judgment; output depends on spelling of identity text.
5. Cases. `callable_result_bytes` is a fixed match (`:192-234`): `ScalarType::Boolean`, `Callable`, `Integer`, `IeeeFloat` only; `(Address, Signed)` is `unreachable!` (`:215-217`).
7. Callers: not counted separately.
9. Fail-open risks. `identity!` writes `identities.text(i).unwrap_or_default()` (`:130`, `:168`, `:174`, `:255`), so an absent or out-of-range identity hashes as the empty string. Two applications that differ only in which absent identity they hold get the same digest. Open (consumers not read).

### B9. `interfaces/declarations.rs` (21 lines)
`ProgramInterfaces` struct, 16 `Vec` fields (`:4-20`). Data only.

### B10. `interfaces/external_providers.rs` (117 lines)
`ExternalProviderConformance` (`:23`) and `validate` (`:38-116`): refuses a sealed intrinsic that also has a numeric intrinsic or a non-`CompilerIntrinsic` binding (`:45-50`); numeric without schema (`:51-53`); incomplete schema (`:54-60`); empty target or numeric meaning without a signature (`:61-67`); any empty identity text or zero contract in the realization (`:68-83`); same for specialization (`:84-101`); selecting provider without text (`:102-106`); empty `VtableField`/`TableFunction` field (`:107-115`). Decides on presence (`!text.is_empty()`, `:40-44`). Fail-open: none seen (all refusals).

### B11. `interfaces/installed_interrupt.rs` (19 lines)
`TerminalInstalledInterruptRegistration` record with several `String` identities (`:10-14`) and `callbacks: Vec<TerminalCallbackApplication>` (`:18`). Data only.

### B12. `interfaces/mod.rs` (64 lines)
Module list and re-exports; header comment (`:1-4`). No logic.

### B13. `interfaces/non_observation.rs` (97 lines)
`BoundaryMachineDeclaration::requires_checked_non_observation` (`:8-84`): true if any structural parameter has `WriteOnlyBorrow` access (`:12-17`) or if a non-erased field/element reaches a write-only reference, element view, or byte-sequence carrier (`:45-79`). Walk uses `BTreeSet<StructuralTypeId>` for visited (`:24`; ratchet note: ordered set keyed by a dense id) and `pending` stack. Cases: `Reference`, `ElementView`, `ByteSequence`, `Record`, `Sum`, `Mixed`, `FixedArray` with nonzero length (`:46-79`), `_ => {}` for the rest (`:80`). Fail-open risk: `let Some(declaration) = lookup(current) else { continue }` (`:29-31`) skips a type that cannot be looked up and so can return `false` (no check needed) when a type is missing from the lookup. Open (callers: grep count above, `interfaces/non_observation.rs` 1 match, not traced).

### B14. `interfaces/numeric_intrinsics.rs` (43 lines)
`validate_numeric_intrinsic_signature` (`:1-43`): returns `Ok(())` when `boundary.numeric_intrinsic` is `None` (`:6-8`). Otherwise requires callable role, normal return, no attachment, no structural parameters, matching scalar parameter count and type, and a result matching the signature (`:12-41`); only `Scalar`+`Primitive`, `Scalar`+`Boolean`, `Structural`+`FloatClass` result pairs are allowed (`:24-37`). Fail-open: the `Ok(())` at `:7` is for non-numeric boundaries (traced by name: the check applies only to numeric intrinsics). Callers: 2 in `provider-admission/src/admission.rs`, 1 in `verification/numeric-intrinsics/signature.rs` (from the grouped greps, not separately confirmed which name).

### B15. `interfaces/providers.rs` (89 lines)
Records for provider requirements and candidate conformances (`ProviderInstanceRequirements`, `RequiredProviderInstance`, `ProviderCandidateConformance`, `ProviderSignature`, `ProviderRefinement`). Data only; `empty()` constructors (`:52`, `:82`).

### B16. `interfaces/queries.rs` (90 lines)
`impl Psi` lookups: `callback_application` (binary search on `application_index`, then fall back to scanning installed-interrupt registrations' callbacks, `:5-21`), `boundary_calling_abi` (binary search by boundary and refuses a duplicate neighbour, `:26-48`), `calling_abi_identity` (`:50`), `calling_abi_signature` (binary search, `:58`), `service` (linear, `:68`), `boundary_machine` (linear, `:72`), `installation_reach_requirements` (`:83`). These binary searches rely on sorting stated in comment `:30-31`: "Source publication sorts this roster; the receiving foundation check requires increasing boundary IDs." Fail-open: if the roster were unsorted, `binary_search` could miss and return `Err`; none seen that accepts.

### B17. `interfaces/reach_applications.rs` (201 lines)
`closed_reach_template_commitment` (`:9-59`): digests per-binder bounds (domain tag `omega.machine-template-binder-bounds.v1\0`, `:27`), requires every upper-bound service to have a nonempty identity (`:40-43`), sorts identities (`:45`), then calls `psi_effect_contracts::template_contract_commitment` (`:52`). Types `ClosedReachApplication` (`:62`), `ClosedReachCapture`, `ClosedReachParameter::{Type, Const, Proposition, Machine}` (`:105`), `ClosedReachMachineBinding` (`:119`, 13 fields), `ClosedReachSchema`, `ClosedReachCall`, `ClosedReachArgument` (`:163`) with `same_identity_as` (`:174`: kind must match; `Machine` also needs equal `contract_commitment`; text must be equal and nonempty, `:196-199`). Comment: "Omission defers to the selected callable. An empty row is an explicit bound." (`:123`). `parameter(position)` uses `nth` (`:99-101`). Fail-open: none seen.

### B18. `interfaces/result_regions.rs` (85 lines)
Trait `ReferenceResultShape` (`:10`) with two implementations that are line-for-line the same: `BoundaryMachineDeclaration` (`:26-55`) and `TerminalDynamicSlot` (`:56-85`). `reference_regions` filters regions by `region.path == path` (`:41`, `:71`). Same job twice inside the file. Callers: group count 28 matches in 16 files (not separate).

### B19. `interfaces/service-surface/commitment.rs` (323 lines; crate `service-surface`)
1. Purpose. Writes a service contract in a fixed byte order and hashes it ("v9").
2. Shape. Trait `ServiceContractSink` (`:7`) for `DomainDigestEncoder` and `Vec<u8>` (`:11-20`); `declared_type_signature` (`:24`); `ServiceSurface::declaration_commitment` (`:47`, tag `omega.service-schema.sha256.v9\0`); `encode_contract` (`:56`); private `ContractEncoder` (`:102`) with `service_method` (`:155-277`).
4. Decides. Sorts methods by name then requirement identity text (`:81-86`), entry claims by `(parameter_index, carrier text, domain)` (`:166-172`), result claims by domain (`:202`), premises by `(profile, subject, projections)` (`:244-249`), routes by `(kind, requirement text)` (`:262-267`). `declared_type_signature` refuses an empty result or empty parameter identity (`:28-30`, `:35-37`).
5. Cases. `BoundaryCompletion` is written as `NormalReturn => 0`, `External(ProcessExit) => 1` (`:188-193`); `ServiceEntryAuthorityFlow::Accepts => 0` (`:183-185`); `ServiceProgressSubject::Parameter` only (`:253-258`). Comment on method order: "The trailer pairs each declaration's canonical type signature with the same sorted method roster" (`:92-93`).
9. Fail-open risks. `identities.text(..)` returns `""` for an invalid id (see `service_surface.rs:54-60`), so an unset identity is hashed as an empty string without error (`:159`, `:162`, `:169`). Open. Tests at `:280-323`.

### B20. `interfaces/service-surface/segment_tests.rs` (168 lines)
Test-only module (`#[cfg(test)] mod segment_tests`, `service_surface.rs:528-529`). It compares current segment types with copies of historical enums for ordering, hashing and `Record` encoding (`:9-26`, `:60-79`). No production logic. Comment `:9`: "Reference declarations from 1b54028582c."

### B21. `interfaces/service-surface/service_surface.rs` (529 lines)
1. Purpose. Service-surface records (trait, receiver applications, methods with claims, reach and termination premises) and equality that compares identity text rather than ids.
2. Shape. `mod identities` with `ProviderIdentityId(u32)`, `ProviderIdentityTable` (`Vec<Box<str>>` + `HashMap<Box<str>, ProviderIdentityId>`, `:33-65`); `ServiceSurface` (`:70`), `ServiceReceiverApplication` (`:79`), `ServiceSynchronousInvocation` (`:135`) with `validate_entries` (`:141`), `ServiceMethod` (`:163`, 18 fields), `eq_in` (`:188`), `impl PartialEq for ServiceSurface` (`:264`), `declaring_receivers` (`:287`), `matches_receiving_application` (`:305`), `selection_slot_identity` (`:333`), `validate_receiver_applications` (`:353`), `ServiceProgressPremise`, `ServiceEntryClaim`, `ServiceResultProjection` (alias of `StructuralPlaceSegment<true>`, `:496`), `ServiceNormalResultMembership` (`:508`) with `has_exact_identity` (`:515`).
4. Decides. Text equality across two tables (`left.text(a) == right.text(b)`, `:95`, `:198-199`). `matches_receiving_application` finds the method by requirement text (`:310-314`), requires a declaring receiver when receivers exist (`:318-320`), accepts `receiving == None` only if all declaring receivers have no arguments (`:321-326`), refuses an empty `Some("")` (`:327-329`), else any receiver whose application text equals (`:330`).
5. Cases. `validate_receiver_applications` checks root equals schema owner (`:357-360`), root lists exactly the method requirements (`:362-370`), each application has nonempty identities (`:372-380`), is a subset of method requirements (`:381-389`), sorted/unique (`:390-396`), and not repeated (`:397-418`).
6. Same job elsewhere. `service_schemas.rs:156-214` re-validates methods in `TerminalModule::service_schema`.
8. Stated limit: none; one `Default` on `ServiceSurface` gives an empty surface (`:69`).
9. Fail-open risks. `ProviderIdentityTable::text` returns `""` for the invalid id or an out-of-range id (`:54-60`). `ServiceSurface::eq` compares the trait names and methods with this accessor (`:274`), so two surfaces whose differing identities are both unset compare equal. `eq_in` for `ServiceMethod` compares `result_claims` and `normal_result_memberships` by derived `PartialEq` over `String`s (`:225-226`). Traced within file.

### B22. `interfaces/service_schemas.rs` (604 lines)
1. Purpose. Holds the service-schema, program-entry and provider-selection rows on a `TerminalModule` and checks each lookup is exact before returning it.
2. Shape. `TerminalSealedNativeIntrinsic` (`:9`), `TerminalServiceSchema` (`:15`) with `join` (`:25`), `TerminalProgramEntryContract` (`:64`), `TerminalProviderSelection` (`:75`), `TerminalModule::core_filesystem_service` (`:92`), `service_schema` (`:156`), `program_entry_contract` (`:216`), `provider_selection` (`:243`), `read_core_filesystem_declaration` (`:285`). Tests at `:373-604`.
4. Decides. `service_schema`: exactly one row for the pair, nonempty identity text, `root_application_identity() == application text`, `validate_receiver_applications`, per-method `validate_entries` and 11 completeness conditions (`:161-212`). `core_filesystem_service` accepts a service only when the declaration is a `ToolchainSource` nominal at the exact bundled core revision `core_revision::FILESYSTEM_AUTHORITY` (`:337-354`), no parents (`:104`), exactly one receiver application with no arguments, no methods (`:138-148`) and the schema application text equals the rebuilt toolchain application text (`:141`). `provider_selection` checks custody pair `(Package(owner), Some(package))` with `owner == package`, or `Toolchain`/`ToolchainContract` with `None` (`:269-280`).
5. Cases. Only filesystem services in `FilesystemService::ALL` are recognised (`:340`). `join` coalesces equal surfaces and refuses a duplicate existing row or a changed contract (`:41-52`).
6. Same job. The 128-depth visitor duplicates `DeclarationIdentitySyntax` (A1 item 6).
8. Stated limits. "neither a familiar name nor successful parsing grants host access" (`:90-91`).
9. Fail-open risks. none seen; every branch returns `Err`. `join` keeps the first row of an equal group (`:53`) after equality, traced.

### B23. `interfaces/services.rs` (21 lines)
`TerminalRootServiceReach`, `InstallationReachDependency`, `ServiceDeclaration` (`:5-21`). Data only.

---

## C. `layouts/` (3,294 lines)

### C1. `layouts/declarations.rs` (5 lines)
`LayoutDeclarations { target_layout_applications, native_layouts }` (`:2-5`). Data only.

### C2. `layouts/geometry/geometry.rs` (22 lines; crate `layout-plans`)
`StorageGeometry { byte_size, byte_alignment }`, `ConventionalSumGeometry` (`:8`), `AggregateGeometryError { EmptyCaseSet, ZeroAlignment, SizeOverflow }` (`:18`). Data only.

### C3. `layouts/geometry/layout.rs` (1,450 lines)
1. Purpose. Placement constraints for materialized storage, symbolic (unresolved) field values, per-field inner layouts, private-callback placement checks, and a test-support module.
2. Shape. Macros `numbered_constructor!`/`numbered_variant!` (`:6-31`). `mod placement` (`:38`): `ByteOrder`, `ConsumptionInstant`, `PlacementPhase`, `PlacementAddressRange` (`:63`, `new` refuses empty/reversed, `:69-82`), `MachineRegimeId`, `ArtifactInstallationScopeId`, `PlacementConstraints` (`:130`, `new` `:139`, `joined_with_layout` `:190`, `validate_site` `:223`), `PlacementSite`, `MaterializationContext` (`:275`), `MaterializationWrite`, `StoredIntegerFit`, `MaterializationAction` (`:302`). `mod private_callback_placements` (`:330`): `private_address_fragment` (`:334`), `validate_private_callback_placements` (`:375`), `compose_private_callback_placements` (`:470`). `mod symbolic_values` (`:629`): `EntryStubId`, `DataSymbolId`, `RelocationTarget`, `SymbolicFieldPathSegment` (`:680`), `SymbolicFieldValue` (`:728`), `SymbolicFieldInterior`, `SymbolicFieldInteriorLayout`, `SymbolicFieldInnerLayout` (`:821`), `from_recursive_sum_paths` (`:917`). `pub mod test_support` behind `cfg(any(test, feature="test-support"))` (`:1078-1413`). `mod target_facts` (`:1415`): `LayoutTargetFacts::validate` (`:1426`). Re-exports (`:1045-1450`).
3. I/O. Consumes `LayoutPlacementReport` lists and reports; produces ranges `(start_bit, end_bit)`, `ScalarFragment`s or diagnostics. No mutation of storage.
4. Decides. Numeric comparisons only: `validate_site` compares phase, `is_multiple_of(alignment)`, range containment, machine regime and scope (`:228-261`). `validate_private_callback_placements`: whole-pointer case requires `storage_alignment >= pointer_alignment`, offset aligned and end within storage (`:386-401`); fragment case requires source bits `[0, pointer_size*8)` exactly covered (`:448-461`) and destination ranges non-overlapping (`:463-466`).
5. Cases by shape. Private callback slot must be one whole `At` pointer or a full set of `Bits` fragments; `IntegerAt` is refused with "narrowed IntegerAt storage requires an address-fit contract" (`:410-419`). `private_address_fragment` returns `None` for `IntegerAt` (`:353`). Pointer size at most 8 and alignment a power of two (`:382`). Recursion limit `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` (64, `layout_reports.rs:166`) applied at `:968`.
6. Same job elsewhere. Fragment validation duplicates the shape in `materialization_records.rs:722-765` (`validate_fragment`), which this file calls (`:362`, `:424`). Fingerprint/ID newtypes with the same `nonzero_identity` constructor appear 4 times (`:106-127`, `:641-662`).
7. Unused or one-caller. Grep `VocabularyMarker::new|layout_plan_reports_match_for_replay|conventional_sum_layout_reports_match_for_replay|private_address_fragment|validate_plan_inputs|compose_private_callback_placements|validate_private_callback_placements` gives 73 matches in 30 files (not separately counted); `layout.rs` itself accounts for 21.
8. Stated limits. "One pointer fragment written by a callback or post-handoff helper." (`:333`); the `test_support` feature is not part of default build (`:1078`).
9. Fail-open risks. `PlacementConstraints::joined_with_layout` checks that the materialization length fits the range size but not that any aligned start exists inside the range (`:206-219`). `LayoutTargetFacts::validate` does not check `little_endian` or target-profile spelling (`:1426-1439`). `private_address_fragment` returns `None` rather than an error on any failure (`:353`, `:357-371`); callers read `None` (callers in `calling_policy_plans/callback_layouts.rs`, `target-*/callback_geometry.rs` are not read). Open.

### C4. `layouts/geometry/layout_reports.rs` (543 lines)
1. Purpose. Normalized layout-plan reports, 64-bit fingerprints of them, and replay-equivalence comparisons.
2. Shape. `IntegerInterpretation` (`:11`) with `stored_bounds` (`:17`), `LayoutPlacementReport::{At, IntegerAt, Bits}` (`:29`), `LayoutFieldEntryReport`, `LayoutPlanReport` (`:61`), `ConventionalSumLayoutReport` (`:87`) and 8 related report types (`:70-164`), `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT = 64` (`:166`), `PrivateCallbackLayoutDemandReport<S>` (`:182`), `NativeLayoutPlanReport<S>` (`:191`), `normalized_layout_plan_report_fingerprint` (`:196`), `normalized_conventional_sum_layout_report_fingerprint` (`:269`), `conventional_sum_layout_reports_match_for_replay` (`:308`), `normalized_native_layout_plan_report_fingerprint` (`:420`), `layout_plan_reports_match_for_replay` (`:457`), `layout_placement_sort_key` (`:508`).
4. Decides. Member identity (`member_identity: Option<u64>`) if present, else field name: `member_sort_key` (`:501-506`), `(current.member_identity.is_some() || current.field == retained.field)` (`:496`, `:333`, `:348`, `:358`). Replay match calls `validate_materialization_field_identities` first (`:461-462`) and `conventional_sum_member_identities_are_unambiguous` (`:312-313`).
5. Cases. Fingerprint tag strings `omega.layout-plan.v3` (`:208`), `omega.conventional-sum-layout.v1` (`:273`), `omega.native-layout-plan.v2` (`:431`); `.max(1)` avoids 0 (`:266`, `:305`, `:454`).
6. Same job elsewhere. FNV-1a copies: see B3 item 6. The unambiguity check at `:367-404` repeats the same "prior row with same identity" scan three times.
9. Fail-open risks. `IntegerInterpretation::stored_bounds` shifts by `stored_width - 1` with no range check (`:19-23`); `stored_width == 0` underflows and `>= 128` overflows. Reached from `stored_value_fits` (`materialization_records.rs:562`) after `byte_sized_width` (1..=64) is checked at `:590` and `:677`, so traced safe on those paths. The 64-bit FNV report fingerprints are not collision-resistant; whether they authorize anything is outside this unit (open).

### C5. `layouts/geometry/materialization_records.rs` (794 lines)
1. Purpose. Records and checks for materializing symbolic fields into byte storage: field-key identity, scalar/aggregate field schemas and values, stored-integer fit, fragment validation.
2. Shape. `mod field_identities` (`:1`): `MaterializationFieldKey::{Numbered, Positional}`, `planned_entries_by_key` (`:26`), `SuppliedFields` (`:49`), `index_supplied_fields` (`:76`), `validate_supplied_field_coverage` (`:110`), `validate_materialization_field_identities` (`:147`), `validate_conventional_sum_materialization_identities` (`:174`), `symbolic_path_display`, `symbolic_path_hops`. `mod field_values` (`:260`): `validate_authored_width` (1..=64, `:264-273`), `ScalarFieldValue`, `AggregateFieldSchema`, `AggregateFieldValue`, `ScalarFieldSchema`. `mod stored_integer_checks` (`:446`): `ScalarFragment`, `scalar_fragment`, `materializer_scalar_window` (`:478`), `validate_stored_integer_value`, `byte_sized_width`, `stored_value_fits` (`:556`), `validate_stored_integer_fit` (`:632`), `validate_stored_integer_fit_shape` (`:662`), `signed_bits` (`:685`), `validate_write` (`:694`), `validate_fragment` (`:722`), `low_mask` (`:767`). Root: `SymbolicMaterializationPlan` (`:778`), `MaterializationDiagnostic(pub String)` (`:786`).
4. Decides. Field key is `Numbered(member_identity)` if present, else `Positional(name)` (`:16-24`). Both a by-name map and a by-key map must be duplicate free (`:88`, `:99`). Coverage requires every layout entry supplied and every supplied field planned (`:116-143`). Container width must be a nonzero multiple of eight up to 64 (`:511`, `:731-734`).
5. Cases. Stored-integer placement: stored width must be byte sized, not wider than source (`:590-601`, `:677`). `AggregateFieldShape::{Whole, Repeated}` (`:322`).
6. Same job. `validate_fragment` (`:722`) vs the width checks in `materializer_scalar_window` (`:511-531`) overlap but differ (`:742` requires `source_lsb + width <= 64`, `:519` requires `<= access_width_bits`).
9. Fail-open risks. `validate_stored_integer_bits` (`:570-619`) checks `source_width_bits < stored_width` but not `source_width_bits <= 64` or `> 0`; `signed_bits` then evaluates `1_u64 << (width - 1)` and `low_mask(width)` (`:685-692`, `:767-773`) which panic for width 0 or above 64. The caller `validate_stored_integer_value` passes `value.width_bits` from `ScalarFieldValue`, whose constructor limits width to 1..=64 (`:290`) but whose field is `pub` (`:280`). A panic, not an acceptance. Open.

### C6. `layouts/geometry/packing.rs` (99 lines)
`conventional_sum_geometry` (`:4`), `pack_fields_at` (`:48`), `conventional_record_geometry` (`:75`), `valid_alignment` (`:81`), `align_up` (`:89`). Alignment is accepted when nonzero; a power of two is not required (`:81-87`, `:89-98`). Overflow returns `SizeOverflow`. Case: sum layout puts the tag at offset 0, common fields after it, payload at `align_up(common size, max payload alignment)` (`:17-24`). Same job elsewhere: `layout-planning/integer_defaults.rs` re-exports these (`:4`). Fail-open: none seen; non-power-of-two alignment is accepted (open: callers that supply alignments not read).

### C7. `layouts/mod.rs` (8 lines)
Module list, re-export `LayoutDeclarations` (`:3-8`).

### C8. `layouts/native_layouts.rs` (152 lines)
`NativeLayoutCatalog` with three arenas (`:8-12`), `NativeRecordLayout` (`:15`), `NativeLayoutFieldIdentity`, `NativeLayoutRealization` (`:53`). `record` is a linear scan (`:81-83`). `for_target` (`:124-151`): a record with absent `policy_identity` must have no realizations and returns `Ok(None)` (`:130-135`); otherwise exactly one realization whose `target` is `None` or equals the target text must exist (`:136-150`). `Default` values use `StructuralTypeId::new(1).expect("arena sentinel")` (`:33`, `:46`). Decides on string equality of target name (`:140`). Fail-open: a realization with `target == None` matches every target (`:138-140`); no comment states why. Open.

### C9. `layouts/record_storage.rs` (132 lines)
`ForeignRecordStorage` (`:7`), `foreign_record_storage` (`:31`), `foreign_opaque_carrier` (`:69`), `StorageGeometryError` (`:88`, 15 variants), trait `RecordStorageResolver` (`:113`). Decides: `TargetLayout(application)` is found by `id` and `record` (`:44`) and a realization by `row.target == target` (`:50`); `AccessView`/`Accessor` are refused (`:61-63`); a quotient-carrier type without a selected opaque row is refused (`:79-82`). `ForeignRecordStorage::field` with `fields: None` falls back to `field.storage.clone()` (`:26`) and with `Some` requires a row (`:20-25`). Callers: grep 14 matches in 4 files (`boundary_values.rs` 3, `layouts/record_storage.rs` 6, `psi/semantics/src/lib.rs` 2, `calling_storage.rs` 3). Fail-open: none seen.

### C10. `layouts/target_layouts.rs` (89 lines)
Records: `LayoutApplicationId(pub u32)`, `StructuralRecordStorage::{Packed, AccessView, Accessor, Authored, TargetLayout}` (`:10`), `TargetLayoutApplication` (`:19`), `TargetLayoutSchemaArgument`, `TargetLayoutSchemaField` (`:51`), `TargetLayoutInput`, `TargetLayoutRealization`, `TargetLayoutFieldStorage`. Data only.

---

## D. `machines/` (4,110 lines)

### D1. `machines/atomic.rs` (298 lines)
`AtomicAccessEvent<Operand>` with 6 variants (`:15`), `AtomicReadModifyWrite` (5 ops, `:6`). Accessors: `access_operation` (`:51`), `replacement`, `ordering_plan`, `ordering_is_legal` (`:112`), `observes_resident`, `modifies_resident`, `operands`, `try_map`, `map_operands`. Decides on ordering legality via `valid_for_load`/`valid_for_store`/`valid_compare_exchange_failure` from `language_core` (`:114`, `:115`, `:122`); `ReadModifyWrite` and `Swap` are always legal (`:116`). `observes_resident` is `!Store`; `modifies_resident` is `!Load` (`:127`, `:131`). Same-job: `RegisterAccessEvent` has the same try_map/map_operands/operands shape (D13). Fail-open: none seen. Tests `:226-298`.

### D2. `machines/block.rs` (29 lines)
`Block` struct (`:7`) with `id`, `parameters`, `erased_scalar_formals`, `erased_proof_formals`, `structural_parameters`, `operations`, `terminator`; `Block::plain` (`:18`). Data only.

### D3. `machines/call_contexts.rs` (74 lines)
`CallSite`, `InvocationOwner::{Operation, InvokeNoNormal, Cleanup}` (`:16`), `CallOrigin`, `CallAcknowledgement` (`:46`) with `generated()` (`:58`) which sets every acknowledgement flag true, `CallFormation` (`:69`). Header comment (`:1-3`): "A redundant marker creates no pause." Data only.

### D4. `machines/direct_calls.rs` (104 lines)
`DirectCallRole::{Unit, Scalar, Structural, NoNormal}` (`:5`), `DirectCallParts` (`:13`), `direct_call_parts` (`:21`), `call_structural_arguments` (`:92`). Shape: only five `OperationKind` variants map (`Call`, `CallUnit`, `CallStructuralScalar`, `CallStructural`, `CallStructuralWithScalarArguments`); `_ => return None` (`:81`). `Call` gets role `Scalar` and empty structural arguments (`:23-30`). `DirectCallRole::NoNormal` is never produced here; grep shows it is created in `psi/semantics/src/calls/evidence_calls.rs:101` and `evidence_calls/terminal_inputs.rs:134`. Callers of `direct_call_parts`: 10 non-test sites in `psi/semantics`, `interpreter`, `terminal_flow.rs:804`, `borrowed_storage.rs:123` (grep). Fail-open: a call-like kind not in the list (`CallMachine`, `CallDynamic`, `BoundaryCall`) returns `None` from `direct_call_parts`; `call_structural_arguments` covers `BoundaryCall` and `CallDynamic` separately (`:94-101`) but not `CallMachine` (which the macro at `operation_patterns.rs:31` does cover). Whether a caller treats `None` as "no call" was not traced. Open.

### D5. `machines/effect-contracts/carry.rs` (198 lines; crate `psi-effect-contracts`)
Generic `PermissionEventSource<Identity>` (6 variants), `PermissionProvenance<Identity>` (`Unknown`, `Established`, `Joined`), `PermissionClaimIdentity`, `PermissionArrival`, `SuspensionValueOrigin` (`Parameter`, `Local`, `CallArgument`), each with `map_declarations` (`:66`, `:97`, `:109`, `:139`, `:177`). Default of `PermissionProvenance`/`PermissionClaimIdentity` is `Unknown` (`:7`, `:30`, `:49`). Data and mapping only. Fail-open: none seen (`Unknown` is refused by `commitment.rs:164-172`).

### D6. `machines/effect-contracts/commitment.rs` (198 lines)
`validate_published_dependency` (`:3`): refuses `concrete && !parameters.is_empty()`, unsorted/duplicate parameters, parameters `>= parameter_count` (`:7-17`). `template_contract_commitment` (`:22`): validates both dependencies, requires carry rows strictly sorted (`:34-36`), tag `omega.machine-template.v4\0` (`:38`), and per carry row requires nonempty value type, a nonconcrete nonempty condition, sorted claims (`:56-62`). Helpers `subject`, `permission_source`, `arrival` (`:88-198`) require nonempty identity strings (`:81-84`), an `Established` claim identity (`:157-165`) and a non-`Unknown` provenance (`:171-173`). Decides on string content. Fail-open: none seen. Callers: `closed_reach_template_commitment` (`reach_applications.rs:52`) and test (`tests.rs:44`); other callers not counted.

### D7. `machines/effect-contracts/effect_contracts.rs` (85 lines)
Crate root: re-exports (`:6-11`), `CapabilityFlowKind` (5 variants, `as_str`, `:15-34`), `ContractDependency<C,P>` (`:43`) with `parameter` and `possible` (`:71-82`), `ConditionalCarryRequirement` (`:52`). `possible()` is `concrete || !parameters.is_empty()` (`:80`). Data only.

### D8. `machines/effect-contracts/tests.rs` (109 lines)
Two tests: commitment changes when the guard, subject, claim ordinal or provenance changes and refuses an out-of-range parameter, `Unknown` claim or provenance, and duplicate rows (`:1-93`); `CapabilityFlowKind::as_str` names (`:95-109`). Test-only.

### D9. `machines/float_result_policy.rs` (55 lines)
`FloatResultPolicy::{Trapping, Saturating}` (`:4`) with `raw_result`, `can_trap`, `operands`, `accepts_operand_types` (`:32-42`), `map_operands`. `accepts_operand_types` requires the raw result to be `ScalarType::IeeeFloat(format)` and, for saturating, `eligible` to be Boolean (`:37-41`); `type_of` returning `None` is a refusal. Fail-open: none seen.

### D10. `machines/generic_calls.rs` (27 lines)
`NominalConstArgument`, `NominalCallArgument` (`:8`, `:14`), comment: "adjunct checking inputs, never members of a published Terminal module" (`:1-2`). Data only; fields identity `TerminalIdentityIndex`.

### D11. `machines/instructions.rs` (55 lines)
`impl Psi`: `pending_normal_completions` (`:7`), `blocks` (`:21`), `operations` (`:25`), `machine` (linear `find`, `:31`), `can_end_the_process` (`:38`: any block ends in `ExternalCompletion`), `unoptimized_machine_behavior` (`:47`). Comment (`:35-37`): a native host sees a process exit "only as an exit status, which a normal return can also produce". Callers: grep `pending_normal_completions|can_end_the_process|unoptimized_machine_behavior` 22 matches in 13 files. Fail-open: none seen.

### D12. `machines/invocations.rs` (574 lines)
1. Purpose. Uniform views of every kind of call in a machine (boundary, callable, direct/module, terminator, cleanup) so consumers need not match 14 operation kinds each time.
2. Shape. `BoundaryInvocation` (`:9`), `BoundaryInvocationCompletion` (`:20`), `Block::boundary_invocations` (`:29`), `TerminalMachine::boundary_invocations` (`:80`), `CallableInvocationInputs` (`:88`), `OperationKind::callable_invocation` (`:107`), `ModuleInvocationInputs` (`:151`), `OperationKind::declared_invocation` (`:187`), `module_invocation` (`:215`), `Terminator::declared_invocation` (`:323`), `Terminator::module_invocation` (`:362`), `ModuleInvocation` (`:394`), `Block::module_invocations` (`:401`), `TerminalMachine::module_invocations` (`:460`). Tests `:467-573`.
4. Decides by variant. `module_invocation` handles `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructuralWithScalarArguments`, `CallStructural` (`:226-299`); others `None` (`:300`). Cleanup calls are found by position: `InvokeOwned` at index `i` in a roster becomes `InvocationSite::Cleanup { edge, action_ordinal: i }` (`:432-452`); ordinal conversion is `u32::try_from(..).expect(..)` (`:446-448`).
5. Cases. `ModuleInvocationInputs` has 10 fields; boundary and no-normal calls fill `erased_*` and `claim_transfers` with `&[]` (`:202-209`, `:344-350`).
6. Same job elsewhere. `direct_calls::direct_call_parts` (D4) and `OperationKind::direct_callee` (`operation_arguments.rs:6`) cover overlapping call-kind lists; the call-kind list is written out in `operation_patterns.rs:4-35`, `direct_calls.rs`, `operation_arguments.rs:8-12,212-217,285-298` and here.
9. Fail-open: none seen. `Operation::module_invocation` overrides `required_contract` from the operation (`operation.rs:47-51`).

### D13. `machines/machine.rs` (150 lines)
`TerminalMachine` (`:10`): `call_formations`, `assembly_target_requirement`, `id`, `attachment`, `ranked_scc`, `structural_places`, `block_entry_claims`, `loan_completions`, `content_identity_reshuffles`, `content_partition_compositions`, `entry`, `blocks`, `contract`. `Deref`/`DerefMut` to `MachineContract` (`:30-42`). Lookups: `block`, `operation`, `structural_place`, `producing_operation`, `structural_parameter`, `parameter` are linear scans (`:85-140`). `structural_place_type` (`:65`). `entry_proposition_context` (`:45`). Same-job: `structural_parameter` and `parameter` are two names for one function (`:124-132`). Fail-open: none seen. Data-oriented note: every lookup is `Vec::iter().find` by id.

### D14. `machines/mod.rs` (61 lines)
Module list, `ExecutionSite` (`:29`), re-exports. Comment (`:25-27`): a block coordinate "names its terminator ... never names block entry or one chosen edge."

### D15. `machines/operands.rs` (96 lines)
`StructuralStoreDisposition::{Vacant, ReplaceDiscardable}` with `required_access` (`:11-17`: `MutableBorrow`/`WriteOnlyBorrow`), `ByteSequenceOperand::{Whole, Field}` (`:20`) and accessors, `ElementViewRegion` (`:79`), `ReferenceStorageView` (`:93`). Data and accessors.

### D16. `machines/operation.rs` (643 lines)
1. Purpose. The executable instruction set: `Operation` and the `OperationKind` enum.
2. Shape. `Operation` (`:19`: `static_reach_binding`, `required_contract`, `suspension_crossing`, `id`, `result`, `kind`), `unsuspended` (`:32`), `module_invocation`/`declared_invocation` (`:47`, `:53`). `StorageLocationQuery::{Address, Alignment}` with `admits_result` (`:65-85`). `OperationKind` (`:88-643`), about 110 variants (109 counted by grep of `^    Name {` plus `ActivateTaskRegister` on the enum line).
4. Decides. `StorageLocationQuery::admits_result` accepts `Address` for address-carrier integers and `Alignment` only for non-address unsigned 64-bit integers (`:76-82`).
5. Cases by shape. Variant families: task/segment registers (`:88-113`), storage address/alignment, references, activation claims (`:145-171`), structural/byte-sequence/element-view operations (`:172-365`), calls (`:372-457`), port/register, IEEE float, about 30 integer operations (exact/wrapping/saturating × add/sub/mul/div/rem/shift), `TrappingInteger`, `AtomicAccess`, `AtomicFence` (`:631-642`). Header oddity: the enum opens on the same line as its first variant (`:88`).
6. Same job elsewhere. Variant lists are enumerated again in `operation_arguments.rs:42-272` (`map_scalar_uses`), in `unit_validation.rs` of `optimization-unit-semantics` and the `operation_semantic_row` catalog in `psi/semantics/src/operations.rs` (suspected, not read).
9. Fail-open: none seen (data).

### D17. `machines/operation_arguments.rs` (315 lines)
`OperationKind::direct_callee` (`:6`), `constructor_arguments` (`:17`), `map_scalar_uses` (`:34-274`), `structural_arguments` (`:276`), `claim_transfers` (`:283`), `structural_arguments_mut` (`:307`). `map_scalar_uses` is an exhaustive `match` with no `_` arm (`:42-273`): every `OperationKind` variant is listed, so adding a variant is a compile error. It also rewrites `RuntimeIndex` path segments first (`:35-41`) via `structural_projection_paths_mut` (defined elsewhere). `claim_transfers` returns claim transfers for 6 call kinds (`:285-302`; `Call`, `BoundaryCall` have none) and `&[]` otherwise (`:303`). Fail-open: none seen. The `ActivateTaskRegister`/`ReloadSegmentDescriptors` selector is remapped (`:199-200`) but their `gdt`/`scratch` structural arguments are not scalar uses.

### D18. `machines/operation_patterns.rs` (97 lines)
Eight exported macros (`static_call_structural_argument_variants`, `call_structural_argument_variants`, `integer_arithmetic_operations`, `move_or_store_structural_field_pattern`, `structural_leaf_copy_pair_pattern`, `call_structural_pair_pattern`, `integer_less_or_equal_pair_pattern`, `element_view_establish_pair_pattern`), each a fixed list of `OperationKind` variants (`:2-97`). Cases: `integer_arithmetic_operations` lists 15 kinds (`:41-55`) and omits `TrappingInteger` and the bitwise/shift operations. Same-job: the same call-kind lists are written in D4, D12, D17.

### D19. `machines/operation_result.rs` (86 lines)
`OperationResult::{Unit, Scalar, Structural}` (`:7`), `StructuralOperationResult` (`:30`) with `is_unrestricted_plain`/`is_affine_plain` (`:42-54`), `StructuralResultClaimBinding`, `scalar`, `scalar_ref`, `structural` (macro `enum_variant_accessor`, `:78`), `expect_scalar` panics on non-scalar (`:80-85`). Fail-open: none seen.

### D20. `machines/ranking.rs` (96 lines)
`TerminalRankedScc::Natural(Vec<TerminalNaturalCycle>)` (`:5`), `TerminalNaturalCycle` (`:10`) with arena-backed rank values, `rank_values` (`:17-29`), `scalar_rank_value` (`:31`), manual `PartialEq` comparing spans' contents (`:42-58`), `TerminalIntegerRankValue` (`:63`), `TerminalNaturalRankEdge`, `TerminalNaturalRankComparison::{Preserving, Strict}` (`:93`). `rank_values` returns `Some` only if the span is nonempty, has the same length as the first block's rank signature and every integer type equals the signature's and is not an address type (`:23-28`). `ranks.first()?` means a cycle with no ranks returns `None`. Fail-open: none seen.

### D21. `machines/record_field.rs` (25 lines)
`RecordFieldInitializer`, `RecordFieldValue::{Scalar, Structural}`, `as_structural`. Data only.

### D22. `machines/register_access.rs` (108 lines)
`RegisterAccessEvent<Operand>` (9 variants, `:7`), `operation()` (`:39`), `inputs()` (`:53`), `try_map_inputs` (`:69`), `map_operands` (`:103`, unwraps `Infallible`). Same-job: parallels `AtomicAccessEvent` (D1). Fail-open: none seen.

### D23. `machines/scalar_case_field.rs` (8 lines)
`ScalarCaseField { field, value, range_obligation }`. Data only.

### D24. `machines/terminator.rs` (536 lines)
1. Purpose. The block-ending instruction set and its edge/cleanup/argument accessors.
2. Shape. `Terminator` (`:11`) 11 variants: `PendingNormalCompletion`, `EliminateEmpty`, `InvokeNoNormal`, `ExternalCompletion`, `Jump`, `Conditional`, `StructuralCase`, `Return`, `ReturnUnit`, `ReturnStructural`, `Crash`. Methods: `storage_uses` (`:95`), `storage_uses_with_arguments` (`:101`), `map_structural_uses` (`:183`), `cleanup_rosters` (`:232`), `cleanup_rosters_mut` (`:278`), `edge` (`:324`), `edges` (`:341`), `map_scalar_uses` (`:362`), `successor_block_arguments` (`:401`), `successors_with_runtime_arguments` (`:425`), `successor_edges` (`:467`), `successor_targets` (`:493`). `SuccessorEdge` (`:502`), `StructuralCaseSuccessorEdge` (`:513`), two macros (`:522-536`).
4. Decides by variant. `edge()` panics for `PendingNormalCompletion` ("an open block tail has no edge", `:334`) and for `Conditional`/`StructuralCase` ("a branching terminator has multiple successor edges", `:336`). `successors_with_runtime_arguments` includes an edge only if it has scalar or structural arguments (`:428`).
5. Cases. Comment on `PendingNormalCompletion`: "This record has no executable edge or wire tag." (`:13-14`). `map_structural_uses` remaps `structural_arguments` places and sources but not cleanup-action places or `returned_claims` (`:183-230`), whereas `storage_uses_with_arguments` includes cleanup action places (`:171-179`). Comment on arguments: "Branch and tail-call arguments may transport a pointer without materializing its referent." (`:99-100`).
6. Same job. `cleanup_rosters` and `cleanup_rosters_mut` are the same `match` twice (`:232-276`, `:278-322`).
9. Fail-open: none seen. A caller that skips `Terminator::edge()`'s panic cases is not traced.

### D25. `machines/trapping_integer.rs` (83 lines)
`TrappingIntegerOperation` (8 variants), `TrappingIntegerPrimitive`, `primitive`, `operands`, `map_operands`, macro `trapping_arith_set_pattern` listing Add/Divide/Multiply/Remainder/Subtract (`:74-83`). Data and accessors.

---

## E. `proofs/` (2,653 lines)

### E1. `proofs/applications/proof_applications.rs` (1,246 lines; crate `proof-applications`)
1. Purpose. Computes which machines and declarations a build target pulls in, by walking calls, static bindings and provider edges, generic over a `Declaration` id type.
2. Shape. `ProofApplicationMachine<D>` (`:7`), `ProofApplicationTargetMember`, `ProofDeclarationDependencies`, `ProofApplicationPlan<D>` (`:33`) with `callable_for_target` (`:46`), `callable_without_target` (`:57`), `declared_callable_for_target` (`:65`), `declaration_closure` (`:127`), `try_map` (`:823`). `Application`, `ProofInvocationEdge`, `ProofApplications<'a,D,Calls>` (`:196`) with `scopes` (`:312`), `required_scopes` (`:331`), `membership` (`:366`), `reachability` (`:382`), `invocation_reachability` (`:413`), `invocation_machines_walk` (`:429`), `walk_applications` (`:446`). Trait `ProofApplicationCalls` (`:673`), `ProofCallGraph<D>` (`:693`) with `index_machines` (`:788`), `machine_row` (`:803`), `capture` (`:924`). Tests `:989-1246` (6 tests).
3. I/O. Input: a plan (machines, family members, dependencies, provider edges) and a call graph. Output: ordered `Vec<Declaration>` of reached machines, calls, invocation edges.
4. How it decides. By target string: `authored_target.is_none_or(|declared| Some(declared) == target)` (`:120-123`, `:305-309`). Product selection by flag `product_selected` (`:79`, `:284`). A machine with a `source_target` and no target selected is skipped (`:292-294`). Family members found by `member.target == target` (`:302`). Visited set keyed by `Calls::declaration_order` into a `BTreeMap` (`:451`). The absent declaration is `Declaration::default()` and is skipped (`:459`, `:498`).
5. Cases. Call binding substitution handles one level of static binding with recursion via `resolve_bindings` (`:637-662`). Provider edges match when requirement equals the callee, its called machine, the call's state or operator (`:529-533`). Cleanup edges and dynamic realizations are followed only if the machine has a call summary (`:552-554`).
6. Same job elsewhere. `ProofApplicationPlan::matching_machine`/`target_machine_matching` (`:82-125`) and `ProofApplications::matching_machine`/`target_machine` (`:261-310`) are two implementations of the same lookup; they differ (the second adds the `source_target`/no-target refusal `:292-294` and hard-codes `product_selected` for family members `:303`; the first takes an `accepts` function `:118`). The binder rebasing loop is written twice (`:470-483`, `:602-615`).
7. Unused or one-caller. Grep (`callable_without_target|declared_callable_for_target|callable_for_target|.required_scopes(|declaration_closure(|invocation_machines_walk|.invocation_reachability(|.scopes(`) = 47 matches in 24 files, 16 of them in this file. External users include `build/evaluation/authority/admission/admission.rs` (3), `psi/semantics/src/evaluation_scope.rs` (4), `installation-admission/src/resources.rs` (3).
8. Stated limits. Comment at `:55-56`: "No target is selected. Only an untagged, non-family declaration may supply code; this does not choose a host or a default family member."
9. Fail-open risks. `required_scopes` returns only `[primary]` when `owners.is_empty() && !unscoped` (`:338-340`). `walk_applications` `continue`s when a symbol has no `called_machine` or no `target_machine` (`:464-469`): an unresolved callee contributes no machines (a missing row keeps the machine out of the closure rather than in). Whether a consumer reading a smaller closure then skips obligations was not traced. Open.

### E2. `proofs/certificates.rs` (279 lines)
Proof-certificate types: `ProofSystemMarker` (`get() == 9`, `:8-22`, `new` accepts only 9), `AdmissionKind` (3), `CertificateEnvelope<Proof>`, `AdmissionEvidence`, `EvidenceRoute<Proof>` (`KernelDerived`, `CertificateDerived`, `Admitted`, `SourceChecked`, `:59-64`), `PrimitiveJudgment` (6 variants, `:86-93`), `ProofNode` (`:96`), `ProofRule` (about 36 variants, `:102-228`), witness structs (`:231-269`). Data only. Same-job: `psi/semantics/proofs/bounded/bounded.rs:454-459` re-exports `ProofNode`, `ProofRule`, `ProofSystemMarker` from here rather than redefining them (confirmed for the marker, `:13`, and for `PrimitiveJudgment` via `kernel.rs:16`).

### E3. `proofs/declarations.rs` (15 lines)
`ProgramProofs` with 10 `Vec` fields (`:4-15`). Data only.

### E4. `proofs/evidence/content.rs` (167 lines)
Content-conservation records (`ClaimContentProjection`, `ContentEntryClaim`, `ContentIdentityReshuffle`, `ContentPartitionComposition`, `ContentTransformationDerivation` and custody/lineage types) and `ContentTransformationDerivation::map_places` (`:108-166`): rewrites places in `conservation`, `inputs`, `outputs` and, for `Boundary` custody, the `target` of each substitution and both sides of each address transport (`:143-163`), then re-sorts. It does not map `substitution.source` (`:152-154`) or `ancestry`. Two empty `impl` blocks (`:28`, `:47`). Decides nothing. Fail-open: none seen.

### E5. `proofs/evidence/contracts.rs` (255 lines)
`RuntimeDomainMembership` (`:10`), `PublishedWriteFrame` alias (`:26`), `MachineContract` (`:31`, 34 fields), `validate_parameter_order` (`:85`), `runtime_parameter_formals` (`:100`: scalar positions first, then structural), `MachineContract::empty` (`:117`), `OutcomeSpecific*` types, `CrashRouteBucket`/`CrashRouteGuard` (`:226-236`), `CrashPredicateTerm` newtype (`:239`), `ContractClause`. Decides: `validate_parameter_order` requires counts of each kind in `parameter_order` to equal `parameters.len()` and `structural_parameters.len()` (`:86-96`). Stated limits: "Omitted reach and crash clauses can leave requirements open; an explicit empty clause promises none." (`:50-52`); "`None` retains the conservative ceiling; `Some([])` establishes an empty frame." (`:68-70`). `empty()` leaves `termination: NoGuarantee` (`:139`). Fail-open: none seen.

### E6. `proofs/evidence/declarations.rs` (135 lines)
Proposition and evidence declaration records (`PropositionDeclaration`, `PropositionBinderKind`, `PropositionEvidence::{FactOnly, Witness}`, `PropositionApplicationIdentity`, `EvidenceInterfaceIdentity`, `EvidenceContractLane`, `EvidenceArgumentSubject`, `EvidenceCallableOwner`). `ScalarPredicateDefinition` comment (`:5-6`): "A transparent scalar predicate, not evidence that its body holds." Data plus `argument_position` (`:104`). Identity types use `String` (`:18`, `:49`); the test at `psi/tests/ir_schema.rs` is per crate map (not read) said to forbid string identity fields.

### E7. `proofs/evidence/mod.rs` (54 lines) / E8. `operation_crash_contracts.rs` (10 lines) / E9. `outputs.rs` (88 lines) / E10. `recursion.rs` (68 lines) / E11. `scalar_block_invariants.rs` (15 lines)
`mod.rs`: module list and re-exports (`:6-54`). `TerminalOperationCrashContract` (machine, operation, `published_routes`, `crash_continuations`). `outputs.rs`: `ProofOutputCall` (`:6`), `StaticRequirementDispatch` (`:18`), `ProofOutputRuntimeResult` (4 variants, `:38`), `EvidenceCallableTarget` (`:46`, `owner` `:55`, `machine` `:65`), `ProofOutput`. `recursion.rs`: `TerminalProofRankingRelation`, recursive component/edge/call-site records (call site by `state_identity` + `statement_index` etc.). `scalar_block_invariants.rs`: `ScalarBlockInvariant` with arrivals `(edge, obligation)`. All data only; none decides.

### E12. `proofs/evidence/values.rs` (157 lines)
Proof-only float-meaning types: `ProofValueId(pub u32)`, `ProofPropositionId(pub u32)`, `FloatMeaningSource` (10 variants, `:100`) with `format()` (`:114-127`), `FloatMeaningProjection`, `FloatMeaningEqualityProposition`, `float_meaning_equality_proposition_id(index)` (`:145-157`): SHA-256 domain digest of the index, low 8 bytes `| 1`. Decides: none. A computed id can collide in 63 bits; no collision check here (data note).

### E13. `proofs/mod.rs` (9 lines), E14. `proofs/proof_application_scope.rs` (119 lines), E15. `proofs/theorems.rs` (36 lines)
`mod.rs`: module list. `proof_application_scope.rs`: `ProofDeclarationId(pub u32)`, `ProofScopeDeclaration::{Missing, Nominal, Callable, Operator, Unavailable}` (`:11`), `ProofCallableHeader` (`:31`; defaults are `Err("... absent")` `:51-67`), `ProofApplicationScope` (`:70`: arenas, `plan: ProofApplicationPlan<ProofDeclarationId>`, `calls`, recursive components), `callable_declaration` (linear scan, `:96-107`), `applications` (`:109`), `declaration` (index `id - 1` into storage slice, `:115-118`). `theorems.rs`: `TerminalMathematicalCarrier`, `TerminalIdentityFormation`, `TerminalIdentityFormations`, `TerminalIndexedTheorem` (`certificate: Vec<u8>`, `:29-36`); comment `:9-10`: "These rows grant no identity witness and allocate no runtime value copy." Fail-open: none seen.

---

## F. `psi.rs` and `scalars/` (892 lines)

### F1. `psi.rs` (69 lines)
`pub struct Psi` with the 11 fields `vocabulary_marker`, `identities`, `entry`, `machines`, `types`, `scalars`, `interfaces`, `access`, `execution`, `proofs`, `layouts` (`:8-31`), module declarations (`:33-44`), `Psi::for_entry` (`:47`), `Psi::declarations` (`:54`, sets `vocabulary_marker: VocabularyMarker::CURRENT`). Decides nothing. `TerminalModule` alias is in `exports.rs` (not in this unit).

### F2. `scalars/declarations.rs` (10 lines) / F3. `scalars/mod.rs` (8 lines)
`ScalarDeclarations` with 5 fields (`:3-10`); module list. Data.

### F4. `scalars/folded_constants.rs` (106 lines)
`FoldedConstantClosure` (`target`, `dependencies`), `FoldedConstantDependency` (`target`, `requirement`, `plan: [u8;32]`, `candidate`), `FoldedConstantCandidate`. `validate_plan_inputs` (`:26-59`): target must equal, the number of supplied preimages must equal the number of distinct plan digests for that target, and each preimage's `DomainDigestEncoder::new(b"")` digest must be in the expected set (`:34-57`). `validate` (`:61-105`): no dependencies requires no target; dependencies need a nonempty target, strictly sorted order, nonempty target text, nonzero plan digest, nonempty requirement text; a `candidate` must match exactly one `provider_candidates` row on four fields (`:88-102`). Decides on equal digests and counts. Fail-open: `candidate: None` is skipped without check (`:85-87`); open (no comment; whether a `None` candidate is legitimate was not traced).

### F5. `scalars/scalar_applications.rs` (122 lines)
`ScalarApplications` (`:7`: private arena `declarations`, `conditions`, `proof_scope`), `ScalarApplication::{Missing, LayoutObservation, Evaluate}` (`:14`), `LayoutMeasurement::{Size, Alignment}` with `SEMANTICS_VERSION = 1` (`:36`), `ArrayExtent::{Literal, Application, Parameter}` (`:40`) with a comment that zero is admitted (`:43-44`), `ScalarApplications::insert` (`:94`: refuses `Missing`, ids are `len + 1`), `declaration` (index `id - 1`, `:110`), `declarations`, `is_empty`. Fail-open: none seen.

### F6. `scalars/scalar_conditions.rs` (577 lines)
1. Purpose. The scalar conditions a program publishes: range conditions (bounds computed by a small stack program), completion conditions and predicate trees, with validation when they are retained.
2. Shape. `ScalarConditionSubject` (`:14`), `ScalarRangeObservation` (`:21`), `ScalarRangeRequirement::{Fixed, AddressModel}` (`:35`), `ScalarRangeCondition` (`:41`), `ScalarCompletionCondition` (`:53`), `ScalarConditions` (`:63`: four arenas), `ScalarPredicateNode` (`:71`), `ScalarPredicateCondition` (`:76`) with private `new` (`:97`) and `evaluate` (`:153`), `ScalarConditions` methods (`:177-322`), `ScalarApplications::layout_predicate` (`:327-370`), private `LayoutPredicate` builder (`:373-485`). Tests `:487-577`.
3. I/O. `layout_predicate` turns a ground `Proposition` over `ScalarTerm::Application` observations into a predicate tree. `retain_range` validates and stores a bounds program.
4. How it decides. `ScalarPredicateCondition::new` rejects `Missing`, `Unknown` and `Leaf(Missing)` nodes (`:106-113`), computes each node's value kind from preceding children only (`:114-124`), requires the root to be a valid handle of kind Boolean (`:126-136`). `retain_range` rejects inverted fixed bounds, `Missing` steps, inverted `Bounds`, `Observation(Missing)`, operator steps with fewer than 2 operands and a final stack depth other than 1 (`:274-312`). `layout_predicate` returns `Ok(None)` when the goal is not ground or observes no target application (`:339-344`); a guard that is not ground is an `Err` (`:346-348`).
5. Cases by shape. `LayoutPredicate::proposition` accepts `Truth`, `Falsehood`, `Equal`, `LessThan`, `LessOrEqual`, `Implication`, `Conjunction`, `Disjunction`; every other proposition returns `None` (`:432-476`); terms accepted are `Application` (integer-typed, matching the declared scalar type), `Integer`, `Boolean` (`:393-418`). Limits: 4,096 nodes (`:384`), recursion depth 64 (`:429`).
6. Same job elsewhere. `PredicateBudget` in `closed-predicates/budget.rs:8` also caps at 4,096 steps and depth 64; same numbers, separate counter.
7. Unused or one-caller. Grep (`insert_predicate|retain_completion(|retain_predicate(|retain_range(`) gives 30 matches in 9 files. `insert_predicate` (`:228`, `pub`) has 1 production caller (`source-to-psi/pipeline/06_.../verdict.rs:1425`) and 1 test caller (`psi/artifact/wire/src/foundation_validation.rs:5235`); `retain_completion` (`:191`) has callers in `module_wire.rs:2958,3110`, `scalar-applications/conditions.rs:597,640` and a Build test. `layout_predicate` 1 caller (`psi/semantics/src/scalar_target_ranges/calls.rs:47`).
8. Stated limits. "A ground comparison can follow its observations into target closure. Runtime values still need the ordinary source proof." (`:325-326`).
9. Fail-open risks. `insert_predicate` is `pub` and takes an unvalidated `ScalarPredicateCondition` (`:228`); `ScalarPredicateCondition` has a `Default` with an invalid root (`:84-94`) and public `subject`/`role`/`description`. Only `new`/`retain_predicate`/`layout_predicate` validate; callers of `insert_predicate` must pass a condition that came from `new` (verdict.rs clone). Traced for verdict.rs only by name (not read). `retain_completion` stores any `(subject, application)` pair without checking the application exists (`:191-201`). Open. `evaluate` passes a final `&|_, _| true` closure to `evaluate_nodes` (`:172`), whose meaning is defined in `numerics` (not read).

---

## G. `semantics/` (1,549 lines)

### G1. `semantics/closed-predicates/budget.rs` (190 lines)
`PredicateBudget { remaining }` default 4,096 (`:8-16`), `step(depth)` refuses depth `>= 64` (`:19-24`), `proposition` (`:34`), `content`, `mathematical`, `scalar` walk terms and charge nodes; path lengths are charged by `count(path.len())`. `check_predicate_evaluation_size` (`:182`). Decides on counts. Cases: `proposition` is an exhaustive `match`, with `Truth | Falsehood | Atom(_)` charging nothing (`:112`). Fail-open: none seen.

### G2. `semantics/closed-predicates/closed_predicates.rs` (16 lines)
Module list and re-exports (`:5-16`).

### G3. `semantics/closed-predicates/guard.rs` (135 lines)
1. Purpose. Three-valued evaluation (true, false, unknown) of a crash-route guard so that a route is ruled out only when its guard is definitely false.
2. Shape. `closed_guard_with` (`:15`), `closed_guard` (`:23`), `closed_guard_with_float_meanings` (`:32`), `walk` (`:40`), `closed_leaf` (`:69`).
4. How it decides. `Conjunction`/`Disjunction`: a child equal to the opposite of the identity ends the walk (`:50`); unknown children make the result unknown (`:52-55`). `Implication`: false premise gives true; true premise gives the conclusion; unknown premise gives true only when the conclusion is true (`:60-64`). Leaves: `Truth`, `Falsehood`, `FloatMeaningEqual` (size-checked first, `:78-80`), `ScalarIeeeFloatComparison`, Boolean `Equal`, ordered comparisons of integers of the same type (`:89-101`), integer-math comparisons (`:102-108`). Everything else is `None` (`:109`).
5. Cases. Integer comparison requires `left_type == right_type` else unknown (`:93`).
6. Same job. The interpreter calls the same function (module comment `:1-3`; `closed_guard*` has 12 matches in 5 files, including `interpreter/embedding/boundary_effects.rs` with 4).
9. Fail-open: none seen. Unknown is not treated as true; unknown means the route stays reachable (comment `:1`: "ruled out only when its guard evaluates to false").

### G4. `semantics/closed-predicates/ieee.rs` (170 lines)
`closed_ieee_relation` (`:13`), `ieee_relation` (`:17`: both operands must have the stated float format, else `None`, `:30-34`), `observed_meaning` (literals or an `observe` callback, `:49-57`), `closed_float_meaning_equality` (`:69-115`: validates the clause, evaluates the value graph in order, operands `Meaning(slot)` must refer to an earlier value (`values.get(slot)?`), `Integer(..)` operands must be integer literals, kernel budget argument 4096 at `:105`). Decides on bit patterns and slot indices. Callers: `closed_ieee_relation` is used at `proofs/bounded/kernel.rs:95,98` (2 sites). Fail-open: none seen; every unmatched case is `None`.

### G5. `semantics/closed-predicates/integer.rs` (425 lines)
`ClosedIntegerEvaluator` (`:28`) with per-invocation counters `term_nodes`, `allocation_limbs`, `limb_work` against fixed limits: nodes 8,192, depth 128, result bits 65,536, allocation 262,144 limbs, work 1,048,576 (`:8-12`). `compare` (`:35`), `evaluate_closed` (`:52`), `check_term_size` (`:60`, iterative), `evaluate` (`:97`), `charge` (`:179`), `check_integer_math_term_size` (`:205`), `big_integer_literal` (`:214`). Decides: `MathValue` (open variable) evaluates to `None` (`:102`); negative shift count to `None` (`:138-140`); shift count not representable as `usize`, or any result beyond 65,536 bits, to `ResourceLimitExceeded` (`:141-146`, `:160-162`). `Embed` evaluates a scalar by `integer_value()`, which `None`s for non-literal terms (`:103-108`). Cases: only `Add`, `Subtract`, `Multiply`, `ShiftLeft` are evaluated (`:113-119`). The budget is not reset between relations (test `:412-424`). Same job: `bignum` work is estimated, not measured (limb formulas `:127-159`). Callers of `check_integer_math_term_size`: `proofs/bounded/integer_rules.rs:2843,2865` and `kernel.rs:31`. Fail-open: none seen.

### G6. `semantics/default-integer-layouts/integer_defaults.rs` (60 lines; crate `layout-planning`)
`standalone_integer_geometry(bits)` and `field_integer_geometry(bits)` (`:8-15`) return `None` outside 1..=128 bits (`:18-20`); size is `ceil(bits/8)` and alignment is `next_power_of_two(size)` capped at 8 (standalone) or 16 (field) (`:21-25`). Re-exports `conventional_record_geometry`/`conventional_sum_geometry` (`:4`). Callers: `calling-conventions/src/plans/vocabulary.rs:187,192`, `references/reference_storage_geometry.rs:80`, `driver/compile_time_invocations/fixed_layouts/reflection.rs:92` (grep). Fail-open: none seen (`None` for unsupported widths). Case: 17 bits give size 3, alignment 4 (test `:40`), so size is not a multiple of alignment (records round up in `packing.rs`).

### G7. `semantics/proofs/bounded/bounded.rs` (553 lines; crate `proof-admission` module root)
1. Purpose. Root module of the proof-admission crate: declares submodules and defines the admission-evidence entry points `verify_obligation` and `verify_obligation_with_machine_parameters`; also re-exports the crate's and the kernel's public surface.
2. Shape. `mod bounded_denotation`, `integer_affine` (`:6-7`); `pub(crate) mod admission::evidence` (`:10-334`): `AuthorizedAdmission`, `ObligationClass::{Derivable, AdmissionAuthorized}`, `Obligation`, `AcceptedFactRoute::{KernelDerived, CertificateDerived, Admitted}`, `AcceptedFact`, `verify_obligation` (`:65`), `verify_obligation_with_machine_parameters` (`:82`), `primitive_derivation` (`:141`), `EvidenceError` (7 variants, `:157`). `term_matchers` (`:348-390`). Re-export block (`:392-459`). Test helpers and 3 macros (`:461-553`).
3. I/O. Input: proposition context, an `Obligation`, assumptions, semantic axioms, machine parameter values and an `EvidenceRoute<&ProofNode>`. Output: `AcceptedFact` or `EvidenceError`.
4. How it decides. First `context.validate(&obligation.proposition)` (`:90-92`). Then by route: `KernelDerived(judgment)` is re-decided by `decide_primitive` (`:94-98`); `CertificateDerived` goes through `accept_certificate_with_machine_parameters` (`:99-113`); `Admitted(evidence)` first refuses if the proposition is primitively derivable (`:116-118`), then requires class `AdmissionAuthorized` (`:119-121`), then compares site, kind and authority identity (`:122-127`), and then always returns `AdmissionNotAcceptedByProfile` (`:128`); `SourceChecked` always returns `SourceCheckedOutsideOperation` (`:130-132`).
5. Cases. `primitive_derivation` tries five of the six `PrimitiveJudgment` variants (`:145-151`); `ClosedFloatIntegerRelation` is not in the list.
6. Same job. `AdmissionEvidence`, `EvidenceRoute`, `ProofSystemMarker` come from `terminal_psi` (`:13-16`, E2), not redefined.
7. Unused or one-caller. `verify_obligation` and `_with_machine_parameters` are re-exported (`:392-396`); counts not run. `psi/semantics/src/calls/callable_refinement.rs:757` calls it per the crate map.
8. Stated limits. Test `proof_system_marker_has_no_compatibility_ladder` (`:318-331`) pins the single accepted marker 9. Name text `AdmissionNotAcceptedByProfile` at `:128` is the stated limit: no admitted evidence is accepted today.
9. Fail-open risks. none seen: both non-certificate, non-kernel routes return `Err` (`:128`, `:131`). `primitive_derivation` omits `ClosedFloatIntegerRelation` (`:145-151`), so for a proposition derivable only that way an admission is not blocked by that rule; but `Admitted` still ends in `Err`, so nothing is accepted. Traced within the file.

---

## Coverage

- Read in full, all 93 files, every range in `units/unit_041.tsv` (20,300 lines): 
  `identities/` 2,929; `interfaces/` 4,873; `layouts/` 3,294; `machines/` 4,110; `proofs/` 2,653; `psi.rs` 69; `scalars/` 823; `semantics/` 1,549.
- Skimmed: none.
- Not read: none.
- Outside this unit, only these were read (to check claims): `psi/artifact/wire/src/foundation_validation.rs:5215-5245`. Everything else was grep output.

## Five things a maintainer most needs to know about this unit

1. Nearly everything compares identities by text, not by index. Identity tables are per-module string arrays, so `same_identity_as` in `identities/declarations.rs:31`, `:83-92`, `interfaces/reach_applications.rs:196-199` and `service_surface.rs:95` compares strings. Missing or absent identities read as `""`/`None`, and several comparisons (`declarations.rs:83-92`, `conformances.rs:130`, `commitment.rs:159`) treat two absent ones as equal or hash them as empty.
2. The identity grammar in `application-identity/src/type_identity.rs` is a spelling-driven parser with a visitor, and the visitor decides what is trusted. The shared visitor `DeclarationIdentitySyntax` (`:1814`) accepts any package owner and any embedded name; its comment says it checks framing, not authority (`:1812`). Four separate visitor implementations exist in the repository.
3. The call-kind and operation-kind lists are written out by hand in several places (`direct_calls.rs`, `operation_patterns.rs`, `operation_arguments.rs`, `invocations.rs`). `map_scalar_uses` (`operation_arguments.rs:42-273`) is exhaustive and compiler-checked; the others use `_ =>` arms (`direct_calls.rs:81`, `invocations.rs:300`), so a new call kind could silently be missed.
4. Duplicated mechanisms: 64-bit FNV-1a is hand-rolled in `boundary_machines.rs:711` and exists in four other places besides `wire::fnv1a`; `ProofApplicationPlan` and `ProofApplications` each implement the same machine/target lookup with different rules (`proof_applications.rs:82-125` vs `:261-310`); `ReferenceResultShape` has two copy-pasted implementations (`result_regions.rs:26-85`).
5. Scalar condition and proof entry points: `ScalarConditions::insert_predicate` is `pub` and unvalidated (`scalar_conditions.rs:228`); `proof_admission::verify_obligation` never accepts an `Admitted` route (`bounded.rs:128`) and `SourceChecked` is always refused (`:131`); the closed-predicate guard is three-valued and only a definite `false` rules a route out (`guard.rs:1`, `:44-67`).

## What I am unsure about

- Whether callers treat the `None`/empty returns I flagged (`private_address_fragment`, `direct_call_parts`, `lookup` in `non_observation.rs`, `validate_stored_integer_bits` panic paths) as refusal; I did not read those callers.
- The exact number of `OperationKind` variants (about 110, by grep of line shapes, not by compiling).
- Whether `candidate: None` in `FoldedConstantDependency` (`folded_constants.rs:85-87`) is meant to be accepted without a provider association; no comment says.
- The meaning of the final `&|_, _| true` closure passed to `numerics::scalar_predicates::evaluate_nodes` (`scalar_conditions.rs:172`); defined outside this unit.
- Grep counts that combine several names (marked "not counted separately") do not give per-function call counts; where I gave a number it came from a per-name listing.
