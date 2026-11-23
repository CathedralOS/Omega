# Record: unit_060 (area 03-source-stage-05)

Root of every path below: `source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/`,
written `<s>/`. Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. All 24 ranges were read in full.
Read = I opened the lines. Grepped = ran the Grep tool (count or content mode; the command is named). Inferred = my reading of context.

---

## 1. `<s>/operational.rs` (1-253)

1. Purpose. Keeps the source-written `suspend`/`block` acknowledgement on each call before the call becomes an SSA operation, and refuses a `suspend`-marked call that is not at a legal position (complete statement, simple `let` initializer, transition subject, terminal expression). It also copies each call's authored callee token span out of the typed trees for diagnostics.
2. Shape. `retain` :11, `boundary` :34, `retain_callee_spans` :41, `positions` :88, `statements` :92, `expression` :146. No types.
3. In/out. `retain` writes `Expression.call_context` (a `terminal_psi::CallAcknowledgement`) from `language_core::CallOperationalAcknowledgement` :16-31. `retain_callee_spans` reads `typed.expression_table.call_node(..).authored_call_selection` / `StatementNode::Call` and `typed.authored_declaration_selections()` :49-65 and pushes `(CallSite, span)` into `lowered.sources.call_targets` :74-85. `positions` returns `Checked<()>` over a `Body`.
4. Decisions. The refusal is keyed on three flags of the node: `origin == CallOrigin::Authored && acknowledges_suspend && !suspension_position` :147-151, refusal text at :152-153. `retain` branches on the acknowledgement origin enum (`CompilerSynthesized` vs `Source`) :16-31. The callee span is kept only when the selection `kind() == Call` :63. All keyed on identity/flags, no spelling.
5. Cases by shape. `statements` :99-141 and `expression` :155-251 list every `Statement` and `ExpressionKind` variant explicitly (no `_` arm); `Declare`, `CaptureReference`, `ParkedBuildLocals` are skipped :138-140.
6. Same job elsewhere. none found in the unit.
7. Callers (Grep `retain_callee_spans|operational::positions|operational::boundary|operational::retain` over `source-rust/`): `positions` 1 (`<s>/lib.rs:1512`); `retain_callee_spans` 1 (`<s>/lib.rs:1802`); `retain` 3 (`read/calls.rs:668`, `:765`, `read/transitions.rs:582`); `boundary` 4 (`read/machine.rs:4408`, `read/statements.rs:37`, `:511`, `read/transitions.rs:586`).
8. Stated limits. Module comment :1-3: "Source position is a formation rule". None else.
9. Fail-open. `boundary` does nothing when `call_context` is `None` :35 (traced by reading: a call with no context has no `suspend` acknowledgement to misplace; `positions` only refuses on `Some`). `retain_callee_spans` skips silently when a call has no selection span :74 (only a diagnostic span is lost, no acceptance). Otherwise none seen.

---

## 2. `<s>/outcome.rs` (1-903)

1. Purpose. For every machine, works out from structure whether some path completes normally, which crash routes it publishes, what service reach and synchronous entries it publishes, and whether it may suspend/block. Callee components are walked first; only call cycles iterate. It never proves a route possible or impossible (header :1-4).
2. Shape. `Outcome` :59 (`operational:[bool;2]`, `completes_normally`, `crash_routes`, `service_reach`, `fixed_service_reach`, `synchronous_entries`). `outcomes` :106. `counted_outcomes` :131 (cfg(test)). `Reached` :166, `Contributed` :178, `CaseProgress` :192, `Walk` :218 with `new` :238, `settle` :277, `visit` :334, `table` :384, `fixed_reach` :403, `published` :420, `published_row` :450, `statements` :489, `target` :569, `expression` :582.
3. In/out. Reads `IdTable<MachineId, Body>`, `Signatures`, `requirements: &[MachineId]`, `CallableNamespace`, `local_bodies`, `Inhabitation`, `Components` :106-114. Returns `IdTable<MachineId, Outcome>`; when bodies refuse, returns the refusal of the lowest `MachineId` :98-99, :316-320.
4. Decisions.
   - Normal completion: `!declared_empty && (completes || self.local.get(machine) != Some(&true))` :352; `declared_empty` from `inhabitation::excludes_result` on the result type :342. Keyed on machine identity (membership in `local_bodies`, `requirements`).
   - Trap cause: a `Strict` node whose `policy == Some(Policy::Trapping)` adds `CrashCause::Trap` :714-716 (keyed on the enum, not spelling).
   - Published crash routes by `CrashCeiling`: `Authored` as written :427; `Inferred` and the machine is a requirement gives none :430; `Inferred` otherwise gives every reached cause with guard `Truth` :432; `DeclaredNone` gives none :439.
   - Calls: selected call copies authored crash causes, refuses an `Inferred` crash or operational ceiling :764-788; boundary call completes iff `required_contract.is_some()` or the boundary completion is `NormalReturn` :824-826; machine call completes iff result not excluded, `ensures` not false, and `required_contract.is_some() || reached.completes_normally` :854-859; a written crash ceiling on the invocation signature governs over what the callee body reaches :881-884.
   - Reach/entries rows by `Ceiling::{Authored, Inferred}` :459-481; an `Authored` ceiling over an open contribution refuses only on `Axis::Entries` :460-471.
5. Cases by shape. `Statement` :493-562 and `ExpressionKind` :583-900 each list all variants. A `Dispatch`/`Match` completes iff some arm completes; arms of an already-closed case are skipped via `CaseProgress` :190-216, :530-546, :638-661. Comment :703-705 ties `Trapping` to numeric_values.md.
6. Same job elsewhere. Service-reach fixed point: `Contributed`/`published_row` here (:450-481, :729-900) and the `loop {}` over `module_invocations()`/`callable_invocations()`/`boundary_invocations()` in `<s>/reach.rs:435-487`. I read both; they derive reach from different inputs (closed `Body` here, lowered `TerminalMachine` there). Suspected third site: `read/services.rs` (not read).
7. Unused/one-caller. `outcomes`: Grep `outcome::outcomes|counted_outcomes|outcome::Outcome` over `source-rust/` gave production call at `<s>/lib.rs:1636`, plus 3 `use` lines in `<s>/tests.rs:1962,2029,2088`. `counted_outcomes`: 1 call (`<s>/tests.rs:2227`), cfg(test). `Outcome` is re-exported `lib.rs:195` and used in `declare.rs:166`, `lower.rs:88`, `lower/calls.rs:36`, `publication.rs:13`, `tests.rs:24`. The file says it is "the least fixed point"; `components` is the only ordering input.
8. Stated limits. :95-96 "A machine in `requirements` is a requirement's contract and has no body". :446-449 open callback reach "stays deferred here" for the checker.
9. Fail-open.
   - Non-local machines keep a possible normal return whatever their body does (:352; doc :101-105). A body that never returns, outside `local_bodies`, is recorded as completing. Stated (doc comment :101-105 and :221 field doc).
   - `ShortCircuit` ignores the right operand's non-completion :726. Stated (:719-721).
   - `published_row` returns the authored clone for an open Reach contribution (:472) because "The lowered-form checker substitutes the exact callback contracts" :446-449. Stated.
   - `Ceiling::Authored` vs written crash ceiling: a callee that promises less than its written ceiling is still published at the ceiling :878-884. Stated (:878-880).
   - Unknown rows are refusals (`ok_or_else(refuse..)` :251, :368, :749, :841), not accepts.

---

## 3. `<s>/policy.rs` (1-457)

1. Purpose. Maps an arithmetic suffix (`%`, `|`, `!`) on a binary operator to a policy, and maps (operation, policy) to either an executable Terminal operation or a canonical scalar term, so contracts and executable code mean the same arithmetic.
2. Shape. `Suffix` :21, `suffix` :31, `suffixed` :42, consts `WHOLLY_ANONYMOUS` :70 and `QUALIFIED_OPERAND` :73, `shifts` :78, `bitwise_not_type` :84, `total` :101, trait `IntegerComputation` :152, `saturating_shift_left` :179, `Terms` :237, `term` :266. Tests :315-456 (4).
3. In/out. Pure functions over `Strict`, `Policy`, `ScalarTerm`, `OperationKind`. `saturating_shift_left` builds a 10-node computation through the `IntegerComputation` trait, so runtime and contract share it.
4. Decisions. Keyed on the `ArithmeticDomain`/`BinaryOperator`/`Strict`/`Policy` enums (:31-38, :42-66, :107-147, :273-311). `bitwise_not_type` accepts only `ValueType::Scalar(Integer)` that is not an address and has an empty domain set :86-90.
5. Cases by shape. `suffixed` allows 7 operators (add, sub, mul, div, mod, shl, shr) and refuses the rest with "policy suffix on an operator that is neither arithmetic nor a shift" :42-65. `total` returns an operation only for add/sub/mul under Wrapping or Saturating and for Wrapping shifts :107-130, `None` for divide, remainder, bitwise, comparisons, Saturating shifts, Trapping :130-146. `term` matches (operation, policy) pairs :276-311; `ShiftRight` with `None | Some(Saturating)` maps to the exact shift-right constructor :280; bitwise ops with a policy give `None` :307-310.
6. Same job elsewhere. `term` (:266) and `total` (:101) both map (Strict, Policy) to a Terminal form, one for contract terms and one for ops; stated :1-6. `Terms` (:237) and `IntegerOperations` (`lower/operations.rs:816`, grepped) are the two `IntegerComputation` adapters.
7. Callers (Grep `policy::(bitwise_not_type|shifts|suffixed|suffix|total|term|saturating_shift_left|IntegerComputation|WHOLLY_ANONYMOUS|QUALIFIED_OPERAND|Suffix)`): `shifts` 2 (`lower/operations.rs:530`, `read/expressions.rs:1848`); `saturating_shift_left` 1 (`lower/operations.rs:551`); `total` 1 (`lower/operations.rs:561`); `IntegerComputation` 1 impl (`lower/operations.rs:816`); `suffix` 2 (`read/contracts.rs:1610`, `:2108`); `Suffix::` variants 4 uses in contracts.rs; `bitwise_not_type` 2 (`read/contracts.rs:1636`, `read/expressions.rs:507`); `suffixed` 1 (`read/expressions.rs:1823`); `term` 1 (`read/contracts.rs:1930`); `WHOLLY_ANONYMOUS` 2 (`contracts.rs:1898`, `expressions.rs:1828`); `QUALIFIED_OPERAND` 4 (`expressions.rs:1838,1842,2391`, `read.rs:1993` test).
8. Stated limits. :95-99 divide/remainder "still owes its divisor" under any policy.
9. Fail-open. none seen: every unmatched pair returns `None`/`Err`, which callers turn into refusals (callers in contracts.rs/expressions.rs not read).

---

## 4. `<s>/proof_scope.rs` (1-941)

1. Purpose. Builds the `ProofApplicationScope` for the module: one declaration row per emitted machine and per boundary requirement, the call/cleanup edges between them, per-callable headers with "compiler evaluation" facts, provider-candidate pairs, and the declared authority requests of selected calls. It publishes correspondence; it grants no input premise or provider (header :1-3).
2. Shape. `declare` :73 (the one entry, about 550 lines); `non_callable_source` :20; `boundary_header` :624; `published_services` :682; `declaration_custody` :705; `selection_requests` :724; `SelectionReferences` :833 (implements `Extend<SymbolHandle>` :858 and `TypedReferences` :877).
3. In/out. Reads `TypedTrees`, `Elaboration` (`module.machines`, `interfaces.boundary_machines`, `provider_candidate_intents`, `static_conformances`, `tables`), `&[SelectedSourceCall]`. Writes `elaboration.module.scalars.scalar_applications.proof_scope` :618 and sets `instance.policy_declaration` on `calling_policy_requests` :591-617.
4. Decisions.
   - Declaration lookup by `MachineId` through sorted `Vec` + `binary_search_by_key` :134, :208-219.
   - A machine's header gets `compiler_evaluation: Err(reason)` when any callee declaration is `Unavailable`, a `CallDynamic` op exists :342-346, the source has a target/target-family :400-407, or invocation refusal text accumulated :336-340, :375-377, :441-443. Unavailable is recorded, not refused.
   - Linear-carrier check on structural types `!row.no_code_affine_cleanup || !row.descriptor_cleanup || row.owns_linear_claim` :477-481.
   - Competing publication: refuses if `proof_scope != default` :578-590.
   - Custody by `symbol_package_identity` then `symbol_source_origin` then builtin atom :709-721.
   - A selected-call request is built from source-reference walking; mismatch/stale reasons strings :740-800.
5. Cases by shape. Reach-correspondence test :420-439: non-empty declared reach, non-empty published ceiling, or a closed reach application with dependency parameters / conditional carry / a `Machine` binding with `callee.is_none() || schema.is_some()` marks `invocation_refusal` "proof scope needs the complete service reach correspondence". `synchronous_invocations` is `Ok(vec![])` only when `source.invokes.is_empty()` else `Err("proof scope needs complete synchronous invocation declarations")` :529-533. Strings: "proof scope needs the actual selected boundary correspondence" :343-345; "...target-family declaration correspondence" :404-406.
6. Same job elsewhere. `declaration_custody` here (:705-722) duplicates `lowered_source_publication::selection_questions::declaration_custody` (`representations/lowered-source-publication/selection_questions.rs:104-125`, read the body): the shared one returns `Err` for a missing/stale symbol and has no builtin-atom arm; the local one returns `Missing` for a stale symbol and maps `builtin_type_atom` to `Toolchain` :719. This one file calls both: the shared one via `selection_questions::declaration_custody` at :409 and the local at :650, :866-867. Grep `declaration_custody` over `source-rust/`: shared fn used at `proof_scope.rs:409`, `read/provider_selections.rs:234`, `07_checked-program-to-psi/src/source_selection_publication.rs:1742`; local fn used 3 times in this file (:650, :866, :867).
7. Unused/one-caller. `declare` called from `publication.rs:118` (Grep `proof_scope::declare`, aggregated count; see §Aggregate greps). `boundary_header`, `published_services`, `selection_requests` have one caller each inside this file (read).
8. Stated limits. :521-522 "The declaration has been read, not accepted yet"; :526-528 written `invokes` clause "unavailable here".
9. Fail-open.
   - `product_eligible: true` and `product_selected: true` are set for every machine :520, :541. Open (no reason given at :541; :521-522 only covers `supplies_checked_body: false`).
   - `if let Ok(instances) = &mut request.instances` :592 skips an `Err` instances row without error; the `Err` stays in the request. Open (not traced who reads it).
   - `expect("declarations were checked above")` :295 and `MachineId::new(1).expect` :140, :583 panic rather than accept.
   - All unavailable conditions fail closed (`Err` in the header).

---

## 5. `<s>/provider_selections.rs` (1-120)

1. Purpose. Converts read provider rows into `TerminalProviderSelection` and service-schema rows on the module before source is released. Selection is descriptive; permissions are separate (header :1-3).
2. Shape. `SourceProviderSelections` :15 (`source`, `rows`), `SourceProviderSelection` :20 (target, schema declaration/application, surface, provider declaration/identity/application/package/custody, `composition_mode`), `publish` :33. Re-export `ProviderSelectionInput`, `ProviderSelections` :11.
3. In/out. Reads rows + `toolchain_source_identities`; interns identities; writes `module.interfaces.provider_selections` and joins `service_schemas` :115-118.
4. Decisions. Refuses a row with an empty target, empty provider identity, invalid symbol, empty schema declaration, a surface whose `root_application_identity()` differs from the row's `schema_application`, or an empty provider application :56-66. Refuses a repeated (target, schema declaration, schema application) slot :79-88. Refuses if provider selections already exist :39-44. Keyed on strings that are exact identities (target slot text, application identity text).
5. Cases by shape. none.
6. Same job elsewhere. Producer in `read/provider_selections.rs` (not in my unit; `read/provider_selections.rs:234` calls `declaration_custody`). Service schema join `TerminalServiceSchema::join` is also used in `reach.rs:254`.
7. Callers: `publish` 1 (`publication.rs:130`). `SourceProviderSelections` named at `publication.rs:29`, `read/provider_selections.rs:12,20,28` (Grep `provider_selections::publish|SourceProviderSelections`).
8. Stated limits. none.
9. Fail-open. none seen.

---

## 6. `<s>/publication.rs` (1-196)

1. Purpose. Holds the product of reading (`PreparedProgram`) and the three pieces of source publication that depend on the real proof scope, and orders them: proof scope, then calling, then providers. Also defines `SourceOrigin`, the position carried by every body node.
2. Shape. `PreparedProgram` :11 (`program`, `outcomes`, `source`, `selected_calls`, `prepared_applications`, `pending_publication`), `PendingSourcePublication` :22, `require_check_form` :34, `finalize_check_form` :67, `into_check_form` :95, `finalize_source` :104, `SourceOrigin` :140 (`Authored(span)`, `AuthoredNode{source_member, owner, node, span}`, `Unpositioned`), `span`/`source_member`/`node` :151-172, `of` :174, `From<SourceOrigin>` :183.
3. In/out. `finalize_source` calls `proof_scope::declare`, `calling_publication::publish`, `provider_selections::publish` :118-131.
4. Decisions. `require_check_form` refuses when any calling/provider rows are still pending (non-empty requests/signatures/entries/descriptions or provider rows) :36-56 or `selected_calls` is non-empty :58-63. `finalize_check_form` refuses if `prepared_applications` is non-empty :68-73. `finalize_source` checks `std::ptr::eq(typed, pending.source)` :110-117 (identity by pointer). `SourceOrigin::of` is `Authored` iff `span.start < span.end` :174-179.
5. Cases by shape. none.
6. Same job elsewhere. none.
7. Unused/one-caller. Grep `publish_access_plans|Qualifications::declare|PreparedProgram|finalize_source|finalize_check_form|into_check_form|require_check_form` over `source-to-psi/` (one aggregated run, mixed names): 39 hits in 4 files (`compilation.rs` 4, `lib.rs` 23, `publication.rs` 11, `qualifications.rs` 1). I did not split per name.
8. Stated limits. :102-103 "This is the only source-publication dependency order: real proof scope, calling, providers."
9. Fail-open. `finalize_check_form` with no pending publication goes to `require_check_form` which then checks only `selected_calls` :74-77. Traced: with `pending_publication == None` there are no calling/provider rows to owe. Stated by structure, no comment.

---

## 7. `<s>/qualifications.rs` (1-1045)

1. Purpose. Publishes the declared meaning of each domain/qualification (scalar domains, record domains, arithmetic-policy pseudo-domains), interns the qualification sets lowering uses, and records which machine may issue each routed domain.
2. Shape. `StructuralDomains` :24, `domain_identity` :29, `publish_access_plans` :52, `Qualifications` :201 (`catalog`, `identities`, `domains`, `policies`, `issuers`, `structural`, `structural_domains`, `structural_issuers`, `structural_changes`), `RouteKind` :219, `establishment_routes` :230, methods `runtime_membership` :355, `declare` :405, `next_semantic_domain` :589, `complete` :604, `declare_structural` :737, `structural` :878, `structural_establishments` :916, `members` :947, `scalar` :969.
3. In/out. Reads `read::Program` (`declared_domains`, `byte_domains`, `records`, `symbols`, `domain_theory`) and `TypedTrees`; writes `ScalarQualificationCatalog` (domains, sets, returns, coercions) and structural domain rows; `publish_access_plans` writes `access_policy` onto `module.types.structural_domains` :192.
4. Decisions.
   - Domain identity is `DomainInstance` equality (declaration symbol handle + semantic identity), found by linear `.find` :365-368, :386-388, :893-896, :981-984.
   - A policy pseudo-domain is looked up by `(ScalarType, Policy)` :1001-1004; sets are deduped by equal sorted `Vec<ScalarDomainId>` :1036.
   - Issuer routes: `ExactMachine` matches by symbol equality `*symbol == *machine` :257; `CheckedRequirement` matches machines whose conformance edge names the trait and requirement symbols :312-323; `BoundaryRequirement` adds routes but no issuer machine :305-310. Duplicate routes refused :336-338.
   - `complete`: for each domain a cast adds, origin is `Established{route}` if the machine is an issuer, `Forwarded` if routed with no issuer row, `Proved` if only a predicate; a domain with no routes and no predicate is skipped :673-687.
   - Access plan: refuses when a plan changed its subject query's authority :135-139, when two canonical owners match :121-125.
5. Cases by shape. `declare` accepts only `ValueType::Scalar` and `ValueType::Record` carriers; `Rigid` refused "domain retains an unresolved carrier" :502-504. Scalar domain with operation rules refused "scalar domain access rules need retained operation checking" :511-517. Policy only on integer carriers :1008-1010. A policy on a record: "arithmetic policy on a data type" :885-887. `DomainSet::CAPACITY` bound used for capacity :888, :979.
6. Same job elsewhere. Byte-domain identity is derived in two places with two criteria: `publish_access_plans` uses the `byte_domains` roster :100-111, `declare_structural` tests `StorageShape::ByteSequence(bytes) if bytes.is_shared_readable_view()` :780-799; both call `lower::places::byte_domain_identity`. Scalar vs structural route building shares `establishment_routes` (stated :226-229).
7. Callers (Grep over the crate): `publish_access_plans` 1 (`lib.rs:1330`); `Qualifications::declare` 1 (`lib.rs:1663`); `scalar` `lib.rs:1676,1698`, `lower/control.rs:1154,1155`; `runtime_membership` `lower.rs:997`; `complete` `lower.rs:1509`; `structural` `lower/places.rs:1255,1280`; `structural_establishments` `lower/places.rs:2148`. `members` and `domain_identity` callers inside the file only (read).
8. Stated limits. :798-804 "a plan nobody has run shows no machine to be an issuer, so the lowered form permits nobody".
9. Fail-open.
   - `structural_establishments` skips a domain with no issuer row :931-940. Stated: "An absent issuer row claims no new authority. The receiver must prove an existing fact".
   - `complete` skips a coerced domain with neither routes nor a predicate :683-687; such a domain owes nothing. Traced by reading `declare` :565-574 (a routed domain without predicate gets `Truth`).
   - `Err(_) if identities.is_invocation_scoped() => Vec::new()` for source identity :72, :433: stated at :431-432 for `declare`; no comment at :72 (open).
   - Origin `Forwarded` when a routed domain has no issuer :681. Stated :601-603 ("The checker judges them").

---

## 8. `<s>/reach.rs` (1-581)

1. Purpose. Publishes one canonical service namespace for the module and each machine's public service reach; converts source service identities to hermetically ordered Terminal ids. Acceptance of reach against a ceiling is left to the checker.
2. Shape. `SourceInvocation` :20, `SourceEnvelope` :42, `ServiceSource` :51, `ServiceMap` :60 (`prepare` :65, `service` :154, `identity` :162, `row` :168, `envelope` :181, `publish` :208), `pub use ServiceReach` :261, `source_closure` :267, `has_authored_ceiling` :308, `authored_ceiling_owner` :316, `written_ceiling` :358, `authored_source_ceiling` :372, `publish` :386, `union` :523. Test :536-580 (1).
3. In/out. `prepare` reads the trait-scope registry and a `ServiceCatalog`; `publish(entry, services, elaboration)` writes `elaboration.reach.{names,ceilings}`, `machine.declared_service_reach`, `machine.published_service_ceiling`, `root_service_reach.concrete` :488-521.
4. Decisions. Service uniqueness by identity string :109-116 (source spelling of the application identity, normalized by `type_identity_text`). Service lookup by `ServiceReachId` linear find :155-160. "Written" ceiling: provenance walk `authored_service_reach_rows_for(current)`, following `closed_requirement_application(..).template` or the machine/template link, with a `visited` guard :316-354. Reach fixed point: a machine with an authored ceiling is skipped :439-445; otherwise union of callee rows (module calls, callable-contract envelopes, boundary `published_service_ceiling`) :446-481.
5. Cases by shape. `source_closure` walks parents depth-first and refuses cycles "service inheritance has a cycle" :291-293. `prepare` refuses a non-boundary trait or parent :80-96.
6. Same job elsewhere. See §2 item 6 (reach derivation in `outcome.rs`).
7. Unused/one-caller (Grep `fn (has_authored_ceiling|...)|reach::(has_authored_ceiling|...)` over `source-rust/`): `has_authored_ceiling` 5 uses (`read/callables.rs:64`, `read/providers.rs:258`, `read/traits.rs:160`, plus self at :313; two more via `authored_source_ceiling` :378); `written_ceiling` 3 calls (`read/services.rs:239,742,751`); `authored_source_ceiling` 2 calls (`read/services.rs:744,1510`); `source_closure` 2 calls (`read/services.rs:51,1293`); `authored_ceiling_owner` 1 call (inside this file :313). `ServiceMap::envelope` callers: `boundaries.rs:88`, `callables.rs:210,221,231`, `external_providers.rs:94,95`, `lib.rs:1731`, `read/services.rs:1351,1374` (Grep `\.envelope\(`).
8. Stated limits. Header :6 "The receiving checker independently judges every call." :384-385 "No acceptance follows from this computation".
9. Fail-open. An open (omitted) requirement ceiling publishes nothing for that requirement: `published` starts from `unwrap_or_default()` :410-413 and only an `Authored` requirement ceiling is copied :426-433, so a call to an open requirement contributes an empty row :448-453. Stated (:418-420 "An omitted bound remains open in requirement_ceilings"; checker judges). The mismatch between `machine_envelopes` and the row is refused :498-510.

---

## 9. `<s>/read.rs` (1-3296)

1. Purpose. Module root of the reader: declares the 53 submodules (:25-77), the reader state `Program<'t>`, per-body `Slot`, small enums, and the grouping of selected public contracts. Header :1-23 states the identity rules (type is scalar only through `builtin_type_atom`; record only through a `SymbolKind::Data` symbol; slot only by symbol; `result` only through `reserved_result_of`).
2. Shape. `TypeAlternative` :92, `ConstBinding` :98, `LocalSummaryBodies` :108, `FixedContract` :116, `LocalSummaryScope` :129, `DeclaredDomain` :142, `PredicateDefinition` :149, `Program` :156-413 (about 80 fields), `Slot` :417 (`symbol`, `value_type`, `assignable`, `homed`, `erased`, `borrowed`, `borrowed_regions`), `crash_cause` :434, `span` :443, `Program::{is_generic_check_form :453, begin_declaration_storage :456, new :475 (test), exact_toolchain_sources :486, with_captures :492, finish_declarations :599}`, `symbol_kind` :718. Tests :755-3295 (19 `#[test]` by Grep count, about 2,540 lines).
3. In/out. `Program` holds `typed: &TypedTrees`, signature/contract/boundary/callable tables, records, domains, regions, scopes; it is mutated by every reader module. `finish_declarations` reads `declaration_signatures/sources/selections` and produces `declaration_aliases`, `declaration_representatives`.
4. Decisions. `finish_declarations` sorts selected contracts by key `(caller, application identity, requirement identity, body, evidence-parameter coordinates)` :656-671 and merges only equal `Some` keys :677; two contracts with one key but different signature or envelope are refused :681-689. Keyed on identity text and arena coordinates, not on display names. `is_generic_check_form` is `rigid || alternative.is_some()` :453.
5. Cases by shape. `symbol_kind` lists every `SymbolKind` :718-754 (labels for refusals). Header :20-22: "a kind outside the subset is refused with `Unsupported` and its name".
6. Same job elsewhere. `Program` is the one reader state; `Slot` fields (`homed`, `erased`, `borrowed`) are re-read by `read/calls.rs` `lent_from` :3649-3693 and `read/atomics.rs:469-477`, which both construct/test them.
7. Unused/one-caller. `LocalSummaryBodies` (:108) and `FixedContract` (:116) are used by `callables.rs:270` (FixedContract), `calling.rs:465,636`, `callback_bindings.rs:355` (Grep not run per type; counts not given). `new` is cfg(test) :475.
8. Stated limits. :21-22 "A field the subset does not serve is refused when it is set, never ignored." :105-107 "result inference currently extends a callable's contract, so only private bodies may receive those additional clauses."
9. Fail-open. none seen in production code of this file. Test at :794-840 pins that each unsupported node kind is refused by name.

---

## 10. `<s>/read/applications.rs` (1-374)

1. Purpose. Authenticates a generic call's static application (types, scalar consts, schema consts, callable binders) against the retained source tuple and splits it into types, values and callbacks.
2. Shape. `ApplicationArguments` :12, `ApplicationBindings` :33, `Program::application_bindings` :44, `Program::application_arguments` :207.
3. In/out. Reads `typed.expression_table.static_machine_application*`, `machine_type_parameters`, caller's `declaration_type_parameters`, `self.scope`, `self.const_scope`; returns `ApplicationArguments`.
4. Decisions. Source owner check :73-85 (`lexical_owner` vs caller; `registration_operation` vs target) by symbol identity. "Inherited" (no bindings and every parameter belongs to the caller's declaration) :122-168. Binder kinds matched on `TypeParameterKind::{Machine, Type, Const}` :227-283. A record `Const` actual compares `account.application` strings :337 (application identity text).
5. Cases by shape. Refuses an application carrying `selected_machine_arguments`, `input_selections`, `lifetime_bindings`, `conformance_selection`, or a callable contract source: "generic application has another static feature" :86-95.
6. Same job elsewhere. Parallel tuple checks in `read/calls.rs:1972-2018` (`static_value_arguments`) and `read/calls.rs:2224-2272`; suspected overlap, I read both.
7. Callers: grepped within the unit: `application_arguments` called at `calls.rs:2098`, `callback_bindings.rs:319`, `calls.rs:2694`.
8. Stated limits. none.
9. Fail-open. none seen: every mismatch returns `unavailable(..)`.

---

## 11. `<s>/read/arrays.rs` (1-1298)

1. Purpose. Resolves array and slice types (fixed arrays with literal, parameter or application extents; typed element views), array literals, indexing, ranges and borrowed range views, collection recasts, and storage rows for primitive scalars.
2. Shape. `SourceSequenceView` :26, `source_sequence_view` :31, `Program::{extent_input :80, byte_array_element :112, typed_array_element :137, borrowed_array_range :181, collection_recast :492, array_literal :584, indexed :639, array_element :781, storage_value_type :817, sequence_type :839, array_type :952, collection_element_storage :1062, storage_type :1077, storage_identity :1146, storage_source_application :1163}`, `physical_source` :1175, `physical_source_application` :1182, `ElementRead` :1295. Test module :1194-1290 (1 test).
3. In/out. Reads `records`, `record_sources`, `const_scope`, `typed.type_reference_table`; pushes `RecordType` + `RecordSource` rows; returns `Expression` / `ValueType`.
4. Decisions. Row reuse by shape equality: `records.iter().find(|(_, row)| row.shape == shape)` :930, :1036, :1120 (linear scan; identity of the new row is its `StorageShape`). Extent: `FixedArrayLength::ConstParameter` becomes `ArrayExtent::Parameter` if the const input exists, is unsigned, at most 64 bits :968-994; otherwise declared literal / recipe :995-1017. Access mismatch rules in `borrowed_array_range` :262-306.
5. Cases by shape. `array_element` accepts `FixedArray` (literal or parameter length) and `ElementView` only :786-814. `storage_type` refuses an element carrying a domain unless `type_purpose == StorageDescription` :1083-1119 ("array of elements that carry a declared domain"). `byte_array_element` demands a closed `u8` array with no domain :112-130. `source_sequence_view` rejects write-only :52-54.
6. Same job elsewhere. Array-extent matching in `read/arrays/argument_types.rs` (§12) and `calls.rs:3338-3400`. Byte-view casing duplicated with `bytes.rs` (`byte_view_record` :978 vs `sequence_type` :839).
7. Unused/one-caller. Callers within the unit: `borrowed_array_range` from `calls.rs:3462`; `physical_source` from `atomics.rs:77`.
8. Stated limits. :39-40 "Non-byte qualification needs its own predicate owner. Keep the existing refusal"; :1018-1020 constant "waits on a deployment".
9. Fail-open. none seen; the `is_some_and` on `record_sources` in `typed_array_element` returns `None` (refuse) on misses :147.

---

## 12. `<s>/read/arrays/argument_types.rs` (1-106)

1. Purpose. Decides whether two fixed-array storage types agree at a call using supplied length arguments, and rewrites the callee formal's storage type to the caller's when a refinement call forwards lengths.
2. Shape. `array_argument_storage_matches` :9, `apply_forwarded_array_lengths` :58.
3. In/out. Reads `self.records`; mutates `supplied.parameters/result` :101-103.
4. Decisions. Equal ids match; otherwise both must be `FixedArray`, equal qualification/multiplicity, no `requires`, no cleanup, and equal length or a `length_matches` closure :15-49; element recursion bounded by `0..=records.len()` :15. Forwarding requires `ArrayExtent::Parameter` on both sides with `owner == caller`/`callee` and matching input/formal positions :70-89.
5. Cases by shape. FixedArray-only :23-37.
6. Same job elsewhere. `calls.rs:3369-3399` supplies a second `length_matches` closure to the same matcher.
7. Callers: `apply_forwarded_array_lengths` 1 (`lib.rs:868`, Grep); `array_argument_storage_matches` 3 (here :101, `calls.rs:3369`, definition).
8. Stated limits. none.
9. Fail-open. `apply_forwarded_array_lengths` returns early unless `self.rigid` :65-67. Traced: non-rigid reads leave the supplied signature unchanged (no rewrite, so no widened acceptance).

---

## 13. `<s>/read/atomics.rs` (1-484)

1. Purpose. Reads atomic cell types (`AtomicBool`, `AtomicU32`, `AtomicU64`), atomic loads, stores, fetch/swap bindings and `atomic_fence` calls into `AtomicOperation` expressions.
2. Shape. `atomic_type` :24, `atomic_place` :82, `atomic_load` :162, `atomic_fence` :199, `atomic_store` :306, `atomic_operand` :348, `atomic_binding` :366.
3. In/out. Reads typed atomic expressions, `AtomicOrderingPlan`; produces `ExpressionKind::Atomic(AtomicOperation::{Access, Fence})` or `Statement::{Evaluate, Bind}`.
4. Decisions.
   - Cell type from `builtin_type_atom` (3 atoms) :41-50, by symbol identity.
   - Fence: the sealed declaration is found by `sealed_declaration(target, machine.name.as_str()) == AtomicFence` :213-221 (name string passed in); the argument's case is chosen by variant name strings `"Receive" | "Publish" | "ReceivePublish"` :292-297 (source spelling of the case name).
   - Fetch/swap binding requires the initial value be literal zero :383, the following statement be an assignment of an atomic expression whose `result` names the local :389-402, and the binary operator be one of add/sub/and/or/xor with `ArithmeticDomain::Exact` :408-420.
   - Access authority through `Access::{Read, Write}` :95, :141-158.
5. Cases by shape. Only `AtomicBool/U32/U64` :41-50; fetch and swap only on integer cells :442-448; ordering plans `Load`, `Store`, `ReadModifyWrite`, `Swap` :169, :314, :403-440 else refusal "atomic exchange result custody" :434-438.
6. Same job elsewhere. The `authorized(.., Access::Read/Write)` + `storage_path::walk` pair at :141-158 repeats `authority.rs::written_through` / `representation_read_authorized` :137-167 (read).
7. Unused/one-caller. `atomic_fence` 1 call (`calls.rs:2612`); `atomic_binding`: seen in aggregate grep (1 in this file; call site not located).
8. Stated limits. none.
9. Fail-open. `atomic_fence` returns `Ok(None)` (not an atomic fence) when the target is not a machine or its sealed declaration differs :210-222 — then the call proceeds as an ordinary call. Traced: a user machine that is not the sealed core fence is by design an ordinary call.

---

## 14. `<s>/read/authority.rs` (1-299)

1. Purpose. Single question owner for "may the machine being read do this to the contents of a value, given the declared domains on its type". `Opened` is the proof token a record lookup must carry.
2. Shape. `Access` :43 (`Read`, `Borrow`, `Replace`, `Write`), `Opened` :59 (`constructing` :66, `record` :70), `Program::{authorized :79, opened :110, opened_place :124, written_through :137, representation_read_authorized :152, zero_established :220}`, `has_predicates` :296.
3. In/out. Reads `domain_theory(id, domain)` for each domain in `value_type.qualification().domains` :86-87; returns `Checked<()>` / `Option<Opened>`.
4. Decisions. `authorized` refuses only when `definition.access_unjudged()` :91-98 (keyed on the domain definition's plan state). `zero_established` refuses a zero value in a domain with predicates unless the position is contained in a record :236-238, and refuses `TypeParameter`, `Opaque`, mutable `ElementView`, `Reference` shapes :246-283. `has_predicates` is `predicate_presence != Absent || !facts.is_empty()` :296-298.
5. Cases by shape. `zero_established` lists every `StorageShape` :245-288. `representation_read_authorized` accepts `Record`, `FixedArray`, `ElementView`, borrowed `ByteSequence`, `PrimitiveScalar` and refuses the rest :179-203.
6. Same job elsewhere. Duplicated read/write path checks in `atomics.rs:141-158` (see §13).
7. Unused/one-caller. `Opened::constructing`: Grep over the crate gave 4 uses (`read.rs:2418` test, `arrays.rs:602`, `places.rs:1324`, plus definition). `has_predicates`: Grep `has_predicates\(` gave 6 callers (`authority.rs:236`, `casts.rs:571,582`, `predicates.rs:273,515,652`) plus definition :296 and the `pub(crate) use` in `read.rs:30`. The header (:17-21) says `Opened` "comes only from here"; `Opened::constructing` is `pub(super)` and used outside this file (`arrays.rs:602`, `places.rs:1324`), so "only from here" holds for the module, with a documented second route.
8. Stated limits. :26-30 "An unevaluated plan cannot justify the operation, so its use is refused".
9. Fail-open. `authorized` accepts when the type has no domains or every domain's plan is judged :86-104 (stated :100-103: "Pure predicates restrict validity, independently of access"). A value of type with a domain that has no policy is accepted for all four access kinds (stated :30-31 quoting access.md).

---

## 15. `<s>/read/boundaries.rs` (1-337)

1. Purpose. Resolves bodyless requirements (trait boundary requirements, boundary/external-realization machines, intrinsic and hosted primitives, layout facts) into `BoundarySource` rows that own the call and its completion.
2. Shape. `Program::{names_boundary_carrier :13, boundary_application_is_empty :40, read_boundary_declaration :133}`.
3. In/out. Reads `typed.trait_machine_signature`, `machine_by_own_or_state_symbol`, sealed intrinsics; pushes `BoundarySource` rows into `self.boundaries.rows` and calls `admitted_declarations.include(target)`.
4. Decisions. Branches on `sealed_intrinsic(target)` value: `LayoutTargetFacts` :158-183, `FloatOperationRequirement` :184-221, `HostedPrimitive(op)` :222-283; then trait boundary :284-336. Rows are found by `source == target` linear find :150-155. Hosted primitive requires exactly one conformance edge whose requirement has `HostedRequirement(operation)` for the same operation :243-251 and an identical declared signature :260-266.
5. Cases by shape. Three sealed intrinsic kinds then `trait_machine_signature`; a generic boundary trait without a closed application refused "boundary call needs its complete declaring trait application" :290-297. `Operator` vs `Machine` kind from `IntrinsicRequirementKind` :197-200.
6. Same job elsewhere. `names_boundary_carrier` (:13) and `boundary_application_is_empty` (:40) both repeat the test `!body_is_present && supply_mode in {Boundary, ExternalRealization}` (:30-36, :54-61, :141-148): three copies in this file.
7. Callers: `names_boundary_carrier` via `names_requirement` (`calls.rs:379`); `read_boundary_declaration` from `calls.rs:88,2618`, `calling.rs` tests and `read/boundaries.rs:253` (recursion).
8. Stated limits. none.
9. Fail-open. `boundary_application_is_empty` returns `Ok(false)` (not a boundary) for non-boundary traits and bodied machines :48-50, :53-58, :63-65. Traced: caller `applies_nothing` then goes on to check the ordinary static-application rules.

---

## 16. `<s>/read/byte_observations.rs` (1-127)

1. Purpose. Reads Core byte-property declarations (`byte_observation_declaration`) as `ByteSequencePredicate` expressions over a shared byte view, in runtime calls and contract calls.
2. Shape. `byte_observation_declaration` :15, `byte_observation` :22, `byte_observation_clause_call` :73.
3. In/out. Produces `ExpressionKind::ByteSequencePredicate{base, predicate}` with result `bool`, or `(predicate, argument)` for a clause.
4. Decisions. Declaration recognized by `source_state::byte_predicates::byte_observation_declaration(typed, target)` :19 (not read). Exactly one argument :35, :116. Formal/argument type agreement via `same_type` :55. Contract form refuses any other call feature (list of 13 conditions) :96-110.
5. Cases by shape. One shared byte-view argument.
6. Same job elsewhere. The 13-condition "no other call feature" list repeats `calls.rs:1181-1195` (`borrow_factory_call`).
7. Callers: `byte_observation` 1 (`calls.rs:2615`).
8. Stated limits. none.
9. Fail-open. `Ok(None)` when the target is not a byte-observation declaration :30-32, :78-81 (falls through to ordinary call reading).

---

## 17. `<s>/read/bytes.rs` (1-1467)

1. Purpose. Reads byte views, byte literals, indexing/subslicing/measuring of collections, domains on byte views, and mutable-view re-lending, using the terminal byte carrier.
2. Shape. `source_byte_view` :27, `SourceByteView` :100, `byte_view_matches_carrier` :110, `byte_record_source_matches` :141, `mutable_byte_formal_identity` :169, `byte_view_domains` :200, `Program::{is_readable_byte_view :272, is_plain_readable_bytes :289, byte_element :306, collection_index_operand_types :338, collection_count_operand_type :351, collection_range_selection :371, byte_subslice :403, collection_window :465, collection_measure :527, measured_collection :541, is_borrowed_view :673, retain_view_input :694, byte_view_element_type :762, view_type :789, view_type_with_formal_access :797, retain_byte_domains :864, byte_literal :911, byte_view_record :978, logical_byte_domain_carrier :1051, view_predicates :1114, byte_qualification_predicates :1134}`. Tests :1168-1466 (2).
3. In/out. Reads source slice/type applications; pushes `RecordType` rows with `ByteSequence` shape; updates `self.byte_domains` :886-905.
4. Decisions. Byte element recognized by builtin atom resolving to unsigned 8-bit non-address integer :48-57. Access: Shared/Mutable ok, WriteOnly refused :58-62; a mutable view with domains refused :67-69. A domain whose predicate is `NonEmpty` is refused on a view :240-243. Quoted literal to a scalar place rejected with message :935-945. Row identity for a byte view is its carrier + identity text; reuse by `record.shape == ByteSequence(carrier)` with no declaration symbol :983-989. The `collection_count_operand_type` finds the `u64` type reference by scanning all named references :359-368.
5. Cases by shape. `measured_collection` handles fixed-array length (extent Literal / Parameter, refuses Application) :567-637, else descriptor `length_operation()` :645-657. `Length` is the only measure for fixed arrays.
6. Same job elsewhere. `byte_view_record` (:978) and `arrays.rs::sequence_type` (:839) both find-or-push a view row by shape; `is_borrowed_view` here (:673) vs shape matches in `calls.rs:3411-3425 observes_only`.
7. Unused/one-caller. none individually grepped.
8. Stated limits. :63-66 "Qualified mutable, write-only, and access-unspecified view fields remain unsupported". :67-69 mutable view with domain refused ("nothing here follows" the loan).
9. Fail-open. `byte_view_domains` accepts `Some(_)`/`None` predicates alike :240-246 (only `NonEmpty` refused). Open: a non-`NonEmpty` byte predicate on a view is kept without being checked here; I did not trace the caller `retain_byte_domains` :864-908 (it records `SourceByteDomain` and says domains are proved elsewhere via "ordinary structural predicates" :195-198, stated).

---

## 18. `<s>/read/callables.rs` (1-349)

1. Purpose. Reads a stored callable type (`callable_type`) into a `CallableContract` row shared by equal identities, and reads a machine value (a named machine used as a value) into a `MachineValue` expression with a `MachineSelection` descriptor.
2. Shape. `unavailable` :19, `callable_alias_declaration` :32, `require_closed_callable` :47, `callable_type` :85, `machine_value` :203, `selection` :324.
3. In/out. Pushes `callables.contracts`, `callables.descriptors`, `fixed_contracts`; returns `ScalarType::Callable(id)`.
4. Decisions. `require_closed_callable` refuses a missing authored service ceiling ("write reaches") :64-70 and an open crash ceiling unless `declares_no_crash_route` or a `Crashes` clause exists :71-81. Contracts are shared when `row.identity == identity` (string compare of the exact identity) :141-145, with lifetime-binder arity and complete signature/envelope equality enforced :147-185. A machine value must select a `Machine` or `State` symbol, whose state is the entry :246-262; closed named selection only :234-239.
5. Cases by shape. Runtime callable with static type parameters refused :111-116; a rigid type in the signature refused :123-133.
6. Same job elsewhere. `selection` (:324) find-or-push by 5-field equality, analogous to `finish_declarations` coalescing in `read.rs:599`.
7. Callers: `require_closed_callable` used `callables.rs:117`, `boundaries.rs`/`calls.rs:2108`, `calling_abis`-no; `callable_type` from `calls.rs:2953`, `read.rs:1386` (test).
8. Stated limits. :265-267 "A stored selection opens the helper's possible-use boundary".
9. Fail-open. none seen.

---

## 19. `<s>/read/calling.rs` (1-1642)

1. Purpose. Prepares the selected Calling policies and program-entry contracts: validates each selected boundary application against the shared calling catalog, reads the policy machine as a fixed-interface body, collects declared ABI signature ids, and prepares inherited boundary parent contracts.
2. Shape. `SourceCallingPolicies` :24, `SourceCallingHeader` :33, `SourceCallingSignature` :38, `SourceProgramEntryContract` :74, `SourceCallingRequest` :82, `SourceCallingInstance` :88, `application_identity` :95, `Program::{read_calling_requirement_header :106, prepare_calling_header_dependencies :140, prepare_calling_requirement_header :161, read_calling_requirement_headers :205, require_complete_boundary_parent_contracts :228, prepare_calling_policies :260}`. Tests :654-1642 (9).
3. In/out. Reads `CallingPolicySelections`, `self.shared.calling_catalog`, `typed` application identities; writes `prepared.{requests, signatures, entries, completed_applications, headers}`, pushes `fixed_contracts`.
4. Decisions. Selected schema applications are matched to catalog instances by `portable_application_identity` string equality and `argument_owner` equality :424-432 (unique match required or "competing prepared instances" :430-438). Entry nominations collide if two have the same target :576-582 or differing declaration/application/digest/entry :617-631. Pointer equality of the typed owner :269.
5. Cases by shape. A selected scheme must be a boundary declaration :336-341; a missing relationship is accepted only for a boundary-parent contract that is not otherwise selected :367-375.
6. Same job elsewhere. ABI/instance enumeration repeated in `calling_abis.rs:51-130` (`prepare_calling_policy_roots`) and `:136-311`; both walk `calling().requests()/instances()/signatures()`.
7. Callers (aggregate Grep of `prepare_calling_policies|read_calling_requirement_headers|...`): within the crate `prepare_calling_policies` appears in `lib.rs`/this file; `read_calling_requirement_headers` 1; I did not split per name.
8. Stated limits. :454-455 "never an inferred contract fabricated from the machine body".
9. Fail-open. `Ok(false)` when there are no environments and no entries :289-291 (nothing to prepare). The `(None, None)` arm :368-375 accepts a missing Calling request for an inherited boundary parent: stated by the condition (only for parents not otherwise selected); the inherited contracts are still prepared :386-391.

---

## 20. `<s>/read/calling_abis.rs` (1-1276)

1. Purpose. Reads calling-policy roots and ABI declarations for boundaries read into the module; prepares per-ABI parameter/result storage and publishes `CallingAbiDeclaration` rows and boundary-to-ABI associations onto the module.
2. Shape. `PreparedCallingAbis` :18, `PreparedAbi` :24, `PreparedStorage` :33, `unavailable` :39, `Program::{prepare_calling_policy_roots :51, prepare_calling_abis :136, calling_abi_projected_storage :313, calling_abi_storage :415, calling_abi_parameter :440}`, `PreparedCallingAbis::publish` :515, `intern_telescope` :586. Tests :623-1276 (7).
3. In/out. Reads the shared calling catalog and `boundaries.rows`; writes `module.interfaces.calling_abi_signatures`, `calling_signatures` :551-581.
4. Decisions. ABI row identity is `CallingAbiIdentity` (requirement identity text, declaring application text, operands) and `identity.id() == row.id` is required :270-283. Projections must agree on formals/default/owner :324-329 and on storage in the selected `purpose` context :398-410. Parameter reference lifetimes refused if named :468-472. `calling_abi_parameter` classifies the outer `Reference` shell by `ReferenceAccess` :463-482.
5. Cases by shape. Callback storage refused: "calling ABI callback storage has no retained shared projection" when binders or direct callbacks exist :331-335. Slice+Mutable formal uses a different descriptor path :489-497.
6. Same job elsewhere. See §19 item 6.
7. Callers: `prepare_calling_abis` 1 in `lib.rs`; `prepare_calling_policy_roots` 1 (aggregate grep; per-name counts not split).
8. Stated limits. :134-135 "An unsupported storage shape remains an explicit per-declaration refusal".
9. Fail-open. `prepare_calling_policy_roots` returns `Ok(())` when there is no declaration metadata and no boundary rows :52-54; `if !wanted { continue }` skips a calling request whose boundary is neither in metadata nor read :76-78 (stated :49-50 "A calling policy is checked with the product's other executable roots"). Open: a calling policy for a boundary that is not read is not checked here.

---

## 21. `<s>/read/calls.rs` (1-4577)

1. Purpose. Call family of the reader: which callee a call names, authenticating each static application and authored selection, then reading the call (direct, boundary, selected requirement, rigid parameter, named map, runtime callable, layout/storage queries) with argument loans.
2. Shape (main). `StaticInvocation` :40, `CallBinding` :53, `NamedCallBody` :61, `StaticMachineTarget` :74; `Program::{boundary_declaration :83, applies_nothing :102, ordinary_application_target :236, checking_generic_entry :266, closes_rigid_parameters_only :291, entered_machine :324, instantiates_callable :345, names_requirement :379, named_call_target :386, direct_selection :534, bound_target :649, call_statement :660, call_expression :756, read_call_expression :769, storage_location :979, layout_query :1103, borrow_factory_call :1154, designate :1339, requirement_call :1441, selected_call :1605, named_call :1716, bound_scalar_values :1830, bound_scalar_value :1893, static_value_arguments :1961, generic_application_call :2023, const_operands :2161, call_binding :2224, static_invocation :2276, rigid_named_call :2366, read_rigid_call :2398, call :2528, call_selected :2540, call_from :2564, inherited_application_arguments :2663, call_target_symbol :2707, visible_to :2739, visible_declaration_from :2788, call_of :2840, call_of_values :2861, source_call_signature :2888, runtime_call :2944, call_with_signature :3004, call_with_signature_values :3027, const_array_argument_agrees :3338, observes_only :3404, lent :3436, lent_from :3450, borrowed_argument_type :3752}`; free `lent_storage_is_untouched` :3849 with `Lent` :3916, `place_path` :3924, `Touches` :3961. Tests :4177-4576 (3).
3. In/out. Reads typed call nodes (`TableCallExpression`, `TableCall`), signatures, boundaries, selections; produces `ExpressionKind::Call{callee: CallTarget::{Machine, Boundary, Selected}, arguments, ...}` and pushes `declaration_signatures/envelopes/sources/selections`, `selections`, `nominal_calls`, `names.calls/loans`.
4. Decisions.
   - Direct call: the authored selection must resolve to the called machine or its single entry state, or an instantiation of it; a binding must be unrebound or joined by the recorded application correspondence; all comparisons are of symbols :534-642 (stated :530-533).
   - Visibility: public declaration passes; otherwise caller and callee must have the same `(file.origin, package_identity)` pair :2788-2826; a selection exposed as `PublicInterface` makes a private callee refuse :2799-2803.
   - Call arguments: `erased` formal gets a term, `borrowed` formal gets a loan, else an expression :3134-3149; each argument must `agrees`/`same_type` the substituted formal, else `mistyped` :3222-3273.
   - Borrow conflicts: `lent_storage_is_untouched` compares storage paths with `terminal_semantics::storage::place_paths::may_overlap` :3972-3978 (stated :3829-3835 "judged by field path, not by root").
   - Fence/byte-observation/boundary shortcuts in `call_from` :2612-2628 keyed on sealed declaration and byte observation declaration.
   - Named call target by member selection from the source conformance :2257-2267.
   - Builtin function `AddressOf` recognized by `builtin_function_for_symbol(..) == Some(BuiltinFunction::AddressOf)` :787-788.
5. Cases by shape. Quote: "refuses ... receiver call" :725; "interrupt, requirement dispatch or reflected call" :708; sealed borrow: only `AccessBorrowBacking::Ordinary` :1200-1204; a sealed borrow factory must satisfy a 10-condition closed-signature test :1230-1249; `generic_application_call` handles boundary methods with `Type|Const` parameters and machines with `Type|Const|Machine` parameters only :2036-2077; `closes_rigid_parameters_only` lists `Implementation|Const|Type|Machine{Structural|Nominal}` :304-316; `requirement_call` refuses 12 other-feature conditions :1450-1491; `named_call_target` closed vs `SymbolicMap` :397-459.
6. Same job elsewhere. Application-tuple authentication repeated: `applications.rs` (§10) vs `static_value_arguments` :1961-2019, `bound_scalar_values_in_scope` :1839-1890. The no-other-feature condition list is repeated at :1181-1195, :1456-1464, :2300-2311, :2323-2328, and in `byte_observations.rs:96-110`. Visibility logic duplicated in `visible_to_from`/`visible_declaration_from` plus `named_call_target` :394-395.
7. Unused/one-caller. Aggregate Grep of `byte_element(`, `rigid_named_call(`, `names_requirement(`, `closed_callback_requirements`: calls.rs has 5 hits; `rigid_named_call` and `closed_callback_requirements` are `pub(super)`/`pub(in super::super)` (call sites not separately counted).
8. Stated limits. e.g. :1200-1204 "resident view needs evaluated layout, content, profile and policy custody"; :2465-2467 "rigid parameter call with another call feature"; :3824-3828 "until loan timing is represented".
9. Fail-open.
   - `visible_declaration_from`: public declaration returns `Ok` without a package check :2796-2798 (by design, stated :2776-2778).
   - `bound_target` accepts any valid symbol; invalid refused :649-658.
   - `lent_storage_is_untouched` returns `Ok` when nothing is lent :3877-3879; a `Lent` whose path steps contain an unknown field identity is truncated by `map_while` :3957 (paths merge, "leaving it out only merges paths, never separates them" stated :3911-3914).
   - `ExpressionKind::Block` is treated as touching storage whenever anything is lent :4120 (conservative).
   - `ExpressionKind::Call` argument selectors `touches.expression(.., false, ..)` treat call arguments as non-changing :4166-4169; a call argument that moves a lent whole slot is detected only via `place` :4040-4043 (traced by reading; not run).

---

## 22. `<s>/read/calls/callback_bindings.rs` (1-439)

1. Purpose. Binds a supplied static callback to its declared requirement and the authored selection occurrence; checks conformance of the supplied machine to the required header and records a `SourceConformance` row and a static-reach parameter.
2. Shape. `RequiredCallback` :10, `CallbackConformance` :16, `Program::{static_machine_target :25, static_argument_selections :55, static_argument_selection :95, retain_static_reach :148, callable_application_arguments :172, closed_callback_requirements :243, callback_conformance :285, static_reach_parameter :409}`.
3. In/out. Reads `typed.static_machine_binding`, `authored_declaration_selections`, `retained_static_machine_contract`, `scopes`/`source_scopes`; pushes `static_contracts.conformances`, `fixed_contracts`, `static_reach_parameters` (sorted insert :433-436); sets `static_reach_binding` on the call :167.
4. Decisions. The authored selection must be kind `StaticArgument` and resolve to the argument symbol, an instantiation of it, or the same entered machine :105-114; its span must index a non-empty slice of a real source file :120-134. Callback parameter position found by `declaration_type_parameters(source)` position :421-427. Conformance row index is `static_contracts.conformances.len()` :379.
5. Cases by shape. Supplied callable with member/lifetime application refused :310-318; with scalar const inputs refused "supplied callable retains scalar const inputs" :343-348.
6. Same job elsewhere. none.
7. Callers: `static_machine_target` 1 (`calls.rs:2255`) and tests; others via `calls.rs:138,2129`.
8. Stated limits. TASKS.md names this file (EARLY-TERMINAL-JOBS, from the crate map); in-file: :341-342 "A selected callable with scalar consts needs a fully applied required header".
9. Fail-open. `static_machine_target` returns `(target, None)` when the binder is invalid :31-33 (not a callback call). Traced: such a call is read as an ordinary call.

---

## 23. `<s>/read/calls_visibility_tests.rs` (1-816)

Test fixture file (`#[cfg(test)] mod`, `read.rs:37-38`). One group entry.

1. Purpose. Pins that retained private boundary schemas and callback selections grant no source-call visibility across packages, and that selected boundary declarations keep their crash/operational contracts.
2. Shape. `Fixture` :20 with `static_callback` :30, `new` :154, `with_effects` :158, `with_scope_and_effects` :167, `with_owner_provenance` :176, `call_source` :326; 14 `#[test]` functions :331-798 (Grep count 14).
3. In/out. Builds `TypedTrees` by hand with two packages (`app`, `dep`) and runs `Program` methods (`machine`, `call`, `call_selected`, `applies_nothing`, `selected_declarations`, `witnessed`, `elaborate`).
4. Decisions pinned: refusal texts "call private machine of another package", "call static call changed its selected target", "call static call argument changed its authored selection", "call private machine in a public interface", "boundary requirement has an open crash ceiling; write crashes".
5-9. none (tests).

---

## 24. `<s>/read/carriers.rs` (1-554)

1. Purpose. Answers `integer::carrier::<T>().{integer, signed, fixed_width}` in static conditions from a type's carrier identity; lets a generic check form take its classifying `requires` clauses as premises about rigid type parameters.
2. Shape. `Facet` :39, `CarrierFacts` :60, `Written` :116, `Bound` :125, `Scope` :133, `Program::{bound :140, resolve_written :159, value_carrier :185, written_carrier :207, is_data :257, called_machine :262, is_carrier_query :280, call_bindings :295, carrier_read :353, carrier_member :383, assume_premises :397, premise :434, contract_value :474}`, `integer_sign` :549.
3. In/out. Reads written types, symbols, call bindings, callee contracts; writes `self.rigid_carriers` :402-428.
4. Decisions. Facet name from the member field's spelling: `Facet::of_field` matches `"integer"`, `"signed"`, `"fixed_width"` :46-53 and `carrier_read` uses `member.member.as_str()` :373 (source spelling of the field name). The carrier query is recognized by `core_vocabulary(machine.symbol) == IntegerCarrier(Query)` :280-288 (identity, not name; header :13-14 "Nothing here knows the four query names"). Integer-ness: `builtin_type_atom`/`integer_family` on the resolved symbol :243-254; `Data` symbols are non-integers :254, :257-259.
5. Cases by shape. `premise` learns only: a carrier read, `&&` when true, `!`, a callee whose contract is a single `ensures result == E` over a no-argument, no-crash, terminating, no-`requires` machine :434-466, :474-546. `written_carrier` returns `None` for `Named`/`Constrained`/`ConstExpression` references it cannot resolve :234-236.
6. Same job elsewhere. none.
7. Callers within unit: `assume_premises` etc. used from `machine.rs`/`predicates.rs` (outside unit, not read).
8. Stated limits. :25-27 "any other clause teaches nothing, so a requirement that needs it stays open and is not established" (:392-396).
9. Fail-open. `premise` ignores any clause shape it does not recognise (`_ => {}` :464) — stated :394-396: unlearned premises leave requirements open, so unrecognised premises do not add facts. Contradicting premises refuse :419-427. `CarrierFacts::default()` for an unknown rigid is all `None` :198 (no fact).

---

## Aggregate greps used above

- `retain_callee_spans|operational::positions|operational::boundary|operational::retain` over `source-rust/` (content mode): 11 lines (counted in §1).
- `outcome::outcomes|counted_outcomes|outcome::Outcome` over `source-rust/` (content mode): 15 lines (§2).
- `declaration_custody` over `source-rust/` (content): hits listed in §4.
- `policy::...` over `source-rust/` (content): hits listed in §3.
- `HashMap|BTreeMap|HashSet|BTreeSet` over the 24 unit files (count mode): 5 hits, all in `outcome.rs` (`BTreeSet<CrashCause>` :41, :171, :228, :423; no id-keyed maps). No `HashMap`/`BTreeMap` in this unit.
- `#[test]` count over unit files: policy 4, reach 1, read.rs 19, arrays 1, bytes 2, calling 9, calling_abis 7, calls 3, calls_visibility_tests 14 (total 60 in these files).
- Linear-scan storage (read): `Qualifications` uses `Vec` linear `.find` for domains/policies/sets (:365, :386, :893, :981, :1001, :1036); `callables.contracts.iter().find` :141; `records.iter().find` by shape (`arrays.rs:930,1036,1120`; `bytes.rs:983`). `IdTable` is used for per-machine tables.

---

## Coverage

Read in full (all 24 ranges, 21,937 lines): `operational.rs` 1-253; `outcome.rs` 1-903; `policy.rs` 1-457; `proof_scope.rs` 1-941; `provider_selections.rs` 1-120; `publication.rs` 1-196; `qualifications.rs` 1-1045; `reach.rs` 1-581; `read.rs` 1-3296; `read/applications.rs` 1-374; `read/arrays.rs` 1-1298; `read/arrays/argument_types.rs` 1-106; `read/atomics.rs` 1-484; `read/authority.rs` 1-299; `read/boundaries.rs` 1-337; `read/byte_observations.rs` 1-127; `read/bytes.rs` 1-1467; `read/callables.rs` 1-349; `read/calling.rs` 1-1642; `read/calling_abis.rs` 1-1276; `read/calls.rs` 1-4577; `read/calls/callback_bindings.rs` 1-439; `read/calls_visibility_tests.rs` 1-816; `read/carriers.rs` 1-554.
Skimmed: none. Not read: none. Outside the unit I read only `representations/lowered-source-publication/selection_questions.rs:95-139` (to compare `declaration_custody`) and the unit TSV and crate map.
Test-heavy ranges were read but are described only as groups: `read.rs:755-3295`, `calling.rs:654-1642`, `calling_abis.rs:623-1276`, `calls.rs:4177-4576`, `calls_visibility_tests.rs`.

## Five things a maintainer most needs to know about this unit

1. Every refusal in these files is a `refuse(.., Unserved::*)` string or `reject(..)` message, and callers (and tests) match on the exact text (e.g. `calls_visibility_tests.rs:353-355`, `read.rs:1180-1181`, `calling_abis.rs:1273`). Changing a message changes test results.
2. Reach and outcome rows are published structurally and left to the checker: `outcome.rs` (from closed bodies, §2) and `reach.rs:435-487` (from the lowered module) are two separate fixed-point derivations of service reach, and an omitted requirement ceiling contributes an empty row in `reach.rs` (§8 item 9).
3. `proof_scope.rs` contains a local `declaration_custody` (:705) that differs from the shared one in `representations/lowered-source-publication/selection_questions.rs:104` (stale-symbol handling and a builtin-atom arm), and the file calls both (:409 shared, :650/:866 local).
4. `read/calls.rs` repeats the "no other call feature" condition list in five places (:1181, :1456, :2300, :2323) and in `byte_observations.rs:96`, and the sealed fence/borrow/layout calls are recognized by sealed declaration identity, except the fence case names (`"Receive"`, `"Publish"`, `"ReceivePublish"`, `atomics.rs:292-297`) and the carrier facet names (`carriers.rs:46-53`), which are matched by source spelling.
5. Domain, row and set lookups in `qualifications.rs` and the storage-row find-or-push in `arrays.rs`/`bytes.rs` are linear `Vec` scans keyed on `DomainInstance` / `StorageShape` equality (no `HashMap`/`BTreeMap` in the unit other than `BTreeSet<CrashCause>` in `outcome.rs`).

## What you are unsure about

- Per-function caller counts for `read/calling.rs`/`calling_abis.rs` functions and `atomic_binding`, `rigid_named_call`, `closed_callback_requirements` come only from an aggregated Grep (34 hits in 18 files); I did not split them per name.
- `proof_scope.rs:592` skip of an `Err` instances row and `:541` `product_selected: true` for every machine: I did not trace the consumers.
- The 19-test count for `read.rs` is from the Grep count; I read all of the file and recognize fewer than 19 distinct test functions by eye.
- Whether `Opened::constructing` (`pub(super)`) usage in `places.rs:1324` and `arrays.rs:602` is exactly the "fresh carrier" case: I did not read `places.rs`.
- The reach derivation in `read/services.rs` (suspected third site) was not read.
