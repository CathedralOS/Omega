# Unit 106 notes

Files covered (paths relative to `<checkout>/`):

1. omega-rust/psi/compilation/validation/terminal-installation/src/scalar_applications.rs (1-2704)
2. omega-rust/psi/foundation/access-plans/src/access_fields.rs (1-747)
3. omega-rust/psi/foundation/access-plans/src/lib.rs (1-176)
4. omega-rust/psi/foundation/access-plans/src/placements.rs (1-3028)
5. omega-rust/psi/foundation/access-plans/src/plan_policy.rs (1-1189)
6. omega-rust/psi/foundation/access-plans/src/primitive_access.rs (1-1251)
7. omega-rust/psi/foundation/access-plans/src/resources.rs (1-1487)
8. omega-rust/psi/foundation/access-plans/src/tests.rs (1-7168) -- only lines 1-700 read (see Coverage)
9. omega-rust/psi/foundation/arena/src/arena.rs (1-2716)
10. omega-rust/psi/foundation/arena/src/lib.rs (1-51)
11. omega-rust/psi/foundation/arena/src/owned_index.rs (1-168)

Paths below are shortened: `SA` = terminal-installation/src/scalar_applications.rs; `AP/` = psi/foundation/access-plans/src/; `AR/` = psi/foundation/arena/src/.

---

## 1. SA: scalar_applications.rs (2704 lines, read in full)

**Purpose.** Decides, for one Terminal Psi artifact and one target, which retained "scalar conditions" (range bounds, boolean predicates, completion demands) apply, evaluates the scalar applications (constant entry evaluations and layout size/alignment observations) that those conditions read, and checks each condition against the observed values. It also holds the "entry prerequisite" check for one compile-time entry and the ABI-storage demand scope. Header comment: "Scalar application closure, entry condition selection, ABI storage demand, and layout observation checks" (SA:1-4).

**Shape.**
- `mod conditions` (SA:21-725): `ScalarConditionSelection<'artifact>` (SA:31-42; fields proof_scope, conditions, target: String, primary: bool, providers, ranges/predicates/completions as `Vec<usize>` positions into the retained condition tables, application_machines: Option<Vec<ProofDeclarationId>>, condition_scope). `select` SA:45-152; `include_evaluation_entry` SA:162-199; `matches` SA:212-238; `include_applications` SA:240-265; `discharge_ready` SA:267-311; `observations` SA:313-330; `applications` SA:332-358; `extend_unique` SA:361. Tests SA:369-724 (5 `#[test]`s, read).
- `mod prerequisites` (SA:726-1181): `ScalarEntryPrerequisites::prepare` SA:774-864, `applications` SA:894-909, `dependency_reason` SA:911-933, `admit` SA:935-1044; `AdmittedScalarEntry` SA:755-771; `ScalarEntryChecks` SA:1047-1094; `EntryCheck`/`EntryCondition` SA:1053-1069; `EntryCheck::discharge_ready` SA:1097-1179.
- `mod abi_storage_tests` (SA:1185-1479, cfg(test), top-level, 4 tests, read). Not inside a `tests` module.
- `mod closure` (SA:1480-1950): `ScalarAddressModel` SA:1494-1499; `ScalarScopeInputs` SA:1502-1509; `ScalarEntryMembership {Source, Selected(Vec), None}` SA:1512-1516 with `admits_machine` SA:1535-1553; `AdmittedScalarApplications` SA:1557-1562; `CompletedScalarScope` SA:1565-1569; `ScalarApplicationClosure` SA:1595-1605 with `new` SA:1615-1658, `for_invocation` SA:1680-1697, `for_abi_storage` SA:1699-1710, `with_deployment_scopes` SA:1712-1726, `close_scope` SA:1732-1899, `finish` SA:1901-1948; `ScalarClosureKind {Source, Deployment, Invocation(Vec), AbiStorage}` SA:1607-1612.
- Top level: `ScalarConditionScope {Source, SourceRoots, Invocation, AbiStorage}` SA:1955-1962; `AbiStorageScope` SA:1964-1968; `abi_storage_scope` SA:1970-1988; `CallingAbiStorageDemand` SA:1990-2058 (`new`, `with_borrowed_parameter`); `ApplicationObservations`/`EntryResult` SA:2060-2074; `ScalarApplicationObservations<'artifact>` SA:2076-2084 with impl SA:2111-2676 (`new`, `with_target_carriers`, `with_application_roots`, `with_invocation_roots`, `with_abi_storage`, `conditions`, `entry`, `record_value`, `store_value`, `record_entry_result`, `entry_result`, `layout_dependencies`, `observe_layout`, `extent`, `observation_ready`, `integer`, `discharge_range`, `discharge_predicate`); `ScalarApplicationValues` SA:2086-2109; `ScalarLayoutObservationError` SA:2678-2692; `validate_value` SA:2694-2704.

**Inputs and outputs.** Reads `artifact.module().scalar_applications` (conditions, proof_scope, declarations) via `TerminalArtifactView`; the admitted target/provider/layout selections (`AdmittedTargetMachineSelections`, `AdmittedProviderSelections`, `AdmittedTargetLayouts`, SA:2078-2080); target layout facts (SA:2116). Mutates only `ScalarApplicationObservations.applications` (an `Arc<Arena<ApplicationObservations>>`, `Arc::make_mut` at SA:2350, 2392) and the `ScalarApplicationClosure` bookkeeping vectors. Produces `AdmittedScalarApplications` (terminal identity, product_formation flag, primary scope values + geometry, supplemental inputs) SA:1939-1947.

**How it decides.**
- Which conditions apply: `select` builds a closure `applies(subject, address_model)` at SA:90-101. `Module` subject: always true (SA:91). `Declaration(owner)`: true if owner in the selected machines; or, only for address-model conditions, if owner is in the declaration closure, or (source formation && primary && owner in all declarations) (SA:92-98). `Application(a)`: true if `a` is in the demanded list (SA:100). Keyed on proof declaration ids and application ids, not names. Ranges use `address_model = requirement is AddressModel` (SA:112-115); predicates pass `false` (SA:124); completions pass `true` (SA:132) -- so a completion owned by any declaration in the declaration closure (not only a machine) is selected.
- The scope kind picks how machines/declarations are computed: Source/SourceRoots call `applications.membership(...)` (SA:67-72); Invocation calls `invocation_reachability` + `declaration_closure` (SA:73-81); AbiStorage selects nothing (empty vectors, SA:82).
- `matches` (SA:212-238) compares: pointer identity of the conditions table (`ptr::eq`, SA:217-223), target string equality (SA:224), `primary == (target == primary_string)` (SA:225), scope equality, provider list equality. Keyed on target spelling (string compare).
- `close_scope` (SA:1732-1899): rejects if artifact identity or selection differs (SA:1738-1744); scope-kind pairing table at SA:1745-1759; finds the scope by target string (SA:1766-1770); rejects a second close of the same scope (SA:1771); every completion demanded must have a value (SA:1788-1799); every application that already has a value must have its range/predicate/completion debt selected (SA:1775-1787, 1800-1818); then discharges every selected predicate (SA:1819-1821) and range (SA:1842-1847).
- Range check (`discharge_range` SA:2610-2642): evaluates integer-bound steps with `numerics::integer_bounds::evaluate`; rejects inverted required bounds (SA:2629) and any `low < minimum || high > maximum` (SA:2636). Predicate check (`discharge_predicate` SA:2644-2675): `Some(true)` passes, `Some(false)` and anything else (`None`, non-boolean) is an error (SA:2664-2674).
- Entry prerequisites: `prepare` keeps Prerequisite-role Range rows whose subject is Declaration/Application (SA:783-795) and Prerequisite-role Predicate rows whose subject is not Module (SA:796-802); with rows present it narrows them to those the invocation reaches (SA:803-855, `applies` SA:813-838). `admit` checks artifact/target/provider/entry equality (SA:940-952), runs `require_root_failure_admission` (SA:961-964), builds `EntryCheck`s; checks whose inputs are not all available are `pending` (SA:1035-1037) and `require_complete` rejects any pending (SA:1088-1093).
- Address model: `AddressModel` requirement range is `[0, 2^bits - 1]` of the carrier (SA:1010-1015, 1833-1838); requires `target == selected target`, `carrier.is_address()`, `semantics_version != 0` (SA:1004-1007, 1824-1827).

**Cases by shape.**
- Entry shape: a scalar evaluation entry must have no `parameters`, no `structural_parameters`, and a `Scalar` result with the declared carrier (SA:2316-2319); error text "scalar evaluation entry has runtime inputs or a different result carrier".
- Layout observation: integer carriers only (SA:2505-2509); only `LayoutMeasurement::SEMANTICS_VERSION` (SA:2478-2482).
- Array extent: unsigned must fit u64, signed must be non-negative (SA:2522-2539); non-integer rejected.
- Condition observation: only integer carriers (SA:2552-2554, 2566-2568).
- Borrowed-parameter demand: the formal must match `reading_borrow_pair_pattern!()` (SA:2035-2040).
- `matching_scope` is a fixed 4-pair table (SA:1745-1759) with `_ => false`.

**Same job elsewhere.**
- Address-range bound `0..2^bits-1` computed twice: SA:1010-1015 and SA:1833-1838.
- `TerminalScalarValue::Integer` to `BigInt` conversion twice: SA:1117-1127 (`EntryCheck::discharge_ready`) and SA:2597-2607 (`integer`).
- Three loops that walk range steps / predicate leaves for observations: `discharge_ready` SA:281-309, `observations` SA:313-330, `ScalarEntryPrerequisites::visit` SA:870-892.
- "root list must be Callable and deduplicated": SA:2186-2197 (`with_application_roots`) and SA:2227-2237 (`with_invocation_roots`); plus dedup-by-contains at SA:1686-1690 (`for_invocation`) and `extend_unique` SA:361.
- Suspected (not read): `omega-rust/psi/compilation/build-time-evaluation/src/scalar_applications.rs` drives this API (grep `ScalarApplicationClosure|ScalarEntryPrerequisites|CallingAbiStorageDemand`, count mode: 13 hits there, 18 in this file, 3 in terminal-installation/src/lib.rs, 3 in provider-planning calling_program.rs, 5 in calling_policy_plans.rs).

**Unused / one-caller.**
- `include_applications(` : 2 files, 1 call-site definition here (SA:240) and 1 hit in build-time-evaluation/src/scalar_applications.rs (grep count mode; no other callers). One external caller.
- Combined grep `include_evaluation_entry|require_product_formation|with_application_roots|with_invocation_roots|with_deployment_scopes|for_invocation|for_abi_storage|with_borrowed_parameter` outside this file: 6 hits in 4 files (artifact_admission.rs 1, calling_policy_plans.rs 1, calling_program.rs 1, build-time-evaluation scalar_applications.rs 3). Not broken down per method.
- No comment in this file claims "the single/canonical way".

**Stated limits.** None found by comment. Error strings state refusals, e.g. "entry scalar admission does not establish Product declaration formation" (SA:1574).

**Fail-open risks.**
1. `ScalarEntryMembership::admits_machine` SA:1545-1547: when membership is `Selected(_)` but the artifact has zero scalar conditions (`conditions.is_empty()`), returns `true` for every machine, even ones not in the selected member list. Wrong program that would get through: an invocation-scoped admission over an artifact with no scalar conditions admits a machine outside the reachable set. No comment explains. Status: open (callers not traced).
2. `include_evaluation_entry` SA:163-169 returns `Ok(())` without doing anything for Source/SourceRoots scopes. Status: open (inferred: those scopes already select by whole membership; no comment).
3. `discharge_ready` SA:267-311 silently skips conditions whose observations are not ready and returns `Ok`. Status: traced -- `close_scope` re-discharges all selected predicates and ranges (SA:1819-1847) and errors on missing values (`integer` SA:2586, `discharge_predicate` SA:2654-2663), so an unready condition cannot be silently accepted at close; only pre-close.
4. `ScalarEntryPrerequisites::prepare` SA:826-828: a prerequisite whose owner is a `LayoutObservation` application is dropped (`false`) from the entry prerequisite rows. Wrong program: an entry-level prerequisite on a layout-observation owner is not checked at entry admission. Status: open; inferred it is checked when that application is demanded (select by `Application` subject, SA:100, 240-265) but not traced.
5. `ScalarApplicationClosure::finish` SA:1910, 1921, 1929: the "retained condition has no target application" coverage check runs only when kind is `Source`; Deployment/Invocation/AbiStorage skip it. Status: open (no comment).
6. `with_application_roots(None)` SA:2181-2183 returns the observations unchanged (scope stays `Source`). Status: open.
7. `ScalarApplicationValues::value` SA:2090-2093 and `observed` return `None` for unknown ids rather than error; callers treat None as "not completed". Status: traced for the in-file users (they require Some for completions SA:1794).
All range/predicate evaluation paths otherwise fail closed (None/false/missing -> `Err`).

---

## 2. AP/access_fields.rs (747 lines, read in full)

**Purpose.** Data types for per-field access decisions and the validated descriptors derived from them, plus `AccessPlanDiagnostic` (the crate's string error).

**Shape.** `AccessPlanDiagnostic(pub String)` AP/access_fields.rs:9; `reject_when!` macro :19-27. `ObservationModel {Stable, External, Atomic}` :40-44; `AccessExposure` :47-50; `ExternalRead {None, Read, Take}` :53-57; `AtomicPermissions` (12 bool flags) :62-75 with `any`/`contains` :77-107; `AccessPermissions {read, take, write, atomic}` :112-117; `FieldAccess {Inaccessible, Region, Stable, External, Atomic}` :126-153 with `has_destructive_read`, `operation_names` :155-180; `AccessLayoutCommitment([u8;32])` :190; `AccessFieldKey {layout_report_fingerprint, layout_commitment, slot}` :193-197; `AccessFieldEntry` :206; `AccessFields` :227-232 (with `inaccessible(layout)` :243-300, `set` :314-332); `FieldAccessDescriptor` :362-372; `LogicalFieldExtent`/`LogicalFieldFragment` :375-404; `RelativeEffectFootprint` :407-428; `EffectFootprint` :431-452; `AuthorizedFieldAccess` :493-498; `ValidatedAccessFields` :519-527 with `field`, `field_descriptor`, `field_region`, `authorize` :656-683, `authorize_region_projection` :685-746; `FieldRegion` :530-603 (`scalar_access` :573-599).

**Inputs/outputs.** Reads `layout_plans::LayoutPlanReport` (entries with `field` name and optional `member_identity`). `AccessFields::inaccessible` produces one `Inaccessible` entry per canonical field; `set` mutates one entry. `authorize*` return `AuthorizedFieldAccess` or a diagnostic.

**How it decides.**
- Field identity: `Numbered(member_identity)` when present, else `Positional(field name string)` (:251-254); two names for one identity or two identities for one name are rejected (:255-273). Slots are the ordinal in a `BTreeMap` of canonical identities (:276-293), so slot order is by identity, not layout order.
- A key is accepted by fingerprint + commitment equality, then slot index, then full key equality (:319-329, 618-628).
- `authorize_region_projection` (:685-746): projection must have width == region width, 8..=128, whole bytes, power-of-two byte length, offset inside the region and a multiple of the width from region start, and region alignment a multiple of the width (:699-711).
- `ValidatedAccessFields::field_descriptor` is a linear `find` (:636), `field_region` linear (:645).

**Cases by shape.** `FieldAccess` is a fixed 5-variant enum. Region projection: "projection exceeds its parent region, exact leaf width or alignment" (:713).

**Same job elsewhere.** Transfer-width legality (`0 < w <= 128 && w % 8 == 0`) is spelled at :701-703, AP/plan_policy.rs:75-80, 409-412, and AP/resources.rs:1374 (different forms: `is_multiple_of(8)` vs `% 8`). Fingerprint-and-commitment key check appears at :319-321 and :619-621, :631-633.

**Unused / one-caller.** `EffectFootprint::overlaps/end` used in-crate; `ExternalRead`, `ObservationModel`, `FieldAccess`, `AccessExposure`, `AtomicPermissions`, `AccessFields`, `AccessFieldKey`, `FieldRegion` are used outside the crate (see section on lib.rs). `AuthorizedFieldAccess` and `ValidatedAccessFields::authorize` have no callers I grepped outside the crate (not counted separately).

**Stated limits.** None.

**Fail-open risks.** None seen. `AccessFields::set` (:314-332) rejects foreign keys. `ValidatedAccessFields::field` returns `None` on mismatch, not accept.

---

## 3. AP/lib.rs (176 lines, read in full)

**Purpose.** Crate root: documentation, `diagnostic_accessor!` macro (:28-34), the `normalized_identity!` macro (:38-69) which defines nonzero `u64` newtypes with a compiler_storage `Record` impl rejecting zero, module declarations and the public re-export list (:78-173).

**Shape.** Module list: access_fields, placements, plan_policy, primitive_access, resources, tests (cfg(test)) (:37-76, 175-176). `pub(crate) use optimization::digest_encoder` :36.

**Stated claim (canonical/shared).** Doc comment :8-16: "we guarantee that every stable, atomic, or external access request is authorized against both its layout and its backing address-space extent before lowering". Grepped users outside the crate (`access_plans::` in omega-rust, excluding access-plans): FieldAccess, AccessExposure, AtomicAccessOperation, AtomicPermissions, BorrowPolarity, AccessFields/AccessPlan/validate_access_plan, ValidatedAccessPlan, AccessFieldKey, BaseCongruence, ObservationModel, AccessPlanId, FieldRegion, AccessResourceCompatibility, AdmittedResourceProfile, ValidatedResourceProfile, validate_resource_profile, require_stable_region_supply, validate_run_relocation_resources (3 sites), validate_access_resources, validate_initialization_resources, InitializationResourceCompatibility, AccessPlanDiagnostic; `ResourceProfileGrant::from_admitted_provider` at omega/interpreter/terminal-runtime/.../allocation/backing.rs:77.
Count-mode grep for `DeviceOperationRequirement|ProviderAssertedDeviceOperationClaim|SchemaDeviceCorrespondence|PrimitiveAccessRequest|AccessAdmission\b|RuntimeDeviceRevision|StablePrimitiveAccess|AtomicPrimitiveAccess|ExternalPrimitiveAccess|EstablishedOwned|DormantOwned|admit_owned_access|AccessFieldProjection` outside `access-plans`: 0 matches. Separately, `admit_access(|admit_owned_access(|adopt_owned_*(|bind_schema_correspondence_to_access_plan(` outside access-plans: 0 matches. `effect_footprints_conflict|structurally_close_device_operation_requirements` outside access-plans: 0 (only `validate_run_relocation_resources` has outside callers). So the placement lifecycle (admission, owned/borrowed views, schema correspondence), primitive-request specializations, and device-operation requirement closure are exported but have no consumer outside this crate's own tests (finding: documented as the gate that authorizes every access, but only the field-policy and resource-profile parts are used elsewhere). Whether `tests.rs` exercises them: not read.

**Stated limits / Fail-open.** None.

---

## 4. AP/placements.rs (3028 lines, read in full)

**Purpose.** Binds a `ValidatedAccessPlan` to a concrete address-space extent: admission (`access_admission`), the polymorphic authority reference (`access_authority`), the plan type itself (`access_plan`), and a family of linear-ownership carriers (borrowed/owned x Stable/Atomic/External, plus schema/device correspondence). Each carrier re-validates ("replays") its admission when establishing, retiring or projecting, returning the original carrier on error.

**Shape.** Macros `resident_view_accessors!` :11-83, `owned_view_accessors!` :85-150, `field_projection_accessors!` :152-169. Modules:
- `access_admission` :171-346: `AccessAdmissionId` (normalized id), `AccessAdmission<'extent>` :189-196, `admit_access` :244-262, `admit_owned_access` :264-287, `validate_access_admission` :289-305, `place` :307-345.
- `access_authority` :348-614: `AccessAuthorityRef` enum of 7 carrier kinds :365-376, with accessor/replay methods.
- `access_plan` :616-745: `AccessPlan {layout, access, reach}` :625, `ValidatedAccessPlan` :643-649, `validate_access_plan` :673-699, `AccessPlanSnapshot` :702 with `validate` :725-743.
- `atomic_resident_views` :747-984, `borrowed_view` :986-1092, `owned_access_lifecycle` :1094-1316, `owned_atomic_resident_custody` :1318-1507, `owned_external_correspondence` :1509-1742, `owned_resident_custody` :1744-1905, `resident_views` :1907-2108, `schema_correspondence` :2110-3028 (incl. 5 tests :2749-3027, read).

**Inputs/outputs.** Inputs: `ExtentLoan`/`Extent` from the `extents` crate, `ValidatedAccessPlan`, `AdmittedResourceProfile`, `ProviderExistingContentGrant`. Outputs: carrier structs; every fallible operation returns an error struct wrapping the unconsumed carrier plus `AccessPlanDiagnostic`.

**How it decides.**
- `validate_access_admission` (:289-305): restrict the profile to the loan, run `validate_access_resources`, then require `base.admits(loan.base())` (base modulo congruence).
- `place` (:307-345) re-runs the validation and requires the replayed `AccessResourceCompatibility` to be equal to the retained one.
- `validate_provider_content_binding` (:1806-1828) compares content interpretation, origin, lineage root, base+length, address space, provenance, era (via `reject_when!`).
- `validate_resident_observation` (:1830-1848): every field descriptor must have the required `ObservationModel` (Stable for stable resident, Atomic for atomic, External for external) -- a plan with any other observation cannot enter that route.
- `validate_schema_correspondence_access_binding` (:2716-2747): correspondence placement == admission plan, receipt equal, profile receipt matches the profile, replay equals retained resources.
- `ValidatedAccessPlan.identity` is `non_authoritative_access_compatibility_fingerprint` (FNV) (:682-683); `content_interpretation` is replaced by the SHA-256 authoritative digest after construction (:684-697) -- the initial placeholder `[0; 32]` is overwritten at :697.

**Cases by shape.** Fixed carrier family (7 `AccessAuthorityRef` variants :365-376); each `match` over them is exhaustive except `replay_correspondence` (:527-539: `_ => Ok(())` for non-corresponded carriers), `correspondence` (:546-549), `resident_claim` (Borrowed/Corresponded/OwnedCorrespondedExternal -> None).

**Same job elsewhere (duplicate structure).**
- The retire/replay pattern is written separately for each carrier: `borrowed_view::validate_authority` :1061-1082, `atomic_resident_views::validate_retirement_authority` :909-941, `resident_views::validate_retirement_authority` :2057-2088, `owned_resident_custody::validate_owned_admission_authority` :1867-1885. Same sequence (receipt equal; replay `validate_access_admission`; equal to retained resources; content binding; observation model).
- `borrow_view` / `borrow_view_mut` for Atomic (:767-840) and Stable (:1924-1992) are near copies (differences: Atomic keeps references to lender plan/profile/resources; Stable clones them).
- The three-field profile/plan/receipt drift check recurs in `validate_lender_binding` :943-956, `validate_schema_correspondence_access_binding` :2716-2729, `validate_external_correspondence_binding` :1628-1646.
- Revision-vs-provider/device/receipt binding check is written 3 times: :2216-2220, :2361-2365, :2410-2414.
- Same job outside: AP/primitive_access.rs `validate_authority_binding` (:456-520, :786-835) re-replays the same authority (resources, correspondence, resident content).

**Unused / one-caller.** See lib.rs section: 0 outside-crate uses of `admit_access`, `admit_owned_access`, `adopt_owned_stable|atomic|external`, `bind_schema_correspondence_to_access_plan`, `AccessAdmission`, `DormantOwned*`, `EstablishedOwned*`, `SchemaDeviceCorrespondence*`, `RuntimeDeviceRevision*` (grep, count mode, excluding the crate). `validate_access_plan`, `ValidatedAccessPlan`, `AccessPlan`, `AccessPlanId` are used outside.

**Stated limits.** `#[must_use = "..."]` strings state linear authority retained (e.g. :1111, 1128, 1142). None about unsupported.

**Fail-open risks.**
- `replay_correspondence` `_ => Ok(())` (:538): for carriers that have no correspondence (Borrowed, resident, owned stable/atomic) it accepts; intended, since those carriers have none. Status: traced (the variant set has correspondence only for CorrespondedBorrowed and OwnedCorrespondedExternal, matched at :528-537).
- `replay_resident_content` (:470): `Borrowed | CorrespondedBorrowed => return Ok(())` skips resident-content replay. Status: traced (those carriers hold no resident content grant).
- `ExternalPrimitive...` etc. none here. Otherwise none seen.

---

## 5. AP/plan_policy.rs (1189 lines, read in full)

**Purpose.** Validates a whole `AccessFields` against its layout, defines the operation enums and the per-operation authorization, boundary service reach, and the identity/commitment hashing for plans and profiles.

**Shape.** `access_fields_validation` :8-565 (`validate_access_fields` :20-200; `validate_atomic_transfer_units` :202; `validate_external_write_units` :228; `validate_destructive_access_units` :247; `logical_extent_covers_effect` :278; `validate_entry_policy` :330-419; `validate_entry_geometry` :421-544; `validate_transfer_range` :546). `access_operations` :567-684 (`BorrowPolarity`, `AccessOperation`, `AtomicAccessOperation` with `ordering_plan`). `authorization` :686-803 (`effect_footprints_conflict` :694, `authorize_descriptor` :712, `validate_operation_ordering` :779). `boundary_reach` :805-849 (`BoundaryReach(BTreeSet<BoundaryServiceReachId>)`). `normalized_identities` :851-1189 (FNV compatibility ids and SHA-256 "authoritative" digests).

**How it decides.**
- `validate_access_fields`: requires fixed layout size (:24); equality of fingerprint/commitment/`layout_plan_reports_match_for_replay` (:28-35); exactly one canonical decision per field with matching key and name (:36-49). Region entries: `permissions.any()`, no atomic, not Atomic observation, Stable with no `take`, not read+take (:63-73); length/alignment/width legality (:75-88); exactly one layout entry with an `At` placement (:89-101).
- Geometry (`validate_entry_geometry` :421-544): selected by pattern of the layout entry list for the field name (string equality on `entry.field == field`, :430): `[At]`, `[IntegerAt]` (stored width must equal transfer width), or all-`Bits` fragments in one container with `container_width == transfer width`; mixed placement rejected.
- Overlap rules: atomic fields with overlapping but unequal footprints rejected (:209-213); destructive field must cover its whole container and not overlap any other descriptor (:247-274) or any region (:140-157); destructive region must not overlap any other accessible field (:159-183); External write must cover its whole container (:234-241).
- `authorize_descriptor` (:712-777): borrow permission (Exclusive receiver needs both current and source loan Exclusive), plus per-operation permission flag, plus ordering legality (`validate_operation_ordering` :779-802).
- Identity: `non_authoritative_*` FNV functions :975-987, 1106-1161, 1175-1188; `authoritative_*` use `DomainDigestEncoder` :989-1021.

**Cases by shape.** Fixed three layout shapes ("`[At]`", "`[IntegerAt]`", "all `Bits`" :440-543). Transfer widths 8..=128 whole bytes (:409-417). `AtomicAccessOperation` (AP/plan_policy.rs:605-626) has no non-once try-exchange variant, while `AtomicPermissions` has a `try_exchange` flag (AP/access_fields.rs:73); `TryExchangeOnce` is authorized against `try_exchange_once` (AP/plan_policy.rs:765-767). In the files I read, the `try_exchange` flag is only hashed (AP/plan_policy.rs:905), listed in `any`/`contains` (AP/access_fields.rs:89, 104) and no `authorize_descriptor` arm reads it (read; no grep run on other crates).

**Same job elsewhere.** Transfer-width legality again at AP/resources.rs:1374 and AP/access_fields.rs:701-703. `validate_entry_geometry` single-container logic vs resources `select_effective_supply` -- different concerns. Hashing: `canonical_field_access_bytes` used by both FNV (:1185) and SHA (:1009) fingerprints.

**Unused / one-caller.** `effect_footprints_conflict` (:694): grepped `effect_footprints_conflict` across omega-rust excluding access-plans: 0 call sites; it is `pub` and re-exported in lib.rs:126. In-crate use: only tests (not read).

**Stated limits.** None.

**Fail-open risks.**
- `validate_entry_policy` `Inaccessible => Ok(None)` (:334): skipped by design (no descriptor). Stated by the error text "use Inaccessible".
- `FieldAccess::Region` in `validate_entry_policy` is rejected (:335-339), but handled earlier in the loop (:54-115). Fine.
- `container.expect("nonempty placements")` (:525): `placements.is_empty()` was rejected at :433; unreachable.
- None seen that accept an invalid input.

---

## 6. AP/primitive_access.rs (1251 lines, read in full)

**Purpose.** Turns an authorized field projection into a sealed `PrimitiveAccessRequest`, then into narrow per-observation requests (Stable, Stable compound, External, Atomic) and optionally "corresponded" wrappers that also carry schema/device correspondence. Every conversion re-validates and returns the input on rejection.

**Shape.** Macros `corresponded_primitive_access!` :9-124, `atomic_ordering_accessors!` :126-139, `authority_accessors!` :141-155. Modules `corresponded_atomic` :157-182, `corresponded_external` :183-209, `corresponded_stable` :210-235, `corresponded_stable_compound` :236-262, `field_projection` :263-607 (`project_access_field` :278-364, `AccessFieldProjection` :367-380, `AccessFieldAccess` :554-565), `primitive_request` :608-856 (`PrimitiveAccessRequest` :621-642; four validators `validate_effective_supply_binding` :703, `validate_descriptor_binding` :744, `validate_authority_binding` :786, `validate_authorization_binding` :837), `primitive_specialization` :857-1251 (`validate_stable_primitive_request` :1018, `validate_stable_compound_mutation_request` :1071, `validate_external_primitive_request` :1139, `validate_atomic_primitive_request` :1230).

**How it decides.** `project_access_field` (:278-364) rejects unless the authority's plan, receipt, resources, admission, base and source loan equal the copied arguments (:290-301), replays resources/correspondence/resident content, looks up the descriptor by key, requires observation match if given (:327-335), looks up the supply row by key (:336-341), checks address overflow (:342-349). Specialization validators choose by `request.observation`, `effective_supply.kind()` and `request.operation`: Stable accepts only Read/Write (:1027-1035); External accepts Read/Take/Write and allows `EffectiveSupplyKind::External`, or `Stable` supply for Read/Write only, never Atomic (:1157-1170); Atomic requires Atomic supply (:1236) and legal ordering (:1245).

**Cases by shape.** Stable ordinary: "accepts only one sealed Read or Write event" (:1032). External: "accepts only one sealed Read, Take, or Write event" (:1153). Atomic: "accepts only one sealed Atomic operation" (:1242). Compound: requires `CompoundMutation` and Exclusive current and source borrow (:1081-1092).

**Same job elsewhere.** `AccessFieldProjection::validate_authority_binding` (:456-520) and `PrimitiveAccessRequest::validate_authority_binding` (:786-835) both replay resources/correspondence/resident content and compare plan/receipt/admission/reach/loan; and `project_access_field` (:290-320) does it a third time. Same replay set also in AP/placements.rs `validate_retirement_authority` (:909, :2057).
The four corresponded wrappers (:157-262) are macro-generated from one macro (4 instantiations).

**Unused / one-caller.** No outside-crate callers (count-mode grep above returned 0 for PrimitiveAccessRequest, StablePrimitiveAccess, AtomicPrimitiveAccess, ExternalPrimitiveAccess, AccessFieldProjection).

**Stated limits.** None.

**Fail-open risks.** `validate_external_primitive_request` (:1157-1170) deliberately accepts conservative Stable supply for External Read/Write; stated in the error text ":1167 ... or conservative Stable supply for Read or Write". Status: stated. Otherwise none seen.

---

## 7. AP/resources.rs (1487 lines, read in full)

**Purpose.** Resource profiles (what each byte range of a provider-granted extent supports: Stable, External, Atomic, peer writability, reach), their validation and admission against a provider grant, compatibility of a placement plan with a profile (alignment congruence on the base address), device-operation requirement closure, and run-relocation validation.

**Shape.** `device_operation_requirements` :10-347 (`DeviceOperationRequirement`, `ProviderAssertedDeviceOperationClaim`, `structurally_close_device_operation_requirements` :256-290, `validate_device_operation_requirements` :292-346). `resource_compatibility` :349-818 (`validate_access_resources` :545-677, `select_effective_supply` :679-761, `require_base_congruence` :785-817, `require_stable_region_supply` :427-477, `validate_initialization_resources` :405). `resource_profile` :820-977 (`ResourceProfile`, `ValidatedResourceProfile::restrict` :941-975). `resource_profile_admission` :979-1226 (`ResourceProfileGrant::admit` :1082-1124, `AdmittedResourceProfile::restrict_to_loan` :1190-1224, `initialization_compatibility` :1166-1188). `resource_profile_validation` :1228-1388 (`validate_resource_profile` :1236-1302). `run_relocation` :1390-1487 (`validate_run_relocation_resources` :1396-1486).

**How it decides.**
- `select_effective_supply` (:679-761): Stable access needs `region.stable.permits(read, write)`. External access is satisfied by an `External` capability whose read behavior matches (`None` demand matches anything, `Read` needs Repeatable, `Take` needs Destructive :763-769), write allowed, and a transfer rule with the exact `width_bits` (:713-725); otherwise falls back to Stable supply for non-Take reads (:726-732). Atomic needs a transfer rule for the same width whose operations contain the demanded set (:747-757). Keyed on resource region + width (not names).
- `validate_access_resources` finds, per descriptor, the first region fully containing the transfer (:576-588); requires region reach to contain plan reach (:589-594); accumulates a base congruence (`require_base_congruence` :785-817), failing if two alignments conflict.
- `restrict_to_loan` (:1190-1224) requires address space, provenance, era, root origin, lineage root equal and rights containing the profile's required rights (:1194-1207).
- `validate_resource_profile` (:1236-1302): sorts regions, rejects empty, overflow, out-of-range, hostile-shared+stable, no-operation, overlaps; merges adjacent identical regions (:1277-1291).
- `validate_run_relocation_resources` (:1396-1486): checks only a SAMPLE of destination cells (first, second, last, and cells near each destination region boundary, :1439-1464), not every cell: `cells` is `[0]` or `[0, 1, count-1]` plus boundary neighbors, deduplicated (:1439-1466).

**Cases by shape.** Run relocation: sample cell set is a fixed pattern ("0, 1, count-1, and boundary cell +/-, +1, +2", :1439-1464). Footprint zero: `cells.truncate(1)` (:1463). External capability: must list at least one transfer rule (:1321-1323).

**Same job elsewhere.** Width validity `rule.width_bits == 0 || > 128 || % 8` :1374 vs AP/plan_policy.rs:409-412 and AP/access_fields.rs:701-703. `HashMap` not used; `BTreeMap`/`BTreeSet` used in device_operation_requirements :13, 296-309 and boundary_reach AP/plan_policy.rs:808 (note AGENTS.md says dense IDs should use flat tables; these are keyed on normalized u64 ids -- record only).

**Unused / one-caller.**
- `validate_run_relocation_resources`: 3 call sites outside the crate (terminal-interpreter storage_providers.rs:3061, :3068; selection_access_lifecycles.rs:2919).
- `structurally_close_device_operation_requirements` and `DeviceOperationRequirement`/`ProviderAssertedDeviceOperationClaim`: 0 uses outside the crate (grep above). In-crate: tests only (not read).
- `require_stable_region_supply`: outside use at storage_providers.rs:1522, selection_access_lifecycles.rs:2908.
- `ResourceProfileGrant`: 1 outside use (backing.rs:77).

**Stated limits.** None.

**Fail-open risks.**
- `validate_run_relocation_resources` samples cells (:1439-1466). A wrong program that gets through: a destination whose resource profile is non-uniform in a way not near region boundaries or the first/second/last cell (e.g. an interior cell with different alignment congruence) is not checked cell-by-cell. Status: open (no comment says why sampling is sufficient; boundary-driven selection suggests the reasoning that profile regions change only at boundaries, but not stated).
- `require_stable_region_supply` loop (:440-470): regions are assumed sorted (they come from `validate_resource_profile` which sorts); a profile built elsewhere is not re-sorted. `ValidatedResourceProfile` fields are `pub(crate)`, so traced: only constructed via `validate_resource_profile`/`restrict`.
- `select_effective_supply` `Inaccessible => unreachable!` (:688-690): traced; only descriptors (non-Inaccessible) are passed.

---

## 8. AP/tests.rs (7168 lines)

Lines 1-700 read; lines 701-7168 NOT READ (ran out of room). A grep count found 285 lines matching a `fn|mod|struct|enum` declaration or `#[test]` in the whole file (Grep, count mode), so the file holds a large test population; nothing is claimed about 701-7168.

What lines 1-700 contain: header comment (AP/tests.rs:1-5: "access plan validation, resource profile admission and compatibility, borrowed and owned resident custody, schema/device correspondence, and stable, atomic, and external primitive access specialization"); imports :7-38 (this is the only in-crate user I saw of `admit_access`, `admit_owned_access`, `adopt_owned_atomic`, `adopt_owned_stable`, `DeviceOperationRequirement`, `ProviderAssertedDeviceOperationClaim`, `SchemaDeviceCorrespondenceGrant`, `PrimitiveAccessRequest`); fixtures: UART layout with 3 fields (`status` At 0, `transmit` At 4, `control` Bits container 8, 32-bit container, width 8; size 12 align 4) :72-94, access decisions :120-153, `uart_access_fields`/`uart_access_plan` :155-169, extent/root-grant helpers :171-224, resource-profile helpers (UART, Stable word, destructive word, all-atomic) :226-477, `dormant_atomic_word` :479-499, `PrimitiveRequestSnapshot` and `primitive_request_snapshot` :501-579 (snapshots every field of a request plus the authority kind by matching all 7 `AccessAuthorityRef` variants and the authority pointer identity), `assert_atomic_specialization` :581-613, macro `expect_exact_rejections!` :615-646 (asserts a failed specialization returns the identical request, compared by snapshot), `provider_existing_content` :648-671, `established_stable_word` :673-698. No `#[test]` function appears in lines 1-700. This confirms the test file does construct the placement lifecycle (owned Stable and Atomic residents), which has no outside-crate callers.

---

## 9. AR/arena.rs (2716 lines, read in full)

**Purpose.** The dense, generation-checked arena that stores every Psi compiler table: `Handle<T>` (1-based index + generation, zero = invalid/dummy), `HandleSpan<T>` (contiguous range), `Arena<T>` (Vec storage, dummy row, optional slot metadata), `HierarchyArena<T>` (parent + one contiguous child span per parent) and `OrderedRootArena<T>` (ordered root handles over a backing arena). Includes `compiler_storage::Record` serialization with validation on read.

**Shape.** `Handle` :19-144 (`invalid`, `from_arena_index` generation 1, `from_parts`, `Record` read validates generation via `input.generation::<T>` :137-141). `unique_handles_in_order` :112-122. `HandleSpan` :146-374 (`push_contiguous` :216-241 panics on non-contiguous). `SpanHandles` :244-314. `ArenaSlotStorage` :376-381, `ArenaStorageView` :385-391. `Arena<T>` :414-424 (fields: dummy, items, slot_metadata: Option<Box>, active_count, fresh_generation, fresh, implicit_slot_metadata, revision, rewrite_revision); constructor `from_storage` :437-501; `insert` :792-825; `append` :827-839; `append_to_span` :841-874; `insert_many` :876-900; `try_insert_many_with` :902-933; `try_insert_many` :935-975; `copy_span_pair` :977-1020; `get`/`get_mut` :1022-1038; `free` :1040-1061; `handle_of` :1068-1099 (pointer arithmetic on item address); `span`/`span_mut` :1101-1137; `valid_span_range` :1139-1174; `clear` :1186-1202; `rewind_appended` :1204-1217; `reset_retain_capacity` :1219-1230; `map` :1232-1247; `iter` :1257; `for_each_mut` :1286-1307; `index_from_valid_handle` :1309-1340. `ArenaIter` :1369-1404. `Record for Arena<T>` :1434-1475. `HierarchyNode` trait :1477-1482; `HierarchyArena` :1484-1636 (read validates parent, child span, parent-chain acyclicity :1597-1635). `OrderedRootArena` :1638-1783 (read validates roots live and unique :1798-1821). Tests :1824-2716 (read, ~30 tests).

**Inputs/outputs.** Pure in-memory container. Mutation counters: `revision` bumped by every mutation-capable call, including `get_mut` and `span_mut` (:1030, :1119, `bump_rewrite_revision` :726-732); `rewrite_revision` bumped by non-append rewrites. `append_epoch()` returns `(rewrite_revision, len)` only while `fresh` (:712-715).

**How it decides.**
- Handle validity: `index_from_valid_handle` (:1310-1340): handle must have nonzero arena_index and nonzero generation; if `fresh`, generation must equal `fresh_generation` and index < len; else per-slot generation equality and occupancy. Keyed on (index, generation) pairs.
- `get` of an invalid handle returns the `dummy` row, not an error (:1022-1027) -- invalid handle silently reads as `T::default()` equivalent. `get_mut` on an invalid handle returns `&mut dummy` (:1029-1038) and mutations to it persist (test :2135 `*source.get_mut(Handle::invalid()) = ...`).
- `span` returns `None` if any slot in the range is freed or from another generation (:1157-1173); `span_or_empty` turns that into an empty slice (:1113-1115).
- `append_to_span` relocates a span to the tail when it is no longer contiguous with the end (:846-860) and panics if the span is unresolvable (:855-858).
- Storage validation on read (`from_storage` :437-501): generation nonzero, row counts, free list exactly equal to vacant slots, "fresh" claims consistent.

**Cases by shape.** `HierarchyArena::insert_children` requires parent valid and with no existing child range (panics, :1517-1524). `ArenaStorageView.fresh` mode vs slot metadata mode (`implicit_slot_metadata`), two physical layouts (:421).

**Same job elsewhere.** Free-list pop in `insert` :794-813 vs `append` :827-839 (append never reuses free slots); span roster rollback duplicated in `try_insert_many_with` :915-923 and `try_insert_many` :951-958 and `PayloadCloneGuard::drop` :665-688. `unique_handles_in_order` :112-122 (an order-preserving dedup); outside users: grep count-mode: 6 files (validation/src/traits.rs, checked-trees/facts/src/lib.rs, 06_checked-trees-to-lowered-psi/src/projections.rs, 05_typed-trees-to-checked-trees/src/flow_place.rs, plus arena lib/arena) -> 4 outside call files, one hit each.

**Unused / one-caller (grep, count mode, excluding `foundation/arena`).**
- Pattern `.append_epoch(|.rewind_appended(|.equal_by(|.copy_span_pair(|.span_mut_or_empty(|.reset_retain_capacity(|.try_insert_many_with(` outside the arena crate: 174 occurrences in 41 files (combined; not broken out per method). `HierarchyArena|OrderedRootArena|HierarchyNode`: 87 total hits in 9 files, of which 36 are inside arena.rs/lib.rs (arena.rs:34, lib.rs:2), so ~49 hits in 7 files outside (symbols table.rs 4, symbol.rs 2, symbol-resolved-trees lib.rs 24, declarations_and_machines.rs 9, storage_and_indexes.rs 1, 02_syntax pipeline selection.rs 2, symbols.rs 9) -- these counts include the pattern matching other names ending in these words; treat as upper bounds.

**Stated limits.** lib.rs (AR/lib.rs:17-26) says handle zero is the shared null entry so absence needs no `Option`.

**Fail-open risks.**
- `Arena::get`/`get_mut` with a stale/foreign/zero handle returns the dummy row instead of failing (:1023-1038). A wrong program that gets through: a stale handle read yields default content that callers may treat as "empty/absent" rather than an error. Status: stated at lib.rs:17-26 ("Handle zero is the shared null entry so absence requires no Option wrapper") for zero handle; for stale-generation handles no comment (open).
- `span_or_empty` (:1113-1115), `span_mut_or_empty` (:1130-1137) convert an unresolvable span into an empty slice. Status: open (callers not traced; ~outside count above).
- `get_mut` on the dummy: mutation through an invalid handle is accepted and changes the dummy (:1033-1037, test :2135). Status: open.
- `free` returns `false` on invalid handle and does nothing (:1040-1045). Not a risk (returns status).
- `handle_of` (:1068-1099): returns `Some` for any pointer within items for `fresh` arenas without checking occupancy (fresh means no frees). Fine.

---

## 10. AR/lib.rs (51 lines, read in full)

**Purpose.** Crate doc and re-exports. Declares `mod arena; mod owned_index; mod paged;` (:37-39) and re-exports. The doc comment names a `paged` module (`PagedArenaSet`, `ConcurrentPagedArena`, `OwnedInterner`, ...) (:7-12) which is a file not in this unit (`AR/paged.rs`, not read). Comment :28-33 records a past consolidation of seven subdirectories into `arena.rs` and `paged.rs`.

No decisions, no fail-open.

---

## 11. AR/owned_index.rs (168 lines, read in full)

**Purpose.** `OwnedIndex<T, Revision = u64>` (alias `RevisionIndex`): a lazily built derived index tagged with the revision it was built for, stored inside the owner; sealing moves it to a lock-free copy.

**Shape.** Struct :15-18 (Mutex<Option<(Revision, T)>> + sealed Option). `with` :60-76, `with_updated` :78-105, `seal` :107-116, `seal_with` :118-123, `unseal` :125-132, `with_mut` :134-145, `clear` :147-153. `PartialEq` always returns `true` (:45-49); `Debug` prints only "OwnedIndex" (:53-57); `Record` writes nothing and reads default (:156-168).

**How it decides.** Freshness keyed on equality of the `Revision` value passed by the caller vs the stored one (:66-67, :72, :91). Sealed copy is used only if its revision equals the query revision (:66-69); otherwise falls through to the mutex slot (which is empty after `seal`'s `take`), rebuilding.

**Same job elsewhere / users.** grep `RevisionIndex|OwnedIndex` count mode: 51 occurrences in 10 files (typed-trees declaration_tables_and_index.rs 21, owned_index.rs 10, typed-trees lib.rs 3, checked-trees lib.rs 3, symbol-resolved storage_and_indexes.rs 3, declarations_and_machines.rs 3, others 2 or fewer). So it has outside users.

**Fail-open risks.**
- `PartialEq for OwnedIndex` always true (:45-49), so two owners differing only in cached index compare equal. Intentional by design (derived cache); no comment at the impl; the module doc (:1-11) describes it as a derived lookup. Status: stated by role (derived data), no explicit comment on the `eq`.
- `with_updated` when the existing slot's revision is stale and `update` returns `None` keeps the old value but sets `seen = revision` (:92-97): the index is then treated as fresh although `update` declined to rebuild. Wrong case: caller's `update` returns `None` for a stale index and later queries at that revision use the stale value. Status: open (depends on callers' `update` closures, not traced).
- `Record::read` returns default (:163-167): a deserialized owner has an empty index and rebuilds on demand. Traced (by `with`'s build path).

---

## Coverage

- Read in full: SA 1-2704 (2704); AP/access_fields.rs 1-747 (747); AP/lib.rs 1-176 (176); AP/placements.rs 1-3028 (3028); AP/plan_policy.rs 1-1189 (1189); AP/primitive_access.rs 1-1251 (1251); AP/resources.rs 1-1487 (1487); AR/arena.rs 1-2716 (2716); AR/lib.rs 1-51 (51); AR/owned_index.rs 1-168 (168). Total read in full: 13,517 lines.
- Also read in full: omega-rust/psi/foundation/access-plans/src/tests.rs 1-700 (700). Total read in full: 14,217 lines.
- Skimmed: none.
- Not read: omega-rust/psi/foundation/access-plans/src/tests.rs 701-7168 (6,468 lines).
- Total: 20,685 lines (14,217 + 6,468).

## Five things a maintainer most needs to know about this unit

1. `access-plans` splits into two halves with very different usage: the field-policy and resource-profile half (`FieldAccess`, `validate_access_plan`, `validate_resource_profile`, `validate_access_resources`, `validate_run_relocation_resources`, `ResourceProfileGrant`) is used by the validator, typed-trees, terminal-psi, interpreter and selection crates; the placement-lifecycle, primitive-request, schema-correspondence and device-operation halves (`AP/placements.rs`, `AP/primitive_access.rs`, `device_operation_requirements`) had 0 matches outside the crate in count-mode greps; only `tests.rs` (not read) may exercise them.
2. Each carrier in `placements.rs`/`primitive_access.rs` re-runs the same replay (profile receipt, `validate_access_admission`, equality to retained resources, content binding, observation model) on every transition, written out per carrier rather than shared (AP/placements.rs:909, 1061, 1867, 2057; AP/primitive_access.rs:290, 456, 786).
3. In `SA`, which conditions apply is decided by proof-declaration/application ids and a `ScalarConditionScope` (Source, SourceRoots, Invocation, AbiStorage); address-model range conditions on any declaration in the module are selected for the primary target in source formation (SA:92-98), and `finish` only checks coverage for `Source` kind (SA:1910-1935).
4. `Arena::get`/`get_mut`/`span_or_empty` return a dummy row or empty slice for invalid, stale or zero handles instead of erroring (AR/arena.rs:1022-1038, 1113-1137); mutating through an invalid handle mutates the shared dummy.
5. `validate_run_relocation_resources` checks a sampled set of destination cells (first, second, last, and neighbors of region boundaries), not all of them (AP/resources.rs:1439-1466).

## What you are unsure about

- Everything in `AP/tests.rs` lines 701-7168 (6,468 lines): unread. Which of the placement/device-operation APIs the tests actually exercise is therefore unconfirmed beyond the imports at tests.rs:7-38.
- Whether `OwnedIndex::with_updated` callers ever return `None` for a stale index (AR/owned_index.rs:92-97).
- Whether `ScalarEntryMembership::admits_machine` returning `true` for empty conditions (SA:1545) is reachable with a `Selected` membership; callers in build-time-evaluation/provider-planning not read.
- The `access_plans::compute_selected_access_plan` references in build-time-evaluation/src/layouts.rs refer to a function defined at layouts.rs:1159 (a local module also named `access_plans`), not to this crate; inferred from `fn compute_selected_access_plan` grep (1 hit), module not read.
- The Atomic permission `try_exchange` (AP/access_fields.rs:73) has no matching `AtomicAccessOperation` variant (only `TryExchangeOnce`, AP/plan_policy.rs:622); I did not grep other uses of the `try_exchange` flag, so whether it is dead or consumed elsewhere is unchecked.
- Per-method outside-call counts for the arena API (`append_epoch`, `rewind_appended`, `equal_by`, etc.) are only available as one combined count.
