Second pass for unit 111

Ranges covered (gap_028): `omega-rust/psi/interpreter/terminal-interpreter/src/engine_calls.rs` lines 1-3713 (3713 lines, the whole file; the earlier reader read none of it). All lines were read in full. The file is mounted as `crate::engine::calls` (lib.rs:39 `mod engine_calls;`, lib.rs:60 `pub(crate) use crate::engine_calls as calls;`; grepped).

The earlier notes (unit_111.md) say engine_calls.rs was not read and cite one fact: `validate_input_requirements` is called at `engine_calls.rs:3383`. My read agrees (engine_calls.rs:3383). No contradiction.

The file is a set of inline modules followed by top-level items. Range-start note: line 1 is the file header comment, so the range starts at the top of the file, inside no module.

---

## Overview

**Purpose.** This file is the call machinery of the terminal-Psi interpreter (`TerminalExecution`). It (a) prepares the arguments of a call (scalar and structural), (b) pushes the caller's whole state onto `call_stack` as a `SuspendedCall` and switches the execution to the callee, (c) handles calls to "boundary" machines, whether supplied by a target-family member, a checked provider, a component provider in another program, or the host effect handler, and (d) dispatches the call operation kinds `CallUnit`, `CallStructuralScalar`, `CallStructural`, `CallStructuralWithScalarArguments`, `CallDynamic`, `Call`, `BoundaryCall`, `PortWrite`, `SelectImplementation`. The header (:1-7) says: "We prepare scalar, structural, and component arguments, reserve and capture suspended caller frames, and restore caller state upon callee completion." Restoration on completion is NOT in this file (grep: the return path lives in `engine_terminators.rs`; I read no code of it, so "inferred").

Order of items:
1. `arguments` module :9-127
2. `frames` module :128-985 (including `tests` :819-984)
3. `target_families` module :986-1190
4. `component_arguments` :1192-1597
5. `component_calls` :1598-1910
6. `component_contents` :1911-2387 (including `tests` :2164-2386)
7. `component_empty_arguments` :2388-2475
8. `component_scalar_arguments` :2476-2797
9. Top level: `PendingBoundaryCall`, `select_positions`, `impl TerminalExecution` with the `execute_*` entry points :2798-3686, and `family_actual_integer` / `family_tuple_member_value` :3688-3713.

---

## 1. `arguments` module (:9-127)

**Purpose.** Turns a call's `StructuralArgument` list into a `StructuralCallArguments` bundle that the call entry can install into the callee frame.

**Shape.**
- `StructuralCallArguments` (:21): `local_identity_cursor`, `descriptor_parameters`, `values` (`RuntimeRows<PlaceId, TerminalStructuralValue>`), `aggregate_backings`, `byte_sequences`, `primitive_loans`, `scalar_arrays` (`ScalarArrayBindings`), `element_views`.
- `validate_carrier_partition` (:35): every structural parameter must sit in exactly one of `values` (opaque) or `scalar_arrays`; the `descriptor_parameters` list must equal the opaque parameters in order.
- `resolve_call_arguments` (:62): reads scalar args (`scalar_values_at`) plus `prepare_structural_call_arguments`.
- `prepare_structural_call_arguments` (:77).

**Inputs/outputs.** Reads the callee machine (`program.verified_machine`), the structural type table, `structural_cases`, the account. Calls `prepare_scalar_array_argument` (defined `storage_records_and_arrays.rs:3516`), `structural_argument_provenance` (`engine_custody.rs:1060`), `prepare_boundary_arguments` (`boundary_effects.rs:1186`) and `into_call_arguments` (boundary_effects, per earlier notes :772). Returns the bundle; mutates nothing on `self` (takes `&self`).

**How it decides.** The split between "scalar array" and "opaque" is keyed on type shape and parameter facts: `scalar_array_leaf_shape(types, parameter.structural_type).is_some() && parameter.access == Owned && parameter.projected_qualifications.is_empty()` (:93-100). Argument count mismatch is an error (:83-88).

**Cases by shape.** Only owned, unqualified scalar-array-shaped parameters go the scalar-array route; every other parameter goes through the opaque route (:103-106). Same split is repeated in `execute_boundary_call` (:3218-3234, see below).

**Same job elsewhere.** The owned-scalar-array/opaque partition appears twice: here (:92-107) and in the checked-provider branch of `execute_boundary_call` (:3218-3231, then publishing at :3271-3286). Same file.

**Unused/one-caller.** `resolve_call_arguments`: callers :2873, :2972, :3042 (all in this file). `prepare_structural_call_arguments`: callers :73, :622, :645, :1117, :2854, :3017 (grep).

**Fail-open.** None seen; mismatches return `malformed()` or specific errors.

---

## 2. `frames` module (:128-985)

**Purpose.** Pushes the caller frame, installs the callee frame, and enforces the result contract of a structural-result call.

**Shape.**
- `is_direct_borrowed_rebinding` (:147): private predicate, one caller (:412).
- `transferred_call_sources` (:170): the set of owned, non-`Unrestricted`, whole-place (`path.is_empty()`) arguments (:178-182); errors if a source place has no value (:185-190). `retire_transferred_call_sources` (:195) removes them from a values table.
- `TerminalExecution::suspend_with` (:207): reserves one storage cell from the account (`reserve_storage(1, 0)`) and pushes a `SuspendedCall` that takes (`std::mem::take` / `.take()`) ALL of the live per-frame tables: access_view_occurrences, values, structural_values, linear_homes, aggregate_argument_backings, byte_sequence_values, primitive_loan_values, scalar_array_values, element_view_values, live_affine_frontier, live_claims, plus current_machine/current/next_operation and the `result` kind (:224-242). If the reservation fails it stores `result` in `self.failed_call_result` and returns `Storage` error (:219-222).
- `consume_affine_call_argument` (:246): requires `stage_constructor_operand` to return true, else `AffineFrontierMismatch`.
- `bind_byte_sequence_arguments` (:257) and `bind_element_view_arguments` (:346): per-parameter binding of byte-sequence and element-view carriers.
- `enter_call` (:440): the core. Steps (all read): bind scalar parameters (`bind_arguments` :450); `validate_carrier_partition` (:455); `bind_call_access_views` (:458); if result is `Structural`, capture `expected_reference_backings` via `bind_reference_return_backings` (:463-482); `stage_owned_value_arguments` (:483); bind the callee's affine frontier (:491); `transfer_claims` (:497); `exclude_linear_regions` (:509); consume affine args from a clone of the caller frontier (:525-535); compute the loan completions for the call operation unless the result is `ExternalCompletion`/`NoNormal` (:536-555); `suspend_with` (:556); then overwrite self's tables with the callee's (:565-580).
- Entry wrappers: `begin_unit_call` (:584), `begin_external_completion_call` (:608), `begin_no_normal_call` (:634), `begin_structural_scalar_call` (:660), `begin_structural_result_call` (:685). Each picks a `SuspendedCallResult` variant (`Unit`, `ExternalCompletion{boundary,edge}`, `NoNormal{callee,edge}`, `Scalar(id)`, `Structural{...}`) and calls `enter_call`.

**Inputs/outputs.** Reads the program (`verified_machine`), catalogs, structural types, live claims/frontier. Mutates the execution: replaces every per-frame table, pushes the call stack, and changes `current_machine`/`current`/`next_operation`.

**How it decides.**
- `begin_unit_call` and `begin_external_completion_call` require `callee.result == Unit` (:594, :618). `begin_no_normal_call` does no result check (:634-658).
- `begin_structural_scalar_call` compares only the scalar TYPE of callee result and expected result (`result.scalar_type`, :671).
- `begin_structural_result_call` keys its judgment on type id, multiplicity, qualification domain lists, and place presence (:776-796): result carrier type equal to callee declaration; multiplicity equal; `claim_origin.result_qualifications_match`; `result_projected_qualifications_match`; destination place not already in `structural_values` or `scalar_array_values`. A `TargetFamilyReturn` origin must name `member == callee_id` and `program.is_target_family_head(head)` (:710-717).
- Projected qualifications on a `CalleeReturn` result that the callee does not declare must be "preserved": the code finds the single defining call operation in the caller (:726-743), checks it is the same call (callee and structural args equal, :745-760), then asks `ReferenceResultMemberships::new(&program.catalogs).for_operation(caller, operation)` and rejects rows not in that set (:761-774).
- `bind_byte_sequence_arguments` skips non-`ByteSequence` shapes (`continue`, :271-273) and skips "borrowed storage parameter with stored reference access and no reference ceiling" (:274-285). Then tries, in order: owned byte descriptor (:286), byte descriptor (:294), and a bounded-owned owned/owned unrestricted argument snapshot (:302-340); anything else is `malformed()` (:341).
- `bind_element_view_arguments` checks stored vs resolved identity (`opaque_identity`, `path`, `borrowed_referent_matches`, :379-388), then picks the access by parameter: Owned (requires `argument.access == Owned`, same type, `view.owned_multiplicity() == Some(parameter.multiplicity)`, :390-397), borrowed-storage parameter (:398-410), or direct borrowed rebinding (:412-430) else a `StructuralArgumentPreparation` error whose detail uses `format!("formal={parameter:?}, source={resolved:?}...")` (:420-427). That `format!` of debug values is only an error message, not a decision.

**Cases by shape.**
- `is_direct_borrowed_rebinding` (:147-168): accepted only if the parameter is a view descriptor parameter, same type, same access as the argument, access is one of the reading-borrow pair (`reading_borrow_pair_pattern!()` :161), no parameter qualifications, every projected qualification on the parameter is an element-membership row also on the resolved value, and the resolved value has no qualifications.
- `suspend_with` always frees nothing; `transferred_call_sources` handles only `argument.path.is_empty()` (whole-place) transfers (:181).
- `enter_call` skips loan-completion retention for `ExternalCompletion` and `NoNormal` calls (:536-541).

**Same job elsewhere.**
- `transferred_call_sources` is reused by `component_arguments::prepare_component_arguments` (:1552) in this file. The same "owned, non-Unrestricted, whole-place" filter is written a third time inline in `complete_boundary_effect` (:3579-3586). Same job, three sites.
- Claim transfer construction from completion receipts is written out twice: :1150-1157 (target family) and :3287-3294 (checked provider). Both map `receipt.claim, argument_index, path.clone()` into `ClaimTransfer`.
- The three-arm match on `operation.result` (Unit / Scalar / Structural) that picks `begin_unit_call`, `begin_structural_scalar_call`, `begin_structural_result_call` appears at :1158-1186, :2974-3000, :3295-3342 (grep and read).

**Unused/one-caller (grepped).**
- `begin_unit_call`: 4 callers outside its definition in the file (:1159, :2855, :2975, :3296). `begin_structural_scalar_call`: :1166, :2875, :2982, :3333. `begin_structural_result_call`: :1174, :2991, :3018, :3044, :3310. All callers are in this file.
- `begin_external_completion_call`: 1 caller, `engine_terminators.rs:2548`. `begin_no_normal_call`: 1 caller, `engine_terminators.rs:2162`.
- `bind_byte_sequence_arguments`: callers `engine_terminators.rs:708`, `:729`; `boundary_effects.rs:1264`. `bind_element_view_arguments`: `engine_terminators.rs:864`, `boundary_effects.rs:1273`. (Grep command: `bind_byte_sequence_arguments|bind_element_view_arguments` across omega-rust.)
- `consume_affine_call_argument`: callers in this file :533, :1549 and in `storage_primitives.rs:453, :561, :659, :660`.
- `is_direct_borrowed_rebinding`: 1 caller (:412).
- `suspend_with`: callers in this file :556, :3672 (grep not run for other files; test at :937). Not counted outside the file.
- `empty_program` (tests, :838): used from `boundary_effects.rs:2786/2845`, `boundary_values.rs:224/243`, `engine_terminators.rs:1468/1500`. It is a test helper shared by four files.

**Stated limits.** None as comments in this module. Error strings: "result destination already has a structural value" (:791), "projected result qualification has no preserved input source" (:769).

**Fail-open risks.**
- `bind_byte_sequence_arguments` `continue`s past borrowed-storage parameters with a stored-reference shape and no ceiling (:274-285) and past all non-`ByteSequence` shapes (:271-273). The skip leaves the parameter without a byte-sequence binding. Whether another carrier then binds it is "open" (I did not trace `borrowed_storage_reference_ceiling` or the callers in engine_terminators/boundary_effects).
- `bind_element_view_arguments` likewise `continue`s for non-`ElementView` shapes (:363-365) and for borrowed-storage parameters with no ceiling (:366-376). "open", same reason.
- `begin_no_normal_call` (:634-658) has no callee result check at all. Who gets through: a callee of any result kind, because the call is entered with `SuspendedCallResult::NoNormal`. Caller site `engine_terminators.rs:2162` not read; "open".
- `begin_structural_scalar_call` compares scalar types only (:671); it does not compare the result's value id or range qualifications. "open".
- `enter_call` returns `Ok(())` after overwriting self; there is a window where `suspend_with` has succeeded but later steps (`aggregate_payload.publish`, :573) can fail, leaving the execution in a half-switched state. This is an error path, not an acceptance path; "traced" as such, I did not trace what the error handler does with the half-switched state.

---

## 3. `target_families` module (:986-1190)

**Purpose.** Resolves a call to a "target-family head" (a boundary whose implementation is chosen per build target) to its selected member, and executes the call as an ordinary machine call.

**Shape.**
- `selected_member` (:1004): needs `AdmittedTargetMachineSelections`; errors if absent (`TargetMachineSelectionMissing`, :1010-1011) or if `selections.terminal_psi() != program.identity.semantic()` (:1012). Then matches `(callable, supply)`: `Machine(member)` with `CheckedBody | Boundary` supply is accepted after `verified_machine(member)`; `Boundary(member)` with any supply other than `CheckedBody` is accepted after `verified_boundary_machine`; everything else is `UnsupportedCapability(TargetMachineMemberSupply{..})` (:1018-1039).
- `completion_callee` (:1042): returns `Some(member)` for a target-family head that selects a `Machine`; else a `CheckedProvider(callee)` supply gives `Some(callee)`; no supply but the program has a provider is `ProviderInstallationMissing`; otherwise `Ok(None)` (:1048-1062).
- `validate_result_transport` (:1065): checks the call operation's result kind and type/multiplicity/qualifications against the member's declared result.
- `execute_target_family_call` (:1103).

**How it decides.** By identity ids: `BoundaryMachineId`, `MachineId`, `StructuralTypeId`, `ScalarType`, and domain id lists. Scalar results compare only `scalar_type` (:1078-1081). Structural results compare type id, multiplicity, qualification lists via `ResultClaimOrigin::TargetFamilyReturn{head, member}` and that every `qualification_establishments` row's domain is in the member's declared qualifications (:1092-1095).

`execute_target_family_call` (:1103-1188): head and member must have the same count of structural parameters (:1118); for each pair, if the member parameter's value is in `prepared.values` it is collected under the HEAD parameter declaration, else the parameter must be a scalar array of the same structural type (:1128-1135). Then `bind_active_structural_arguments` and `validate_boundary_requirements` against the head (:1137-1149), then `begin_*_call`.

**Cases by shape.** Three-way match on `operation.result` (Unit/Scalar/Structural) (:1158-1186). The `selected_member` match arms are the fixed list of accepted (callable, supply) pairs (:1018-1039); the rest are refused.

**Same job elsewhere.** `terminal-installation` has its own `selected_member` (`provider_and_target_installation.rs:1053`) and `borrowed_selected_member`; this file's `selected_member` is a different function with the same name that checks the admitted selection at run time. Suspected duplicate of the install-time selection check; I did not read the installation one beyond the grep line.

**Unused/one-caller (grepped).**
- `selected_member` (this module): callers `engine_execution.rs:458`, `embedding.rs:2517`, and in-file :1050, :3182.
- `completion_callee`: callers `engine_terminators.rs:2539`, `engine_execution.rs:552`.
- `validate_result_transport`: callers `engine_execution.rs:470` and in-file :1115.
- `execute_target_family_call`: 1 caller (:3188).

**Fail-open risks.**
- `validate_result_transport` in the `Scalar` arm compares scalar type only (:1078-1081); a same-typed scalar with a different range contract on the member would pass. "open".
- `completion_callee` returns `Ok(None)` when there is no target-family selection and no supply and no provider (:1061). That means the caller treats the boundary as having no machine callee. Who the caller is: `engine_terminators.rs:2539`, `engine_execution.rs:552`, not read. "open".

---

## 4. `component_arguments` module (:1192-1597)

**Purpose.** For a call into a component provider (a separate admitted Terminal Psi program), prepares the arguments: validates the receiver/provider/entry contracts, maps qualifications into the provider's theory, captures contents of owned values, and arranges lending of borrowed values.

**Shape.**
- `PreparedComponentArguments` (:1209): `custody`, `remaining_affine`, `remaining_claims`, `retired_sources`, `borrowed_mutations`.
- `ComponentInputCustody` (:1217): `source` (`Option<BoundaryArguments>`), `values`, `contents`, private `opaque_arguments`, `scalar_arguments: Vec<LentScalarArgument>`, `empty_arguments: Vec<SharedEmptyArgument>`, `_scalar_scope`. `establish_lent_inputs` (:1228) installs the lent inputs into the child execution: for each opaque borrowed argument, records `component_lent_host_atoms` and `structural_host_atoms` (`HostStructuralPayload::Opaque(opaque_identity)`) (:1239-1259), then runs `establish` on empty and scalar arguments.
- `ComponentCallScope` (:1271): `namespace`, `machine`, `block`, `depth` (call-stack length). Built by `component_call_scope` (:1279).
- `prepare_component_arguments` (:1288-1595).

**How it decides (`prepare_component_arguments`).**
- Arity: receiver, provider-supplied and entry structural parameter counts must equal the argument count; entry must have no `content_entry_claims`; `entry.entry_claims.len()` must equal the number of Linear provider parameters (:1301-1315).
- Per argument, a combined reject (:1362-1396): access of argument, receiver and provider equal; must be Owned or a reading borrow; multiplicity equal; no projected qualifications on required, supplied or the value; `owns_linear_claim == linear` for both types; non-linear types need `no_code_affine_cleanup`; and the type identity TEXT must match and be nonempty: `identities.text(receiver_type.identity)` zip `identities.text(provider_type.identity)` with `left == right && !left.is_empty()` (:1374-1386). So component types are matched by identity string equality. Owned with a live loan (`has_live_loan`) is also refused.
- Entry formal must equal the supplied parameter in type, multiplicity and access (:1400-1408).
- Claims: for a linear input, exactly one whole (empty-path) entry claim and exactly one receipt, receipt's claim live, `live.place == Some(argument.place)`, same path, multiplicity Linear (:1417-1440); non-linear must have none (:1441-1444). Overlap with any other live claim on the same place is refused (:1448-1459).
- Storage carriers: "within value" tests (`opaque_identity` equal and path prefix, :1461-1464) are run against many tables. A lent value inside `structural_cases`, `structural_byte_storage`, or `empty_array_storage` is refused (:1465-1473). Any value inside `reference_referents`, `structural_element_view_fields`, `original_storage_roots`, `provider_storage.descriptors`, `.completed_profiles`, `activation_storage.claim_descriptors`, `structural_byte_descriptors`, `structural_byte_sequence_fields` (parent) is refused (:1474-1497).
- Routing of each argument (:1498-1533): if lent and in scalar primitive storage or scalar fields, a scalar lending (`prepare_component_scalar_argument`); else if not `forward_host_atom(value)`: a shared-borrow empty record (`prepare_shared_empty_argument`), else if not owned refuse, else capture contents (`capture_component_contents`); else if lent, record as an opaque argument.
- Qualifications are rewritten with `selected.input_membership(argument_index, domain)`; a missing mapping is refused (:1534-1546). `value.structural_type` is rewritten to the provider's type (:1547).

**Cases by shape.** The comment/message text lists the supported shape: "component input lacks exact owned closed Inert type/custody" (:1394); "component borrowed input needs its original materialized storage carrier" (:1471). The supported inputs are: Owned or reading-borrowed, no projected qualifications, identical type identity text on both sides, no outstanding loans/resources/descriptors/element views/original-storage roots, linear types with exactly one whole claim, or non-linear types with no-code affine cleanup.

**Same job elsewhere.** The "identity text equal and nonempty" test is written twice in this file for inputs (:1374-1386) and results (:1775-1787). The `component_contents::ComponentContentsMapping::map_identity` (:2149-2161) does the same lookup by index. Three sites.

**Unused/one-caller (grepped).** `prepare_component_arguments`: 1 caller (:1659). `ComponentInputCustody` / `ComponentCallScope`: used by `embedding.rs:268-293, :472-476, :533`. `component_storage_path`: callers :2611 (and its own definition :1925).

**Stated limits.** The reject strings above are the limits. None are phrased as comments.

**Fail-open risks.**
- Line :1441-1444 and :1446 treat a non-linear input with no claims as fine; correct if the type is non-linear. Reached with a non-linear type only. "traced" fail-closed because the `owns_linear_claim == linear` check at :1369-1370 comes before.
- `prepared.opaque_values(self.local_structural_identities.cursor())?` (:1326) is called and its result discarded; its effect (error or none) is not visible in this file. "open" (callee in boundary_effects, not read).
- When `lending` is false, `prepared.values` is moved out with `mem::take` (:1334) and `prepared` is not kept (`source: lending.then_some(prepared)`, :1582). Fine; no acceptance path.

---

## 5. `component_calls` module (:1598-1910)

**Purpose.** Executes a call to a component provider: validates the binding against the admitted installation, prepares arguments once, invokes the provider through `ComponentBindings`, and on completion maps the result back into the receiving program.

**Shape.** `execute_component_call` (:1614-1908) is the only function.

**Inputs/outputs.** Reads `ComponentBindings` (`embedding::instance`), `boundary_supplies`, program identity, the work `meter`. Mutates `live_affine_frontier`, `live_claims`, stored provenance, removes retired source places, pushes `TerminalEffect::ComponentCall`, calls `commit_boundary_result`.

**How it decides.**
- Binding must exist for `boundary` (:1636). The selected installation must match: `selected.receiver() == &self.program.identity`, `receiving_boundary == *boundary`, and `self.boundary_supplies.get(boundary) == Some(&expected_supply)` (:1643-1655).
- Preparation happens once, keyed on `binding.pending_at(operation.id)` (:1656): if not pending, prepare arguments and `binding.prepare(...)`.
- Invocation result: `SponsorExhausted` or `HostComputationPending` yield; `AwaitingEffect` yields only if `operation.suspension_crossing.is_some()` (:1697-1705); any other status is an error (:1706-1710); `None` means completed.
- On completion, provider, candidate and instance namespace must match, and the instance namespace must differ from the caller's `exposure_namespace` (:1717-1726).
- Result is matched as a triple `(receiver result, provider result, completed value)`: Unit/Unit/Unit; Scalar/Scalar/Scalar requiring `left == right` and the value's scalar type equals `left.scalar_type` (:1744-1750); Structural/Structural/Structural (:1751-1886); anything else is an error (:1887-1892).
- Structural result checks: provider type id equal to the returned value's type; multiplicity equal; projected qualifications empty; `owns_linear_claim` agreement; non-linear needs `no_code_affine_cleanup`; identity text equal (:1765-1793). Qualifications mapped by `selected.result_membership` (:1796-1804), then closure equivalence in both directions via `entails` (:1807-1828) using `missing_membership_requirements` against `structural_domains` (the domain's `carrier` must equal `left.structural_type` :1814). The receiving place must be fresh (:1834-1847); claims must be exactly one whole claim for linear types, none for non-linear (:1848-1867).

**Cases by shape.** Result kinds Unit, Scalar, Structural only, plus matching by kind triple (:1738-1892). `Structural` result requires `operation.result` to be `Structural` ("component structural result has no receiving home", :1877).

**Fail-open risks.**
- `entails` uses `.is_some_and(...)` over a `find` of the domain: a required domain id not found in the catalog gives `false` (reject), not accept (:1809-1819). Fail-closed.
- `Some(completed.instance_namespace()) == self.local_structural_identities.exposure_namespace` rejects when equal; if the caller's namespace is `None` the test is false and passes. Who gets through: a completion whose namespace exists while the caller has no exposure namespace. "open".
- The `.expect("owned source was staged without parent mutation")` at :1684 would panic rather than error if the retired source were absent. It is a panic path, not an accept path.

---

## 6. `component_contents` module (:1911-2387)

**Purpose.** Captures the stored contents (scalar leaves and chosen case) of an owned argument value and re-homes them from the receiver's type ids to the provider's type ids, matching types, cases and fields by identity text.

**Shape.**
- `component_storage_path` (:1925), `component_contents_mapping` (:1937), `capture_component_contents` (:1949).
- `ComponentContentsMapping` (:1993): source/destination type tables and identity tables. `rehome` (:2001), `rehome_path` (:2030), `map_identity` (:2149).
- Tests :2164-2386: `Fixture`, two tests (`component_contents_rehome_nominal_cases_payloads_and_array_cells` :2293, `component_contents_refuse_reference_crossings_bad_indexes_and_nominal_substitutions` :2333).

**How it decides.**
- `capture_component_contents` refuses a description with `requires_erased_establishment` or any `Opaque` or `ByteField` leaf: "component contents cannot manufacture opaque backing or erased custody" (:1958-1969).
- `map_identity` maps a source identity index to a destination index by identity TEXT: `source_identities.text(identity)` must be nonempty, then `destination_identities.index_of(text)` (:2153-2160). This is the key: source spelling of the nominal identity.
- `rehome_path` walks the path; at each step checks `map_identity(source.identity) == destination.identity` (:2043). Cases are matched by identity (`structural_case_by_identity` :2057), fields by identity (`live_field_by_identity` :2079). An erased source field is refused (:2084-2086). Scalar-typed fields may only end the path (`(None, None) if position + 1 == source_path.len()`, :2096). Fixed arrays need equal literal lengths and `element_index < length` (:2117-2123). `Referent` is refused: "component contents cannot cross a reference" (:2128-2131).

**Cases by shape.** Path segment kinds Case, Field, FixedIndex (arrays with literal length), Referent (refused) (:2046-2133). Case contents: Scalar and Case only (:2009-2025).

**Same job elsewhere.** `map_identity` by-text matching is the same mechanism as the identity-text equality tests in `prepare_component_arguments` (:1374-1386) and the result check (:1775-1787).

**Unused/one-caller.** `component_storage_path`: callers :2611 (this file) only (grep of `component_storage_path` across `omega-rust/psi/interpreter`). `capture_component_contents`: 1 caller (:1523).

**Fail-open.** None seen. Every unmatched case returns `malformed()` (:2014, 2017, 2023, 2044, 2055, 2061, 2070, 2074, 2078, 2083, 2097, 2115, 2122, 2136, 2144).

---

## 7. `component_empty_arguments` module (:2388-2475)

**Purpose.** Lets a shared borrow of an empty record (no fields, no storage leaves) be passed to a component provider by carrying its storage address, so the child sees the same original address.

**Shape.** `SharedEmptyArgument{index, original: ScalarStorageAddress}` (:2396); `empty_record` (:2401); `prepare_shared_empty_argument` (:2417); `SharedEmptyArgument::establish` (:2445).

**How it decides.**
- `empty_record`: shape is `Record` with `fields.is_empty()` and `contents::describe(...)` yields no leaves, no atomic storage, no erased establishment (:2405-2413).
- `prepare_shared_empty_argument` returns `None` unless the empty record is either "formed" (allocated by this execution's `local_structural_identities` and inside some owner value, :2428-2433) or already in `component_borrowed_addresses` (:2434).
- `establish` requires parameter access `SharedBorrow` and an empty record, and an existing address entry must equal the original (:2454-2468).

**Cases by shape.** Only empty `Record` types (:2405).

**Fail-open.** `prepare_shared_empty_argument` returns `Ok(None)` for unrecognised values; its caller (:1513-1530) then falls to "not owned, refuse" or capture contents, so the `None` leads to a reject for borrowed values. "traced" fail-closed.

---

## 8. `component_scalar_arguments` module (:2476-2797)

**Purpose.** Lends the scalar storage cells of a borrowed (shared or mutable) record to a component provider, then re-binds the same cells in the child execution under the child's own place keys.

**Shape.** `PreparedScalarArgument` (:2489), `PreparedScalarCell` (:2496), `LentScalarArgument` (:2506). Helpers `scalar_key` (:2518), `parent` (:2532), `cell` (:2539), `cell_mut` (:2549), `insert_address` (:2784). `prepare_component_scalar_argument` (:2562), `PreparedScalarArgument::lend` (:2639), `LentScalarArgument::establish` (:2673).

**How it decides.**
- `prepare_component_scalar_argument` refuses descriptions that need erased establishment or atomic storage or have no leaves (:2571-2576). Each leaf must be `StorageLeafKind::Scalar` (:2580-2588); stored scalar type equal to the cell's (:2591); the cell's parent must lie under the argument's root by `opaque_identity` and path prefix (:2595-2598).
- `lend` (:2639): a source cell already lent in this call (`views`) may be re-lent only if BOTH earlier and current access are `SharedBorrow`; otherwise "component inputs overlap an exclusive scalar home" (:2647-2655).
- `establish` (:2673): the child parameter access must equal the lent access (:2682); leaf count must equal cell count (:2690); each child leaf must be a Scalar leaf with the same `scalar`, `stored_scalar`, `restriction` as the prepared cell (:2703-2716). If the child already has a cell at the key, it must be the same cell and access must be Shared (:2719-2723). Address maps are built and compared (:2735-2760) before binding cells (:2761-2774) and recording borrowed addresses (:2775-2779).

**Cases by shape.** `scalar_key` accepts only `PrimitiveStorageEndpoint::Scalar` and `ScalarField`; every other endpoint kind gives the "needs its original materialized storage carrier" rejection (:2521-2529).

**Fail-open.** None seen; all mismatches give `malformed()` or `reject()`. One quiet point: in `establish`, `if !child.component_borrowed_addresses.contains_key(&key) { insert }` (:2775-2779) silently skips existing keys; existing mismatches were already rejected at :2754-2760 for non-Shared or non-equal addresses. "traced".

---

## 9. Top-level items (:2798-3713)

**Purpose.** The operation-level entry points for calls, called from the main operation dispatch.

**Shape.**
- `PendingBoundaryCall` (:2819): `site` (`TerminalEffectWait`), `awaiting_provider_readiness`, `arguments` (`BoundaryArguments`), `remaining_claims`, `effect`. Held in `TerminalExecution.awaiting_effect: Option<Box<PendingBoundaryCall>>` (`engine_execution.rs:1841`).
- `select_positions` (:2827): clone selected positions from a slice, `malformed()` if out of range.
- `execute_call_unit` (:2838), `execute_call_structural_scalar` (:2865), `execute_select_implementation` (:2886), `execute_call_dynamic` (:2909), `execute_call_structural` (:3004), `execute_call_structural_with_scalar_arguments` (:3030), `execute_boundary_call` (:3056), `boundary_effect_wait` (:3459), `complete_boundary_effect` (:3504), `execute_port_write` (:3627), `execute_call` (:3652).
- `family_actual_integer` (:3689), `family_tuple_member_value` (:3708).

**Dispatch (grepped).** `engine_execution.rs:2721` (`execute_boundary_call`), :2724 (`execute_port_write`), :2793 (`execute_call`), :2796 (`execute_select_implementation`), :2799 (`execute_call_dynamic`), and via a macro table at :2837-2840 (`CallStructuralScalar`, `CallStructural`, `CallStructuralWithScalarArguments`, `CallUnit`). Each has exactly one dispatch site. `execute_boundary_call` and others take the operation kinds named in their `dispatched_operation!`/`operation_kind!` macros.

**How it decides.**

`execute_select_implementation` (:2886): writes the integer `table` id into the result value as an unsigned integer of the result's integer scalar type; refuses non-integer result type and unknown table (:2893-2898). It does not check that the value fits the integer type (`IntegerValue::Unsigned(u128::from(table))`, :2903). "open": whether the type's range can hold `table` is not checked here.

`execute_call_dynamic` (:2909): selector is read from `self.values`, must be an unsigned `Integer` (:2918-2924); table must exist and its `interface` must equal the operation's `interface` (:2927). The slot is the operation's slot unless `family_actuals` is nonempty, in which case the code scans the interface slot list from `slot` onward: stops at a slot whose `family_tuple` is empty (:2950) or whose `requirement_identity` differs from the base slot's (:2953); a candidate matches if the tuple length equals the actuals and every member value equals the actual (:2956-2964). If none matches, `malformed()` (:2969). Then `catalog.row(table, slot)` yields the callee (:2971). The result kind selects the `begin_*` (:2974-3000), using `CalleeReturn` for structural results.

`family_tuple_member_value` (:3708-3713): the identity STRING of a family tuple member is parsed: `strip_prefix("named(integer-const(")`, `strip_suffix("))")`, `parse::<i128>()`. This is a decision keyed on source spelling of an identity string. Same string format is parsed in three other places (grep `integer-const\(`): `typed-trees/src/calls_and_control_flow.rs:978`, `06_checked-trees-to-lowered-psi/src/unit_arguments.rs:6641`, `04_typed-trees-to-typed-trees/src/monomorphization.rs:2611`; the producer is `typed-trees/src/lib.rs:1779` and `calls_and_control_flow.rs:990`. So four parse sites and two format sites for one string encoding (grepped; I read only this file's site). A member whose string does not parse gives `None`, which never equals `Some(actual)`, so that candidate is skipped (fail closed here).

`family_actual_integer` (:3689): value must be an `Integer`, signed or unsigned (unsigned converted to `i128`, error on overflow); anything else is `malformed()`.

`execute_boundary_call` (:3056-3457) is a long ordered chain. In order:
1. If `awaiting_effect` is set, poll readiness and either yield or complete (:3063-3074).
2. Fetch the boundary declaration. `EmptyRunRelease` execution calls `release_empty_run` and advances (:3081-3089). A declaration whose `execution.static_interrupt_operation()` is `Some` is refused with `malformed()` (:3090-3096).
3. If `execution.placement_operation()` is `Some`: find the access lifecycle plan for the boundary (:3102-3106), then match the lifecycle operation `View`, `Initialize`, `MoveOut`, `Relocate` (:3107-3174). `Initialize` requires exactly three structural arguments `[storage, value, profile]` (:3113-3117), builds a value image, prepares the initial image, and for `TerminalAccessOwner::Run` forces `IncompatibleSupply` when `plan.placement.layout().size != Some(binding.length)` (:3142-3149).
4. Bind scalar parameters (`bind_boundary_arguments`, :3180).
5. Target-family head: `selected_member`; `Machine(member)` goes to `execute_target_family_call`; `Boundary(_)` falls through (:3181-3197).
6. Look up supply in `boundary_supplies`: `DeferredComponent` is refused; `ComponentProvider` goes to `execute_component_call` (:3198-3212).
7. `CheckedProvider` or (no supply and the program has a provider): partition owned scalar arrays vs opaque parameters (:3218-3234), prepare boundary args, bind active structural args, validate requirements (:3250-3255), require supply to be `CheckedProvider(callee_id)` else `ProviderInstallationMissing` (:3256-3262), convert to call arguments with the CALLEE's parameter declarations (:3264-3270), publish scalar arrays (:3271-3286), then begin the call by result kind (:3295-3342). For a structural result, `capture_resource_profile_call` runs first and its completion is stored into the pushed frame (:3304-3330).
8. `execute_access_boundary` handles access boundaries; returns true then advances (:3345-3352).
9. A boundary that `requires_checked_non_observation` is refused with `RuntimeSupplySelectionRejected` (:3353-3360).
10. Otherwise a host effect: ask the handler for `storage_requirements` (:3361), resolve boundary arguments (:3363), bind, `validate_boundary_requirements` (:3380), `validate_input_requirements` (:3383), `preflight_boundary_result` (:3390), refuse reference results if the handler does not `accepts_reference_results` (:3391-3398), complete claims, capture reference inputs, prepare original storage, export host atoms for owned arguments (:3416-3425), build `TerminalEffect::BoundaryCall` and a `PendingBoundaryCall`, then either wait or complete (:3448-3456).

`boundary_effect_wait` (:3459): the pending site (program, machine, block, operation, boundary) must equal the current position, else `malformed()` (:3466-3473). Not ready and no `suspension_crossing` is an error (:3480-3487); not ready yields `AwaitingEffect`; ready but not computed yields `HostComputationPending` (:3491-3501).

`complete_boundary_effect` (:3504): hands the effect and buffers to the handler (`handle_effect_with_buffers`); a `Crash` result is admitted via `admit_boundary_crash` and yields `Crashed` (:3529-3536); a `Reference` result requires a region with `Invocation` origin among `result_reference_regions` (:3537-3551); validates write-back, prepares a captured reference result, reserves the host result identity, commits the result, retires owned non-Unrestricted whole-place argument sources (and removes affine roots), installs remaining claims, binds result claims, commits captured storage, completes loan custody and pushes the effect (:3552-3624).

`execute_port_write` (:3627): requires `Unit` result; builds a `PortWrite` effect and calls `handler.handle_effect`, pushes the effect. No other check in this file.

`execute_call` (:3652): the scalar-only `Call`. Refuses (as `malformed()`) a callee that has structural parameters, entry claims, content entry claims, or a non-scalar result (:3664-3669); binds the scalar parameters, suspends with `SuspendedCallResult::Scalar` (no loan completions), and resets structural_values, frontier and claims to empty (:3671-3683). This duplicates part of `enter_call` (:440-582) for the scalar-only case; it does not touch `access_view_occurrences`, `scalar_array_values`, `element_view_values` and other frame tables, whereas `suspend_with` does `take()` all of them. So after `execute_call` the callee frame keeps whatever those tables held in the caller? `suspend_with` takes them (`.take()`, :225-234), leaving the self tables empty, so the callee starts with empty tables. "read"; the effect is the same as `enter_call` for those tables.

**Cases by shape.**
- Dynamic dispatch: slot search over a family tuple; "interface slot list, scan from slot until tuple empty or requirement identity changes" (:2945-2968).
- Boundary placement lifecycle: `View`, `Initialize`, `MoveOut`, `Relocate` (:3107-3174). `Initialize` accepts exactly three structural args (:3113).
- The checked-provider branch distinguishes unit, structural, scalar results only (:3295-3342).

**Same job elsewhere (inside the unit).**
- The Unit/Scalar/Structural result switch at :1158, :2974, :3295 (above).
- `execute_call` (:3652) vs `enter_call` (:440): both bind callee scalar parameters and `suspend_with` the caller; `execute_call` is the structural-free special case.
- `execute_call_structural` (:3004) and `execute_call_structural_with_scalar_arguments` (:3030) are the same function except that one reads scalar arguments and one passes `&[]` (:3021). `execute_call_unit` (:2838) re-implements the scalar-read loop inline (:2849-2852) instead of using `resolve_call_arguments` (:62).
- In the host-effect path, "owned, whole-place, non-Unrestricted" retirement (:3579-3606) duplicates `transferred_call_sources`/`retire_transferred_call_sources` (:170-204), except this one also removes affine roots and records `retired_structural_homes`.

**Unused/one-caller (grepped).**
- `execute_call_unit`, `execute_call_structural`, `execute_call_structural_scalar`, `execute_call_structural_with_scalar_arguments`: each has one use outside its definition, the macro dispatch table `engine_execution.rs:2837-2840`. `execute_call_dynamic`, `execute_call`, `execute_select_implementation`, `execute_boundary_call`, `execute_port_write`: one call site each in `engine_execution.rs` (:2799, :2793, :2796, :2721, :2724). Grep: `\.execute_(call|...)\(` and `execute_call_...` names over `omega-rust`.
- `PendingBoundaryCall`: used in `engine_execution.rs:1841` (field type).
- `select_positions`: 3 uses, all in `execute_boundary_call` (:3235, :3241, :3265).
- `family_tuple_member_value`: 1 caller (:2962).

**Stated limits.**
- "host reference results require an explicit captured-input interface" (:3396).
- "materialized host reference result has no invocation-owned public region" (:3549).
- "component provider was called before its admitted installation" (:3203).
- "opaque host provider requires an admitted write-only non-observation judgment bound to its exact selected implementation and receipt" (:3358): an opaque host provider that `requires_checked_non_observation` cannot run at all in this file.

**Fail-open risks.**
- `execute_select_implementation` (:2899-2905): writes `table` into an integer result without a range check. What gets through: a program whose result scalar type is narrower than the table number (for example a `u8` result and table id above 255). Whether the checker prevents that is not traced here. "open". Also: no check that the operation's table id is consistent with an interface; only that the table exists (:2896).
- `execute_boundary_call` step 2: `boundary_declaration.declaration.as_ref().map(..) == Some(EmptyRunRelease)` runs the release and advances (:3081-3089) with no argument checks in this file; the check is inside `release_empty_run` (not read). "open".
- `execute_port_write` (:3627-3650) forwards the effect to the handler with `value` and no check of `service` or `port` here (the arguments come straight from `dispatched_operation!`); validation is the handler's, or is done at admission. "open".
- `complete_boundary_effect`: a handler can return a `Reference` result only if some `Invocation` region exists (:3537-3551). Other result kinds go to `commit_boundary_result` (not read; "open" for what it checks).
- `boundary_effect_wait` requires `suspension_crossing` before suspending; otherwise an error (:3480-3487). Fail-closed.
- No `true`-on-`None` pattern seen in this section.

---

## Same-job duplicates, summary (inside this file)

1. Result-kind triple switch (Unit/Scalar/Structural to the three `begin_*` calls): :1158-1186, :2974-3000, :3295-3342.
2. `CompletionReceipt` to `ClaimTransfer` mapping: :1150-1157, :3287-3294.
3. Owned/non-Unrestricted/whole-place source retirement: :170-204, :3579-3606, and via `transferred_call_sources` :1552.
4. Type-identity text equality: :1374-1386 (inputs), :1775-1787 (results), :2153-2160 (`map_identity`).
5. Scalar-array vs opaque parameter split: :92-107, :3218-3234.
6. `family_tuple_member_value` string parse of `named(integer-const(N))`: this file :3708; typed-trees `calls_and_control_flow.rs:978`; `unit_arguments.rs:6641`; `monomorphization.rs:2611` (grepped; other sites unread).

---

## Coverage

- Read in full: `engine_calls.rs` 1-3713 (3713 lines).
- Skimmed: none.
- Not read: none.

## Five things a maintainer most needs to know about this unit

1. Every call kind funnels through `enter_call` (:440), which moves ALL per-frame tables into a `SuspendedCall` via `suspend_with` (:207-244) and replaces them with the callee's. The scalar-only `Call` operation uses its own shorter path (`execute_call`, :3652) with a stricter precondition: no structural parameters or claims (:3664-3669).
2. `execute_boundary_call` (:3056-3457) is one ordered chain that decides which implementation supplies a boundary: lifecycle placement, target-family member, component provider, checked provider, access boundary, then host effect. A boundary that matches an earlier case never reaches the later ones; the order is the semantics.
3. Component calls (`component_arguments`, `component_calls`, `component_contents`, `component_scalar_arguments`, `component_empty_arguments`) match the receiver's and provider's types by identity TEXT (:1374-1386, :1775-1787, :2153-2160) and refuse inputs that carry any loan, resource, descriptor, element-view or original-storage state (:1474-1497). Lent borrows are supported only for scalar leaves (:2518-2529) and empty records (:2405).
4. `family_tuple_member_value` (:3708) decides dynamic-dispatch slot selection by parsing the identity string `named(integer-const(N))`; the same string is produced and parsed in at least four other places across the compiler (grep above).
5. Several checks compare only a type, not the full contract: scalar results at :671, :1078-1081, :1748; `execute_select_implementation` writes the table id without a range check (:2899-2905). These are the places a nominal/range mismatch would not be caught in this file.

## What I am unsure about

- The return side (restoring the caller frame, `SuspendedCallResult` handling) is in `engine_terminators.rs`; I did not read it, so what happens after `enter_call` fails midway (:556-581) is untraced.
- Callees I did not read: `prepare_boundary_arguments`, `into_call_arguments`, `opaque_values`, `forward_host_atom`, `commit_boundary_result`, `release_empty_run`, `execute_access_boundary`, `admit_boundary_crash`, `borrowed_storage_reference_ceiling`. Claims about "skipped" parameters in `bind_byte_sequence_arguments`/`bind_element_view_arguments` depend on them.
- Whether the checker guarantees a `SelectImplementation` result type can hold the table id.
- Whether the `named(integer-const(...))` parse sites share a helper; I only saw the format and parse lines in the grep, not the surrounding code.
