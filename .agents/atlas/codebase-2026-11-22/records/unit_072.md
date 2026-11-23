# Record: unit_072 (area 05-source-representations, crate `typed-trees`)

All paths are under `source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/` (written `T/`). Checkout `4f3c255d0559`.
Everything below was read in full unless marked "grepped" or "inferred". Crate map section read first (`crate-map/05-source-representations.md`, typed-trees).
Module paths: `T/callables/mod.rs` and `T/definitions/mod.rs` only list sub-modules; the crate exposes them as `crate::source_state::...` (`pub use crate as source_state`, per crate map). Some files in this unit import with `crate::{TypedTrees, data_applications}` (e.g. `callables/static_operand_binding/mod.rs:3`) and others with `crate::source_state::...`; both forms are used.

---

## 1. `callables/calls_and_control_flow.rs` (2949 lines)

**Purpose.** One file holding three namespaces of typed-tree row types: `calls` (signatures, static machine applications), `control_flow` (machines, states, statements, assembly graphs) and `names` (identifiers, a schema fingerprint). Header comment: `calls_and_control_flow.rs:1-5`. It is mostly data definitions plus the copy and symbol-remap code for statements.

**Shape.**
- `calls::signature` (9-220): `StateSignature` (95-119; fields incl. `invokes`, `contracts`, `termination_guarantee`, `declares_no_crash_route`, `service_reach_row`), `StateParameter` (156), `SignatureContract`/`SignatureContractKind` (188/202), `CrashCause` (216), `AuthoredInvocationTarget` (47: `Unresolved | Undeclared | Parameter | Service | AppliedService`), `ReachSubject` (16), `state_parameter_position` (165).
- `calls::static_applications` (221-413): `StaticMachineApplication` (239-256; `registration_operation`, `lexical_owner`, `binding_declaration`, `bindings`, `lifetime_bindings`), `StaticMachineBinding(Value)` (265/271: `Type | Const | RuntimeValue | Machine | Conformance`), `project_type_operand` (289), `TypedTrees::static_application_lifetimes_for_declaration` (304-411).
- `TypedTrees::call_has_no_runtime_receiver_for_target` (420-504) and private `call_has_no_runtime_receiver` (506-533).
- `control_flow::machine` (538-917): `Machine` (613-658, about 35 fields), `MachinePurpose` (589), `MachineBodyStatus` (582), `TraitConformance` (781), `GenericConformanceBound` (763), `SatisfiedDeclaration` (796), `resolve_satisfied_declaration` (815-912), free `machine_by_symbol` (914), `source_target_context` (556).
- `control_flow::state` (918-1011): `State` (927), `state_by_symbol` (936), `state_position` (943), `parameter_domain_membership_contract` (951-1009).
- `control_flow::statement` (1012-2766): `StatementNode` (2530: `Assembly | AssemblyFact | Assignment | Call | Expression | Evaluate | LocalData | Transition`), `TableCall` (2601), `TableLocalData` (2633), `TableTransition` (2690), `TransitionTargetNode` (2742), `StatementTable` (2177-2189, nine arenas), assembly graph types (1024-1066), `StatementControlGraph`/`StatementExecutionOrder` (1148-1229), copy (`copy_statement_nodes_with_type_expressions` 1429-1730, wrapper `copy_statement_nodes_deep_from` 1417), remap (`remap_statement_symbols` 1731-1931), `expression_roots` (1932-1993), `direct_state_return_values` (2101), `reachable_transition_prefix`/`reachable_state_statements` (2154/2164).
- `names::identity::normalized_schema_report_fingerprint` (2774-2864), `names::name::Identifier` (2873; `IdentifierText` = `Missing | Static | Generated` at 2878).

**Inputs and outputs.** Row types are stored in `TypedTrees`; functions read `TypedTrees` tables (`expression_table`, `statement_table`, `symbols`) and return refs or new handles. The copy function (`:1429`) appends copied statements, expressions, patterns and assembly graphs into `self` from `source_tables`. `remap_statement_symbols` (1731) rewrites symbol handles in place through `SymbolRebinding` (`remapped(...)`).

**How it decides.**
- `call_has_no_runtime_receiver_for_target` (420) decides "namespace qualifier, not runtime receiver" by a chain of symbol-kind and identity tests: machine holding the target state (421-428), selected operator call (429), `static_record_bound_requirement` Ok(Some) (434-447), receiver symbol kind `ConformanceParameter` plus `evidence_bound_requirement` with a name compare `call.target == requirement.signature.name` and an argument count (448-461), the trait owner of the target symbol (462-478), and finally a scan over all `self.traits()` requiring exactly one trait with the target symbol and `call.target.as_str() != signature.name.as_str()` (479-495). Keyed on symbol identity plus a string compare of the written target name against the declared name (458, 475, 492).
- `call_has_no_runtime_receiver` (506-533): symbol kind `Conformance | ConformanceParameter` (527) or equal to `owner.attached_data_symbol` (532).
- `resolve_satisfied_declaration` (815): trait path keyed on `requirement.name == *requirement_name` (828) then, if several, on `normalized_result_dispatch_set` equality (836-838); non-trait path builds `format!("{}::{}", conformance.name, requirement_name)` (859) and looks up a signature-free top-level path by string (888-896), then filters by `realizes_signature` (868, 879).
- `Machine::symbol_spelling` (691-700) builds `format!("{}::{}", name, target)`.
- `parameter_domain_membership_contract` (951): splits `domain_full_name.split("::")` (981) and creates generated identifiers from the parts: a domain is rebuilt from a spelled path string, with `domain_symbol` also passed.
- `finite_execution_order` (1232): visits block graph with a `visited` vector; returns `None` on revisit (1242); returns the order when the reachable window is shorter than the window or ends in an unguarded transition (1248-1256).
- `control_graph` (1276): returns `Err(String)` for each malformed assembly shape (1282, 1293, 1299, 1306, 1317, 1330, 1338, 1343-1346, 1359, 1371).
- `direct_state_return_values` (2101): returns `None` unless every transition is `Ordinary`, targets valid, `TransitionTargetNode::Value` (2109-2123); `Assembly` anywhere returns `None` (2135).
- `normalized_schema_report_fingerprint` (2774): sorts fields/cases by `identity` only if every member has `Some(identity)` (2814-2819); otherwise keeps declaration order and hashes name and position (`member_name` 2778-2795); type text comes from `display_type_reference` (2827, 2848), i.e. a display string, not an identity id. Hash domain tag `b"omega.schema.v2"` (2798). Result `.max(1)` (2863).
- `Identifier` equality (2942) compares `as_str()`; the three storage variants are interchangeable.

**Cases by shape.**
- `StatementNode` match in copy (1443-1694) and remap (1740-1921) lists all eight variants; no wildcard.
- `control_graph` handles `AssemblyExit::{Fallthrough, Jump, EnclosingTransition}` and refuses `ZeroFlag` ("conditional assembly control must be normalized before finite execution", 1344).
- `static_application_lifetimes_for_declaration` (304): accepts only same-declaration, or a requirement/owned-realizer pair (`corresponds` 348-359), else `Err` with a fixed string.
- `has_calling...` not here (see section 16).

**Same job elsewhere.**
- Copy and remap of statements are two parallel walkers over the same eight variants (1429 and 1731); a third walker `expression_roots` (1932) lists roots. Within the unit, expressions from `copy_proof_fact_from` (2007) and `remap_proof_fact_symbols` (2064) repeat the three-variant `ProofFact` match.
- Statement-form and expression-form of "call has no runtime receiver": `call_has_no_runtime_receiver_for_target` (420) vs `operator::selected_named_expression_call_has_no_runtime_receiver` (`definitions/operator_declarations.rs:896`) vs the statement form (`:1066`).
- Syntax-tree copying: suspected the same job (copy of bodies between forests) in `representations/syntax-trees/copying/bodies.rs` (not read).
- `normalized_schema_report_fingerprint` vs other schema/identity hashes (`declaration_reflection.rs` builds `SemanticSchemaGraph` identities): suspected, not read.

**Unused or one-caller code** (grepped, `source-rust/`, counts by word):
- `control_graph`, `StatementControlGraph`, `StatementControlBlock`, `StatementControlExit`: occurrences only inside this file (23 matches, 1 file; command: Grep `\b(control_graph|StatementControlGraph|StatementControlBlock|StatementControlExit)\b` count mode).
- `StatementTable::finite_execution_order` (1268), `StatementExecutionOrder`, `StatementExecutionIter`: only inside this file (3 hits of `finite_execution_order` in source-to-psi, all in this file). `StatementExecutionOrder` has a unit test only (1387-1408).
- `direct_state_return_values` (2101), `reachable_transition_prefix` (2154): no occurrence outside this file under `source-to-psi/`; `reachable_state_statements` only called at 1246 and 2171.
- `copy_statement_nodes_deep_from` (1417): 1 hit (this file only). `copy_statement_nodes_with_type_expressions`: 8 hits in 3 files (specialization.rs, values.rs, this file).
- `call_has_no_runtime_receiver_for_target`: 4 files (`read/byte_observations.rs`, `read/calls.rs`, `proofs/evidence_and_inspection.rs`, this file).
- `resolve_satisfied_declaration`: 13 hits in 11 files.
- free `machine::machine_by_symbol(machines, symbol)` (914): no call found in non-method form (greps for `machine::machine_by_symbol` and `machine_by_symbol(&...`/`machine_by_symbol,` returned none); the `TypedTrees::machine_by_symbol` method (`storage/declaration_tables_and_index.rs:1501`) is a different function with many callers.
- `state_by_symbol` free fn (936): used at `storage/declaration_tables_and_index.rs:3765, 4178, 4354, 4397`; `state_parameter_position` (165): `pipeline/03.../lowerer.rs:320`, `definitions/service_declarations.rs:1321`; `state_position` (943): the `state_position(` grep matched only the definition (no external call).
- `parameter_domain_membership_contract`, `machine_authored_target`, `source_target_context`: 9 hits in 5 files.
- `normalized_schema_report_fingerprint`: 1 external caller (`driver/compile_time_invocations/fixed_layouts/validation.rs`).
- `project_type_operand`/`static_application_lifetimes_for_declaration`: 18 hits in 8 files.

**Stated limits.** "conditional assembly control must be normalized before finite execution" (1344, error text). "Calls can acquire source applications after body completion" (2193) explains a page split, not a limit.

**Fail-open risks.**
- `call_has_no_runtime_receiver_for_target` returns `true` at 430 (any selected operator call), 446 (static-record bound requirement) and 496-497 (`!call.receiver.is_valid()` after the single-trait scan). A call whose receiver is absent and whose target symbol belongs to exactly one trait with matching name and argument count is treated as having no runtime receiver. Open (no comment at the site; callers not traced).
- `call_has_no_runtime_receiver` (515) returns `true` when `!call.receiver.is_valid()` and the target symbol equals the state's symbol. Traced within the file: guarded by `call.target_symbol != target.symbol` (512).
- `normalized_schema_report_fingerprint`: when some member lacks an identity, order falls back to declaration order (2814); two schemas that differ in order would then hash differently, not equal. No acceptance. None seen.
- `Identifier::as_str` returns `""` for `Missing` (2901): a missing name compares equal to an empty written name. Open.

---

## 2. `callables/machine_instances.rs` (676 lines)

**Purpose.** Traces a copied machine ("instance") back to the template it was copied from, through two kinds of receipt: a `MachineSpecialization` or a `CapturedMachineApplication`/`CapturedMachineDeclaration` (map capture). Also checks that retained receipts still agree with the declarations.

**Shape.** `MachineInstantiation<'p>` enum (15: `Specialization | Capture`); `EnclosingInvocationFrame` (23); `entry_template` (54-92); `TypedTrees::validate_executable_template_symbol` (97), `specialization_binding_declaration` (111-264), private `specialization_carriers_match` (266-309), `enclosing_invocation_frames` (313-400), `captured_conformance_owner` (404-488), `captured_declarations` (490), `machine_instantiation` (508-528), `machine_instantiation_path` (530-548), `enclosing_map_capture` (552-571), `instantiated_machine_state_matches` (573-591), `static_machine_binding_position` (595), `static_machine_binding` (607-675). Re-exports `NamedCaptureError`, `NamedConformanceCaptureIndex` (10).

**Inputs/outputs.** Reads `machine_specializations`, `captured_machine_applications`, `captured_machine_states`, `data_type_parameters`, `symbols`, `expression_table` static applications. Produces refs, `Vec<MachineInstantiation>`, `Vec<EnclosingInvocationFrame>`, or `Err(&'static str)`.

**How it decides.**
- `machine_instantiation` (508): collects all receipts keyed on `instance` symbol identity (`for_instance`, and `declaration.instance == instance` across all captured declarations, 516-521); more than one receipt is an `Err` (524-525); none is `Ok(None)`.
- `entry_template` (54): finds the single `captured_machine_states` row whose `state.instance == entry` (80-83), checks lengths agree with both machines' state lists (77), and checks both symbols occur (84-89). Keyed on symbol identity, never on name; comment states "State order and symbol provenance do not supply missing correspondence" (51-53).
- `specialization_binding_declaration` (111): walks `symbols.get(ancestor).parent` until `declaration` (144-152); checks the retained parameter prefix equals the template's parameter prefix, the remainder maps with `generated_from` (165-181); compares binder `kind` per pair, using type-identity text (`specialization_carriers_match` 266) for `Const`/`Value` carriers (197-220); checks parameter kinds against binding value variants (242-257).
- `captured_conformance_owner` (404): checks `machine_callable_identity_text(source)` equals `declaration.normalized_template_identity` (418-419) (a string compare of identity text), then walks parents bounded by `selection.declarations.len()` (447).
- `static_machine_binding` (607): ascends specialization receipts by `owner = instance.template` (673), stops at a `Machine` binding, and compares `entry(argument.symbol)` with `entry(target)` (653).

**Cases by shape.** `MachineInstantiation::{Specialization, Capture}` only (14-21). `captured_conformance_owner` accepts parent kinds `Machine` and `State` only (465-475).

**Same job elsewhere.** `machine_specializations.rs` (index over the same receipts). `enclosing_map_capture` (552) and `NamedConformanceCaptureIndex::get` (`map_captures.rs:93`) both walk to the owning map capture. `specialization_carriers_match` (266) computes type identity under `PackageQualified`, similar contexts to `compiler_intrinsics.rs:360`, `runtime_callable_identity.rs:180` (different options).

**Unused/one-caller** (grepped): the set `MachineInstantiation|EnclosingInvocationFrame|enclosing_invocation_frames|captured_conformance_owner|machine_instantiation_path|enclosing_map_capture|instantiated_machine_state_matches|static_machine_binding_position|specialization_binding_declaration|validate_executable_template_symbol` has 82 matches across 21 files (26 in this file); used by pipeline 04 specialization, pipeline 05 `read/calls.rs`, `declare.rs`, `read/machine.rs`, `provider_methods.rs`, and typed-trees storage. No function in the file appears unused.

**Stated limits.** none. Comment at 392-393: "Inferred instances without an authored outer application retain themselves as the terminal supplying owner."

**Fail-open risks.** None seen as acceptance. `machine_instantiation` and `static_machine_binding` return `Ok(None)` for "no receipt/binding", and callers decide: open for `static_machine_binding` (returns `Ok(None)` for invalid binder, no retained contract, argument with an application, or a non-matching entry at 615-618, 652-656, 646-672; a caller reading `None` as "not statically bound" would treat the call as dynamic; callers not traced).

---

## 3. `callables/machine_instances/map_captures.rs` (285 lines)

**Purpose.** A transient index of "captured machine applications" (prepared copies of a named conformance's map body) keyed by caller and complete argument tuple.

**Shape.** `NamedCaptureError { Invalid(&'static str), Expanding }` (11); `NamedConformanceCaptureIndex { rows: arena::HandleKeyMap<CaptureKey, Option<CaptureHandle>> }` (34-36); `new` (39), `insert` (49), `get` (69-156), private `lookup` (158), `capture_key` (166-263), `introduces_nested_type` (266). `CaptureKey = (SymbolHandle, NormalizedTypeIdentity)` (9).

**Inputs/outputs.** Reads `program.captured_machine_applications`, `static_machine_application`, `conformance_by_symbol`, `type_identity_text`; produces a handle or an error.

**How it decides.** The key is `(source.lexical_owner, applied_index_identity(...))` (262): symbol identity plus a type-identity id. `insert` marks a key ambiguous (`None`) when two different handles share it (60-64). `capture_key` cross-checks identity text bytes against `capture.conformance_application.identity_encoding` (252-259). `get` (69): when no enclosing group matches, it counts "growth steps" through `introduces_nested_type` and errors `Expanding` if growth steps exceed argument count (136-153).

**Cases by shape.** `get` special-cases `StaticArgumentCall::Expression` with a valid `requirement_application` (77) and statement calls separately.

**Same job elsewhere.** `enclosing_map_capture` (`machine_instances.rs:552`) is called from `get` (93).

**Unused/one-caller** (grepped): `NamedConformanceCaptureIndex` 10 matches in 7 files (specialization.rs, named_conformances.rs, pipeline 05 `lib.rs`, `callable_applications.rs`, `exports.rs`, this crate files).

**Stated limits.** `NamedCaptureError::Expanding` message: "recursive map call has no established finite argument closure" (20). Comment 141-142: "Allow that finite chain."

**Fail-open risks.** `NamedConformanceCaptureIndex::new` discards each `insert` error (`let _ = index.insert(...)`, 44). Stated: "Unrelated unsupported records grant no authority. A demanded named application still needs one valid, unambiguous entry." (42-43); a rejected record is simply absent, so a later `get` finds nothing. Stated.
`get` returns `Ok(selected)` where `selected` can be `None` after the growth check (155); a caller treats that as "no capture". Open.

---

## 4. `callables/machine_specializations.rs` (355 lines)

**Purpose.** Table of specialization receipts with an index that finds receipts by `instance` without scanning (log-structured sorted runs). Comment 1-32.

**Shape.** `MachineSpecializations { rows: Vec<MachineSpecialization>, by_instance: Vec<usize>, runs: Vec<usize> }` (38-45); `from_rows` (54), `reindex` (65), `for_instance_rows` (80), `for_instance` (100), `push` (107), `retain` (128), `clear` (132), `edit` (140, rebuilds index on drop), `Deref<Target=[MachineSpecialization]>` (164), `Record` impl writes only rows (194-207). Equality compares rows only (156-160).

**How it decides.** Index key is `(arena_index, generation)` of the instance symbol (49-51); lookup binary-searches each run with `partition_point` (92-93). Merge rule: merge while newer run length >= older (111-125).

**Same job elsewhere.** none in unit; callers elsewhere filter `machine_specializations` directly (not traced).

**Unused/one-caller** (grepped): `.for_instance(` / `.for_instance_rows(` 13 hits in 8 files (4 in this file). `for_instance_rows` (the row number variant): the only non-test mention outside this file would be in `for_instance` (104); no other caller seen in the `\.for_instance(_rows)?\(` count (inferred, not separately counted). The comment calls it "the one index"; users outside the file: specialization.rs (2), callable_signature_shapes.rs (1), machine_instances.rs (2), static_environment.rs (1), access_views_and_commitments.rs (1), references.rs (1), declaration_tables_and_index.rs (1).

**Stated limits.** none. **Fail-open.** none seen.

---

## 5. `callables/mod.rs` (14 lines)

Lists the module tree: `callable_lifetimes`, `callable_copies`, `callable_signature_shapes`, `callable_values`, `machine_instances`, `machine_specializations`, `runtime_callable_identity`, `callable_applications`, `calls_and_control_flow`, `private_callback_slots`, `source_call_occurrences`, `static_operand_binding`. Only `callable_*` and `static_callback_contracts` are not all declared here: `static_callback_contracts.rs` is not listed in this `mod.rs` (14 lines read); grepped `static_callback_contracts` use at `callables/` not done; it is presumably included from another file (inferred; unverified).

---

## 6. `callables/private_callback_slots.rs` (207 lines)

**Purpose.** Builds and validates a "private callback slot": the closed `PrivateCallbackSlot` core conformance binding one semantic type, one fixed layout and one boundary requirement.

**Shape.** Three `TypedTrees` entry points: `private_callback_slot_application` (22, `&mut self`), `check_private_callback_slot_declaration` (34), `check_private_callback_placement_owner` (46); core `close_private_callback_slot` (62-206).

**Inputs/outputs.** Inputs: a `StaticMachineArgument`, owner symbol, optional expected layout policy, `sources: &[(SourceId, [u8;32])]`. Output: `PrivateCallbackSlotApplication { application: ClosedConformanceApplication, selection: PrivateCallbackSlotSelection }` and the applied scope. Builds identity bytes through `ConformanceIdentityDescription{..}.encoded_bytes()` (160-166) and commitments (167).

**How it decides.** Core vocabulary of the trait symbol (`CoreVocabulary::PrivateCallbackSlot`, 79-80); closed memberless implementation (81-85); exactly three trait arguments (88-96); layout argument's applied scope equals the declared bound (105-114); requirement must name a `State` symbol whose parent trait is boundary (115-129). Keyed on symbols and applied scopes. The comment at 150-151: "The machine binder selects the exact overload." Placement owner must be the owning `FixedLayoutPlan` realization (52-57) via `core_vocabulary(row.requirement)`.

**Cases by shape.** Exactly three type arguments `[semantic, layout, requirement]` (88). Closed named application only: refuses `const_literal`, `evidence_projection`, `application`, `authored_machine_application` (69-75). Error text 86 says "closed memberless core PrivateCallbackSlot implementation".

**Unused/one-caller** (grepped): the three entry points have 6 hits in 4 files (`driver/.../fixed_layouts/placements.rs`, `read/calls.rs`, `read/traits.rs`, this file).

**Fail-open risks.** none seen; every unmatched case returns `Err`.

---

## 7. `callables/runtime_callable_identity.rs` (1986 lines)

**Purpose.** Computes the canonical identity text of a callable type: signature (static binders, parameters, result, contracts, reach, invokes, suspends/blocks, termination, native callbacks), machine values and propositions. Differences in the selected machine must not change the callable type (comment 1-5).

**Shape.** `RuntimeCallableIdentityError` (38, 10 variants), `normalize_contract` (156), `with_contract_scope` (167), `with_signature_scope` (196-346), `normalize_signature` (348-438), helpers `parameter_shape` (440), `properties` (466), `machine_contract` (480), `conformances` (545), `clauses` (579), `fact` (669), `encode_fact` (713), `nominal_proposition_endpoint` (813), `proposition` (950), `reach` (1056), `invocations` (1122), `termination` (1160), `callbacks` (1205), `normalize_machine_value` (1239), `impl TypedTrees` public wrappers (1339-1482), `normalize_type_in_context` (1502). Tests at 1531-1985.

**Inputs/outputs.** Reads `TypedTrees` signature tables, `type_identities` interner, service reach rows, domains; produces `String` identity text built from `compound`/`atom` records (88-115).

**How it decides.** Binders are renamed by position (`$C{depth}G{n}`, `V`, `E`, 232-245); sets (conformances 574, clauses 647, reach 1104, invokes 1155, termination 1200) are `sort()`ed and `dedup()`ed as strings. Reach: `open` when `service_reach_row.0 == 0` (1063); ceiling label chosen `open | installation-bound | finite` (1108-1117). A `[copy]`/`[linear]` bound is encoded as strings `"copy"|"affine"|"linear"` and `carry` is always the literal `"omitted"` (466-477, comment 473: "Existing callable identities include the absent source-policy marker."). `result_text` treats an invalid result reference as `Unit` (133-143, stated in the comment 128-132). Recursion: a signature already on `active_callable_signatures` (pointer equality) is `RecursiveSignature` (205-211).

**Cases by shape.** `machine_contract` (480): `RequirementIdentity`, `Structural`, `Nominal` (with `instantiated` required, 501-503). `encode_fact`: `ContentConservation` is refused (733). `proposition` handles `Primitive | Witness | Transparent{BooleanExpression | Application}` (1026-1051). `TypeParameterKind` match covers all six variants (360-396).

**Same job elsewhere.** `compiler_intrinsics.rs:249-444` (`normalized_schema`) builds another canonical identity for a compiler-provided machine header with a separate tag vocabulary (`Tag` 180); `callable_signature_shapes` (other unit) judges "realizes signature"; `operator_declarations.rs:1287` (`operand_signature`) calls `runtime_callable_operand_shape` (here, 1476). `static_callback_contracts.rs:284-303` calls `normalize_signature` on two signatures and compares text.

**Unused/one-caller** (grepped; 11 names, 59 hits in 13 files). `runtime_machine_value_identity` etc. are used in pipeline 03 `runtime_callables.rs`, pipeline 05 `read/callables.rs`, pipeline 07 `source_selection_publication.rs` (6), and several typed-trees files.

**Stated limits.** Error strings: "callable identity contains an unresolved ..." (52-75). Comment 607-610 explains why `clauses` reads only clauses.

**Fail-open risks.**
- `reach` (1091-1103): for an authored `ReachSubject::Service(id)` it only checks the definition exists and adds nothing to the identity text; the identity already carries services from the row (1074-1090). Two signatures whose rows differ only in the authored row but have equal resolved rows get the same identity text. Inferred from reading; not traced.
- `properties` fixes `carry` to `"omitted"`: a signature's carry policy cannot affect identity. Stated (473).
- `fact` (697-709): the closure `|selected| { if !selected.is_valid() { substitution::rejected(context) } ... }` marks invalid then continues (703-706). Traced: `rejected` sets the context's `invalid_index_argument` flag, and `with_contract_scope` turns it into `UnresolvedType` (190-192). Traced.

---

## 8. `callables/source_call_occurrences.rs` (229 lines)

**Purpose.** Generic tree traversal that finds the first "call site" in an expression or statement, tracking whether a branch is conditional (`selected`). Header: "records occurrences and conditional ancestry; it never proves a call." (1-2).

**Shape.** Traits `ExpressionCalls` (9) and `StatementCalls` (129); functions `find_in_expression` (28), private `find` (35), `find_in_statement` (149), `find_in_transition` (204).

**How it decides.** Skips unreachable expressions (`static_type_contexts.is_unreachable`, 41). For `Binary And/Or` with a literal-boolean left operand `false`/`true`, skips the right operand (75-80). `Match` arms and `And/Or` right-hand sides run through `traversal.selected(...)` (53, 57, 82). Calls count when `call.runtime_call.is_some()` or `named_call_is_selected` (88).

**Cases by shape.** Only the left operand being a literal `Boolean` is constant-folded (67-74).

**Unused/one-caller (grepped).** `ExpressionCalls|StatementCalls|find_in_statement|find_in_expression|find_in_transition` over the whole checkout excluding `.agents/` (Grep count mode): 8 matches in 1 file, this file. No implementer or caller outside it. The module is declared `pub mod source_call_occurrences` (`callables/mod.rs:13`) and re-exported in `exports.rs:9`.

**Fail-open.** none seen (nothing uses it).

---

## 9. `callables/static_callback_contracts.rs` (610 lines)

**Purpose.** After a callback parameter is specialized into a direct call, keeps the callback's required contract (applied signature) with the application and re-checks stored contracts on recovery.

**Shape.** `AppliedStaticCallbackRequirement` (14); `prepare_static_callback_requirements` (21-145); `static_callback_requirement_matches` (147); `validate_recovered_callback_contracts` (191-313); `static_callback_source_matches` (315); `static_callback_requirements` (332-406). Test module (409-610).

**Inputs/outputs.** Reads the static application, its bindings, arguments, lifetime rows, `retained_static_machine_contract`; writes `applied_callable_signatures` rows (`AppliedCallableOwner::StaticCallback`) and `callable_lifetime_substitutions`. Recovery check recomputes the expected contract text and compares it to the stored one (284-308).

**How it decides.** Roster equality: callbacks sorted by `(arena_index, generation)` of the parameter symbol, compared pairwise with stored rows (351-365, 370-404); `source == original` structural equality of the signature (329); binding snapshot equality (175-176); lifetime rows equality (177-186). Recovery compares two normalized-signature strings (304).

**Cases by shape.** Only `StaticMachineBindingValue::Machine` bindings produce rows (97); no `Machine` binding returns `Ok(())` immediately (36-41).

**Same job elsewhere.** none in unit. `normalize_signature` comes from `runtime_callable_identity.rs:348`.

**Unused/one-caller (grepped).** the three functions have 13 hits in 6 files (specialization.rs, its tests, `read/calls/callback_bindings.rs`, `read/calls_visibility_tests.rs`, `storage/frozen.rs`, this file).

**Fail-open risks.** `prepare_static_callback_requirements` returns `Ok(())` early when `static_callback_requirements(application).is_ok()` (79-81): an already stored roster is kept without recomputation; the recovery validator (191) is where text is recompared. Traced: `static_callback_requirements` checks roster, owner, source and snapshots (354-405); but not the signature text itself; that is checked in `validate_recovered_callback_contracts` called (per the test at 592-607) from frozen decode. Partially traced (decode path in `storage/frozen.rs` not read).
`validate_recovered_callback_contracts` returns `Ok` when no application exists (209-211). Traced: nothing to verify.

---

## 10. `callables/static_operand_binding/mod.rs` (697 lines)

**Purpose.** Reads one static argument tuple against its declared binders and records the binding on the application. Header: "Source calls and Build operands use the same typed identities and requirement headers." (1-2).

**Shape.** `Stop { Unserved(Cow), Rejected(String), Defect(&'static str) }` (18), `Served<T>` (28), `unserved` (30), `Identity { Type, Declaration, Scalar, Value }` (38) with an `Ord` that agrees with equality (52-65), `Operand` (87) with `Operand::read` (103-255), `Value` (289), private `Scalar` (314), `closed_scalar` (328), `only_a_name` (360), `named_callable` (371), `named_implementation` (402), `named_static_record` (453), `type_argument_meets_bounds` (528), `named_type` (560), `require_written_lifetime_arity` (648), `bind` (682). Re-exports `prepare_requirement_application`, `prepare_requirement_in_scope` (13).

**How it decides (`Operand::read`, 103).** Branches on `binder.kind`: `Type` (121), `Implementation` (136), `Const` for static record (140), `Const` for non-primitive aggregate const declaration (165), other `Const` scalar (185), `Machine` with complete contract (206), and refusals for `Machine` without contract (234), `Value` (239) and `Proposition` (242). A non-`Type` binder with non-default `bounds` is "unserved" (112-118). Identity for a type argument is `Identity::Declaration(symbol)` if the named symbol is a `TypeParameter`, else `Identity::Type(normalized_type_identity)` (124-133). Scalars key on `value.spelled()`, a canonical spelling string (188). Statics keyed `Identity::Value(index_identity)` (159, 179). Machine arguments are keyed on the symbol (`Declaration(chosen)`, 230). `named_callable` requires kind `Machine | State | MachineParameter` (380-383). `named_implementation` requires `ConformanceParameter` or a plain `Conformance` whose trait equals the single declared bound's trait and all argument lists empty (417-446). `type_argument_meets_bounds` (528) accepts `Affine` binders unconditionally, otherwise requires `stored_value_multiplicity(reference) == required`.

**Cases by shape.** Closed list in `closed_scalar` (321-329): integer literal for fixed-width builtin integer, `true`/`false` for `bool`, or the bare name of a type parameter in scope. `require_written_lifetime_arity` (648) emits a fixed message for a written data name lacking lifetime arguments.

**Same job elsewhere.** `requirement.rs:97-104` and the pipeline-04 `specialization.rs:587,1034` each loop `Operand::read` + `bind`. Count of `Operand::read` call sites outside tests: specialization.rs (587, 980, 981, 1034), named_conformances.rs (140), requirement.rs (98). Tests: 10 more.

**Unused/one-caller (grepped).** `prepare_requirement_in_scope`: specialization.rs:1116, product_operand/formation/applications.rs:876, requirement.rs:33. `prepare_requirement_application`: specialization.rs:1124, formation/applications.rs:796. `type_argument_meets_bounds`: `pipeline/03.../type_reference.rs:544`, `pipeline/05.../read/machine.rs:3274`. The comment "Every kind of parameter is bound through this one shape" (84-86): `Operand::read` has 3 non-test callers in pipeline 04 and 1 here.

**Stated limits.** "A request this binding does not serve yet, with the check that stopped it." (19); "a kind with no reading yet is unserved" (86); "a value binder has no specialization operand" (240); "a proposition binder has no specialization operand" (243).

**Fail-open risks.**
- `type_argument_meets_bounds` returns `Ok(())` for `Multiplicity::Affine` (535-537). Stated (520-527): "An unbounded (affine) binder admits any type and leaves the uses to that closed check." Stated.
- `named_type` (560): when `argument.type_reference.is_valid()` and not `Named`-of-parameter, the reference is accepted after `require_declared_application_operands` and `data_applications::prepare` (593-604); an `Err` from `prepare` is `Rejected`. None seen beyond that.
- `Operand::read` for `TypeParameterKind::Const` scalar: a name that is not a literal but passes `only_a_name` and is a `TypeParameter` kind symbol is accepted as `Scalar::Parameter` (346-353): the parameter's own type is not compared with the binder's carrier. Open (name equals "a type parameter in scope" per comment 321-326, but the carrier type is not checked at 347-353).

---

## 11. `callables/static_operand_binding/nominal_callback.rs` (102 lines)

**Purpose.** Checks that a selected callback satisfies the exact requirement of a nominal machine binder on the receiving trait application.

**Shape.** `require_nominal_selection` (7-76), `receiving_scope` (78-102).

**How it decides.** Forwarded parameter (kind `MachineParameter`): its own binder must be `Nominal` with equal `requirement` symbol and the same applied trait scope (17-44). Concrete callback: counts owned realizations (`owned_conformance_realization` matching `row.requirement == requirement && row.realization_state == selected_entry.symbol`) plus direct `machine_trait_conformances` rows whose applied scope `same_application` as required; exactly one must match (57-74).

**Fail-open.** none seen (returns `Err(Stop::Rejected)` on count != 1).

---

## 12. `callables/static_operand_binding/requirement.rs` (120 lines)

**Purpose.** Closes a trait-requirement application: binds each static argument and, if all are closed, instantiates the closed requirement header.

**Shape.** `prepare_requirement_application` (8-34), `prepare_requirement_in_scope` (39-120).

**How it decides.** Returns `Ok(None)` ("nothing to prepare / leave as written") when: target is not a requirement (14-19), no own parameters and declared signatures kept (56-58), trait arguments are not owned by the lexical owner (86-91), or any operand is open (113-115). Errors `unserved` for trait/requirement lifetime parameters (20-28, 59-63), argument count mismatch (64-68), registration mismatch (69-78).

**Fail-open risks.** `Ok(None)` at 90 (`require_owned_application_operands(...).is_err()` swallows the error and returns `Ok(None)`): an application whose trait arguments fail the ownership check is left unprepared, not refused here. Open (no comment; later stages not traced). Similarly 56-58 is structural.

---

## 13. `definitions/build_selections.rs` (48 lines)

`TypedTrees::build_provider_requirement_scope` (7-47): resolves a type reference to a boundary trait application (`Named` or `Generic` only, 16-29), requires `declaration.is_boundary` (31-33), builds `AppliedTraitScope` (34-44) and `require_receiving_trait_operands` (45). Grepped: 26 hits for the group of four names in 12 files; this function appears in pipeline 05 `read/providers.rs`-class callers (inferred from the group count; individually 1 hit outside the file not separately counted). Fail-open: none seen (returns `Err` on every miss).

---

## 14. `definitions/compiler_intrinsics.rs` (1053 lines)

**Purpose.** Derives the signature schema of a compiler-provided machine (a sealed `CompilerPrimitive`) from its ordinary `Machine`, so a token view or operand count cannot stand in for the contract (1-3).

**Shape.** `TypedTrees::compiler_primitive_schema` (26-68) returns `Result<Option<(CompilerPrimitive, NormalizedTypeIdentity)>, Diagnostic>`; `byte_observation_header` (75-175), `Tag` (180), `schema_span` (239), `normalized_schema` (249-444), `contracts` (446), `termination_schema` (481). Tests 541-1052.

**How it decides.** Custody: `self.symbols.sealed_declaration(symbol, machine.name.as_str())` (33-35) (name passed as string with the symbol); then `supply_mode == CompilerProvided && !body_is_present` (47-52). `byte_observation_header` (75) checks about 25 header fields by equality, then the shape `shared &[u8]` with one named lifetime returning `bool` (101-174), identifying `u8`/`bool` via `builtin_type_atom` (171-174). `normalized_schema` destructures `Machine` exhaustively "so a new Machine field requires a deliberate schema decision" (250-251), refuses a long list of states (291-310), accepts reach only `NULL` ("inferred") or `EMPTY_ROW` ("none") (313-321), binders named `$static{n}`/`$value{n}` (335-342), kinds only `Type`/`Const` (380-388), contract facts only `ProofFact::Expression` with no binding and no `EnsuresForResultCase` (453-466). Embeds `format!("{:?}", parameter.bounds)` (391) and `format!("const={};mutable={};relevance={:?}")` (402-405) and a large `format!` envelope with `{interface_exposure:?}`, `{purpose:?}`, `{spelling:?}`, `{assembly_target_requirement:?}` (436) into the identity: debug-format strings.

**Cases by shape.** Exactly one entry state (322-323); `ByteObservation` requires `lifetime_parameters.len() == 1` and exactly one parameter (88, 111-115). Comment 72-74: "Each Core observation takes one shared byte slice with its own lifetime, returns bool, and always completes without effects."

**Same job elsewhere.** `runtime_callable_identity.rs:348-438` (a different identity of a signature); `declaration_reflection.rs:559-576` (parameter descriptions).

**Unused/one-caller (grepped).** `compiler_primitive_schema`: in the group count (26 hits, 12 files) it appears in pipeline 05 `read/boundaries.rs`, `read/calling.rs`, `read/calls.rs`, `read/machine.rs`, `read/providers.rs`, and pipeline 06 tests. `normalized_schema` private.

**Stated limits.** "no product route yet: complete compiler primitive signature schema" (64, error text); "Nonempty/abstract service rows need their complete declaration-owned normalization before this query can describe them." (318-319).

**Fail-open risks.**
- `compiler_primitive_schema` returns `Ok(None)` when no machine has the symbol (31-32) or when custody is not a `CompilerPrimitive` and `supply_mode != CompilerProvided` (37-45). A caller reading `None` as "not a primitive" gets the ordinary path. Open (callers not traced).
- `termination_schema` returns `Some("inferred")` for `InternalDerived` (488). Stated by the comment at 478-480 ("an interface description, never termination evidence").

---

## 15. `definitions/declaration_reflection.rs` (1754 lines)

**Purpose.** Builds the semantic schema graph and the field/parameter/callable description identities for reflection queries, under the author's visibility. Header (1-5): "Queries describe selected types using the caller's visibility. ... Descriptions alone grant no permission".

**Shape.** `reflection::construction` (12-1477): `declaration_query_argument_role` (28), `schema_query_authority` (64), `schema_subject_symbol` (81), `schema_subject_revision` (109), `exact_symbol_identity` (126), `declaration_source_revision` (143), `DescriptionAuthor` (158), `field_description_identity` (258), `field_description_in_scope` (273-481), `parameter_description_identity` (483), `parameter_description_in_scope` (498-577), `callable_description_identity` (582-683), `authenticate_callable_descriptions` (689), `callable_description_sources` (700-858), `collect_nominal_expectations` (998-1173), `describe_field` (1175), `describe_type_parameter` (1214), `construct_semantic_schema_graph` (1235), `..._for_type` (1257), `construct_data_schema_graph` (1300-1419), `push_field_node` (1422), `sorted` (1473). Tests 1481-1753. `pub use schema_reflection::*` (9).

**Inputs/outputs.** Reads `TypedTrees` data, trait, symbol tables, the source map (`symbol_provenance_source_span`, `source_file`); outputs `SemanticSchemaGraph`, `FieldInfoDescription`, `ParameterDescription`, `CallableDescription` (a tuple `(bool, [Vec<u8>;3])`, 673-681).

**How it decides.**
- Authorization (`DescriptionAuthor::authorize` 164): product mode uses `find_product_declaration_from_source(path, occurrence, [(subject, public)]) == Some(subject)`, which includes a path string; ordinary mode uses `source_reference_may_select_symbol(span, subject)` (178-186).
- Authority = `OwningScope | ForeignScope` by `same_symbol_source_package` (71) or `same_product_package_instance` (204-210).
- Foreign scope refuses non-public data (1314-1324).
- Field identity string: `exact_symbol_identity` returns a hermetic identity if available, otherwise `display_path` with `::` and a `false` flag (133-140) (a spelled path).
- `callable_description_sources` (700): sorts requested descriptions by identity bytes and does a `partition_point` range search (716-722, 750-753); candidates come from (a) every `ExpressionNode::Call` whose target machine has `CoreVocabulary::Reflection(Callable)` (774-820) and (b) every machine (823-831) and trait requirement signature (832-840). A request with no match is an error (841-848).
- `describe_type_parameter` embeds `format!("{:?}", parameter.bounds)` (1231) and `construct_data_schema_graph` embeds `format!("{:?}", data.supply_mode)` and `format!("{:?}", data.properties.multiplicity)` (1402-1403): debug-format strings go into the description.
- `nominal_expectations`: owner kind recorded as `format!("{:?}", typed.symbols.get(symbol).kind)` (1034).

**Cases by shape.** `field_description_in_scope` (273): only a `Field` kind selected declaration; member selections through `Data|TypeAlias` suppliers or a `Variant` with no arguments (302-332); "a field query is not its declaring owner's exact field row" unless exactly one occurrence (411-415). `TypeReferenceNode` match in `collect_nominal_expectations` (1042-1171) lists all variants. Schema root: `Primitive` shape for scalar builtins (1269-1292), otherwise `Data` only (1240). Quotients are refused (1307-1312).

**Same job elsewhere.** `field_description_in_scope` and `parameter_description_in_scope` each have a product variant (`product_*_description_identity`, 221, 241) and an ordinary variant; two paths differ in the `scope.product` flag (535-543).

**Unused/one-caller (grepped).** The 16 reflection function names: 83 hits in 17 files; 40 inside this file. Users: driver `declaration_lookup.rs` (2), `typed_call.rs` (2), `domain_access_plans/*` (12), pipeline 04 `reflected_fields/active_cases.rs`, pipeline 05 `read/reflection.rs` (6), `read/source_records.rs` (7), `qualifications.rs`, and others.

**Stated limits.** none labelled. Comments at 533-534 and 772-773 explain two paths.

**Fail-open risks.** none seen; every unmatched case returns `Err(String)`. `callable_description_sources` skips candidates it cannot identify (`let Ok(identity) = ... else { return; }`, 747-749; and `continue` at 780-818) and only errors when a requested description is unmatched (841). Traced in file: a skipped candidate cannot add a match.

---

## 16. `definitions/declarations.rs` (3364 lines)

**Purpose.** Typed declaration rows (data, domains, constants, measures, wire schemas, aliases, layout storage), their visibility, plus several semantic queries built over them. Header (1-5).

**Shape (modules, with `declarations.rs:line`).**
- `boundary_completions` (7-495): `boundary_requirement_completion` (12), `declared_completion` (63), `exact_process_exit_requirement` (82), `exact_process_exit_signature` (132), tests 169-494.
- `calling_vocabulary` (497-616): `standard_calling_vocabularies` (504), `has_calling_interface` (540), `is_calling_interface_data` (592).
- `constant` (617), `data` (634-993; `DataDefinition` 641, `TypeParameter` 769, `TypeParameterKind` 796, `MachineParameterContract` 821, `DataMemberIdentity` 901), `domain` (994-2022; `DomainDefinition` 1092, `DomainIssuance` 1076, `AccessPlanJudgment` 1025, `AppliedDomainApplications` 1693, closure walkers 1830-1991), `intrinsic_requirements` (2023-2176), `invocations` (2177-2266), `layout_storage` (2267-2598), `measure` (2599), `numeric_intrinsics` (2614-2767), `visibility` (2770-3222), `wire` (3223-3332), `type_aliases` (3334-3364). Re-exports 2768-2769.

**Inputs/outputs.** Reads `TypedTrees` tables and `symbols`; most functions return `bool`/`Option`/`Result<_, Diagnostic|String>`.

**How it decides (selected).**
- `exact_process_exit_requirement` (82): bundled trait must have `sealed_member_intrinsic(definition.symbol, "exit_process") == Some(HostedRequirement(ProcessExit))` (93-99), one signature, name literally `"exit_process"` (108), parameter `i32` and return type with `CoreVocabulary::Never` (132-167). Keyed on symbol custody from the bundled catalog plus a string name of the member.
- `standard_calling_vocabularies` (504): groups non-boundary traits by source file id in a `BTreeMap<usize,...>` (507-522) keyed by the literal names `"Calling"` and `"CallingPolicy"` (523-526); `has_calling_interface` requires a `plan` signature, data names `"BoundarySignature"` and `"BoundaryPlanResult"` declared in the same source as the policy (540-615). String compares of names plus same-source test, no sealed custody.
- `DomainDefinition::issuance` (1117): no policy and no routes -> `Unrestricted` (1128); policy with `Checked` plan -> its issuance; `Unevaluated | Unjudged` -> `Planned` ("admitting nobody it can name", 1085-1088).
- `domain_accepts_carrier` (1358): `true` if `has_generic_carrier` (1369-1371); else compares `package_qualified_type_identity` of carrier and substituted target, or of a layout-storage semantic type (1386-1391).
- `declaration_visibility` (2890): per-kind lookup in retained tables, requires all copies to agree (`consistent_visibility` 2957-2965); fields, variants and states inherit from parent (2935-2953).
- `authored_selection_visibility` (2823): for machines follows `machine_instantiation` receipts to the authored template (2857-2870).
- `layout_storage_arguments` (2412): recognises `LayoutStorageData` by sealed custody, two params (type, implementation), one bound whose trait is named `"FixedLayout"` (2458), has `SourceOrigin::Toolchain`, and is in the same source file as the constructor (2459-2465).
- `numeric_intrinsic_execution` (2637): resolves `Namespace::name` strings via `numeric_declarations::resolve` (2697) and `accepts(...)` over parameter primitive types; the float class is detected by `self.display_type_reference(return_type) == "FloatClass"` (2705): a display-string compare.
- `closed_domain_argument_identity` (1427): accepts `Named` args that parse as `i128` by name (1495-1497) or via `CanonicalConstValue::from_atom(name)` (1490); otherwise a path-based `binder:` string (1503-1506).
- `native_data_declaration_identity` (2340): falls back to a `HashSet`-driven walk of `collect_type_symbol_dependencies` (2356-2387), then returns `closed-data-instance::{identity text}` (2388-2391).

**Cases by shape.**
- `has_calling_interface` (540-590): "exactly one" type parameter of kind `Implementation`, exactly one bound, exactly one `plan` with exactly one parameter.
- `numeric_token_execution` (2719): comparison spellings only for six `OperatorSpelling` values (2744-2751) and primitive float binary operators (2727-2738).
- `declaration_visibility` kinds: exactly `Data, TypeAlias, Conformance, Domain, Machine, Operator, Measure, Proposition, Trait, Const, WireSchema` plus parent-inherited `Field/Variant/State` (2908-2930). `Measure` is always `is_public: false` (2915-2923).
- `field_layout_source` (2306): scalar / explicit plan-laid / fixed array of those / default.

**Same job elsewhere.**
- Three "is this static argument closed" walkers: `static_argument_is_closed` / `index_argument_is_in_scope` (1931, 1961) here; `closed_boundary_application_type` (`operator_declarations.rs:379`); `closed_multiplicity_carrier` (`operator_declarations.rs:2622`).
- `exact_process_exit_*` (82-167) and `byte_observation_header` (`compiler_intrinsics.rs:75`) both recognise exact bundled declarations from a header; different catalogs.
- `domain_by_symbol` (1394) vs `TypedTrees::domain_definition_for_instance` (1808).

**Unused/one-caller (grepped, source-rust).**
- `boundary_requirement_completion|compiler_primitive_schema|build_provider_requirement_scope|standard_calling_vocabularies`: 26 hits in 12 files (see section 14).
- Domain helpers (`indexed_domain_instance_name`, `has_generic_carrier`, `index_parameters`, `has_direct_index_telescope`, `domain_accepts_carrier`, `subject_membership_facts`, `type_selects_semantic_domain`, `domain_subject_reference`, ...): 67 hits in 17 files (16 in this file).
- `index_argument_is_closed|..._with_substitutions|index_argument_is_in_scope|static_argument_is_closed`: 26 hits in 14 files.
- `DomainIssuance|requires_access_retention|access_unjudged|has_complete_*_theory`: 45 hits in 13 files (18 in this file).
- `declared_signature_invocations|declared_machine_invocations|machine_entry_parameters`: 8 hits in 3 files (4 here, 3 in `service_declarations.rs`, 1 in `build/planning/package_review/terminal_permissions.rs`).
- `field_layout_source|native_data_declaration_identity|core_layout_declaration|layout_storage_arguments|plan_laid_layout_for_type`: 37 hits in 15 files.
- `DeclarationVisibility|authored_selection_visibility|declaration_visibility|owning_machine_is_retained|requires_declaration_visibility`: 37 hits in 3 files (33 here, 3 in `build/planning/package_review/declarations.rs`, 1 in `declaration_reflection.rs`).
- `IntrinsicRequirement|IntrinsicRequirementKind`: 25 hits in 6 files.
- `numeric_intrinsic_execution` and friends: `grepped` by group only (included above in the 25 for `IntrinsicRequirement`? no). Not separately counted.

**Stated limits.**
- "Observation demands and boundary reach remain unserved until their supply/lifecycle consumers can prove them." (1050-1051).
- "Other semantic definitions need their own translation before the theory is complete" (1135-1137).
- Comment on issuance origin: "written on the declaration with the retired `established by` clause" (1081).
- `AccessPlanJudgment::Unjudged` comment (1031-1036): "stated restrictions no consumer reads yet".

**Fail-open risks.**
- `domain_accepts_carrier` returns `true` for any valid carrier when the domain's target type is its own first type parameter (1369-1371). Input: a generic-carrier domain (`domain D<T> on T`). Open (design looks intended; no comment).
- `boundary_requirement_completion` returns `NormalReturn` unless the exact process-exit signature matches (28, 78). A requirement that never returns but is not recognised is treated as returning. The file states this recognises "its signature, not an emptiness proof or a grant of process-exit authority" (128-131). Stated.
- `DomainIssuance::Unrestricted` when no policy and no routes (1128). Stated in the doc at 1077-1079.
- `numeric_intrinsic_execution` calls `numeric_token_execution` first and returns that execution without the signature `accepts` check (2692-2696). Input: an operator definition with a recognised spelling and primitive parameters; `numeric_token_execution` checks parameters (2753-2759) and `primitive_float_binary_semantics` checks them (`operator_declarations.rs:1665-1686`). Traced.
- `standard_calling_vocabularies` accepts any user-source pair of traits named `Calling`/`CallingPolicy` in one file whose shape matches; no origin or sealed-custody check (504-538). Input: a user package declaring such traits. Open.
- Data-oriented ratchet: this file holds `HashMap` (1695-1697, 1758, 1769), `HashSet` (1838-1839, 2360) and `BTreeMap` (507); `tools/ratchets/dense_id_maps.txt:102` lists the file with count 2. Not reconciled.

---

## 17. `definitions/declared_bindings.rs` (610 lines) and `declared_bindings/tests.rs` (925 lines)

**Purpose.** Look up the declared type of a binder (state parameter or local) inside a state's body, and look up locals globally by symbol, with a per-body index for big bodies.

**Shape.** `DIRECT_BINDING_ROWS = 32` (18); `BindingScope` (21), `StateDeclarationIndex` (74) with `bodies: Vec<(BodyScope, PreparedBindings)>` sorted by scope and `roots: Option<RootBindings>`, `RootBindings` (123). `TypedTrees` methods: `declared_binding_type` (359), `declared_binding_type_by_symbol` (383), `state_local_declaration` (418), private `binding_position` (436), `local_data_positions_for_symbol` (488), `unique_local_data_for_symbol` (494), `first_named_value_declared_type` (521), `state_position_for_parameter_symbols` (576). Tests in `declared_bindings/tests.rs` (`#[cfg(test)] mod tests;` at 609-610).

**How it decides.** Small bodies (parameters + statements <= 32) scan directly and need exactly one declaration (448-467). Larger bodies use the prepared index; `[position]` must be a single row (174-177, 480-483). Body scope identity is the tuple of the state's parameter and statement span starts, generations and counts (34-45). Cache validity is keyed on table `revision()` and an `Arc<()>` identity (`BindingScope` equality 26-30). A failed lookup is cached as a refusal. `unique_local_data_for_symbol` keeps aliases' multiplicity (492-493).

**Cases by shape.** 32-row cut-over (448). `build_roots` indexes bodies only if above the cut (268-271); smaller bodies' parameters are skipped from the root index (274-276).

**Same job elsewhere.** Tests re-implement the scan (`scanned` 50-79, `scan_field` 650-669, `matches_global_roster` 255-308) to compare with the index.

**Unused/one-caller (grepped).** `declared_binding_type|local_data_positions_for_symbol|state_local_declaration|...` group not separately counted. `state_position_for_parameter_symbols` 2 hits (here and tests). `unique_declared_field_by_symbol` and `data_field_positions_for_symbol` are used by tests here but defined elsewhere (not in this unit).

**Stated limits.** "Preserve the legacy first-match query" (519): `first_named_value_declared_type` returns the first match over all machines, owned data, states, propositions (540-571).

**Fail-open risks.** `declared_binding_type_by_symbol` has a fallback when the state's own roster has no unique match: it scans block-value statements after the first span and returns the first local with that symbol, without a uniqueness check (393-402). Input: a symbol that appears twice in block-valued bodies would resolve to the first. Open (no comment; callers not traced). `first_named_value_declared_type` is first-match by design (stated, 519).

---

## 18. `definitions/mod.rs` (15 lines)

Module list: `compiler_intrinsics`, `declared_bindings`, `source_names`, `native_locators`, `service_declarations`, `declaration_reflection`, `declarations`, `numeric_declarations`, `operator_declarations`, `product_operand`, `trait_definition`, `source_subjects`, `build_selections`. `source_names`, `service_declarations`, `source_subjects`, `trait_definition` are outside this unit.

---

## 19. `definitions/native_locators.rs` (482 lines)

**Purpose.** Validates the compiler-owned `ForeignBinding` and `DllImport` data shapes (their three const `u64` width parameters and case lists) and reads closed widths from a binding application.

**Shape.** `NativeLocatorVocabulary` (12), `vocabulary` (17-58), `application_widths` (60-108), `unused_coordinate_widths` (110-171), `exact_vocabulary` (173), `binding_application` (204), `exact_width_parameters` (218), `validate_dll_import_shape` (251), `validate_binding_shape` (318), `require_fixed_bytes` (397), `binding_widths` (420).

**How it decides.** Finds data definitions by name `"ForeignBinding"`/`"DllImport"` plus `core_vocabulary(...)` being `ExternalBinding(..)` and `CheckedShape`, public, non-instance (21-42). Hard-coded names: width parameters `["ObjectLength","SymbolLength","VersionLength"]` (229); DllImport cases `PeByName`, `PeByOrdinal`, `ElfVersioned`, `MachODylibSymbol` with field name lists (257-262); binding cases `DllImport`/`Syscall` with fields `import`, `number` (340, 386-388). `binding_widths` parses widths from type-reference names that are decimal strings with no symbol (467-481): a string-parse decision.

**Cases by shape.** Exactly 4 `DllImport` cases and 2 `ForeignBinding` cases, in that order; `unused_coordinate_widths` returns hard-coded masks per ordinal: `[[false,false,true],[false,true,true],[false;3],[false,false,true]]` for DllImport and `[[false;3],[true;3]]` for ForeignBinding (127-134, 170).

**Unused/one-caller (grepped).** `application_widths` has 1 external caller (`pipeline/07.../build_companions.rs:728`). `vocabulary`, `NativeLocatorVocabulary`, `unused_coordinate_widths`: no occurrence outside this file (greps of `native_locators::`, `unused_coordinate_widths`, `binding_widths`, `application_widths` found only the one caller and this file; `vocabulary` is a generic word, checked via `native_locators::` only). `binding_widths` is `pub` but used only inside this file (106) per the same grep.

**Stated limits.** none.

**Fail-open.** none seen (all `Err`/`None`). `unused_coordinate_widths` returns `None` for unknown cases.

---

## 20. `definitions/numeric_declarations.rs` (263 lines)

**Purpose.** Maps a source declaration's namespace and spelling (e.g. `F32::from_i64`, `I32::from_f64`, `F32::negate`, `F32::add_toward_nearest`) to a `NumericSignature`.

**Shape.** `resolve` (8-38), `numeric_primitive` (40), `conversion_operand` (54). Tests 62-262.

**How it decides.** Namespace string `"F32"`/`"F64"` or `I`/`U` + decimal digits (40-52). Name prefix `from_` makes a conversion (14); otherwise a float builtin via `FloatBuiltin::from_name(name)` or `"negate"` (26-35); conversions to a float ignore the domain (19-23). Pure string matching on spelling.

**Unused/one-caller.** Called from `declarations.rs:2697` (`numeric_requirement_execution`). Visibility `pub(super)`.

**Fail-open.** none seen (returns `None` for unknown).

---

## 21. `definitions/operator_declarations.rs` (4530 lines)

**Purpose.** Everything about operator declarations: spelled-operator candidate selection, closed operator applications, trait operator meanings, named-call resolution, operand signatures, primitive float/integer operator semantics, type matching.

**Shape (sub-modules of `pub mod operator`).** `applications` (8-577), `indexing` (578-746), `named_calls` (747-1262), `operand_signatures` (1263-1611), `primitive_float` (1612-1848), `primitive_integer` (1849-1903), `satisfied_operators` (1904-2007), `spellings` (2008-3491, incl. tests 2839-3490), `trait_operators` (3492-3990), `type_matching` (3991-4439), re-exports (4440-4482), `OperatorConstBinding` (4494), `OperatorDefinition` (4500: `spelling: Option<OperatorSpelling>`, `token_count`, `home_domain`), `SpelledOperator` (4516), `declaration_by_symbol` (4521).

**How it decides.**
- Spelled candidates (`spelled_candidates` 2032): from `operator_spelling_rows(spelling)`; visible when public or same source package (2070-2073) and `source_reference_may_select_symbol(use_site, symbol)` or (can-see and (has domain or `is_default_primitive_operator`)) (2075-2085).
- Operand matching (`operand_bindings_with_indexed_collection` 2379): unknown or primitive operands match in the first pass (2398); then numeric carriers are checked (2436-2446); `bindings_satisfy_known_multiplicities` (2447).
- Type matching (`carrier_type_reference_matches` 4054): structural; expected `Named`/`Generic` that is a declared type parameter or `bindable_owner` binds (4114-4138); `Named` vs `Named` compared by symbol, falling back to name strings only if `allow_name_fallback` and both symbols invalid (4384-4398). `type_reference_matches` passes `allow_name_fallback = true` (4021); `_with_policy` passes a given value.
- Named calls (`named_call...`): operator path matched by last member string `== target_name` and prefix equality with receiver segments (1026-1050); arity `positional == argument_count (+1 for value receiver)` (1052-1064); visibility `source_reference_can_see_symbol` (891-894).
- Float operators: `primitive_float_binary_semantics` (1641) requires namespace string `"Float"`, requirement name strings `add/subtract/.../greater_or_equal` (1646-1661), equal primitive parameter types and expected result (1662-1686); `is_sealed_toolchain_float_operator` (1690) uses sealed custody + `display_path == "{ns}::{name}"` + `core_source == FloatOperations` (1701-1708).
- Qualification (`qualification_identity` 2501): `Constrained` types contribute sorted `(declaration handle, normalized constraint)` pairs; `Named(_)` constraint -> `None` (2532-2534).
- `closed_boundary_application_nominal` (424): true if the name is a canonical const atom or parses as `i128` (429-433), else symbol kind `BuiltinType | Data`.
- `abstract_requirement_may_have_operator_bound` (3932): walks bounds and refinements with a `visited` list; returns `true` on any inconsistent roster (3946-3950, 3963-3967) or missing trait (3958-3960).
- `canonical_closed_operator_realization_bytes` (441): re-derives each row with `closed_operator_realization_application` and compares to the retained row (472-481); package digest is hex-encoded into the row text (559-571).

**Cases by shape.**
- `closed_application_for_operands` (141): only `Type` and `Const` binders; `lifetime_parameters` must be empty (147, 151-158); `Machine | Implementation | Proposition` binders return `None` (208-211).
- `closed_operator_realization_application` (299): first state only (304), same parameter count, `is_const`/`is_mutable` equal (307-317).
- `symbolic_operator_type_application_for_operands` (216): type binders only; no const binders (226-233, 265).
- `closed_indexed_operator_application_for_operands` (127): only `OperatorSpelling::Index | Range` (132-137).
- `builtin_integer_result_carrier` (1853): `Addr` arithmetic limited to (Add Addr,U64), (Add U64,Addr), (Sub Addr,U64) -> `Addr`, (Sub Addr,Addr) -> `U64` (1863-1869); shifts return the left carrier; others need equal carriers (1871-1885).
- `named_expression_call_candidates` receiver kinds: `Module | BuiltinType | Data | Domain | Machine | Trait` (947-955, 1109-1117, 1169-1176).
- `core_integer_operand` (2133): one policy among constraint nodes (2157-2166), integer builtin only.

**Same job elsewhere.**
- Expression and statement versions of the same named-call logic: `named_expression_call_candidates` (822) vs `named_statement_call_candidates` (982); `expression_static_receiver_segments` (929) vs `statement_static_receiver_segments` (1103); `selected_named_expression_call_has_no_runtime_receiver` (896) vs the statement version (1066); family-path builders `named_expression_call_family_path` (1186) and `named_statement_call_family_path` (1211) over one `named_call_family_path` (1158).
- Operator-to-spelling maps: `binary_operator_spelling` (2095), `arithmetic_operator_spelling` (2106), `comparison_operator_spelling` (2117), `operation_spelling` (1627), and the `BinaryOperator -> Operation` map in `declarations.rs:2727-2738`.
- Closedness walkers: `closed_boundary_application_type` (379), `closed_multiplicity_carrier` (2622), `declarations.rs:1931`.
- `trait_operator_operand_bindings` (3536) vs `operand_bindings_with_indexed_collection` (2379): both call `type_reference_matches_with_policy`.
- `qualifications_match` (2472) is invoked both inside `type_reference_matches_with_policy` (4046) and again by callers (e.g. 257, 363, 2431).

**Unused/one-caller (grepped).**
- `closed_operator_application_for_operands`: outside this file, 1 caller (`pipeline/03.../operator_calls.rs:121`); `ClosedOperatorApplicationArgument` is also used there (10, 16, 532, 540). The other eight names: `closed_operator_application_from_bindings`, `closed_indexed_operator_application_for_operands`, `symbolic_operator_type_application_for_operands`, `closed_operator_realization_application`, `canonical_closed_operator_realization_bytes`, `ClosedOperatorRealizationApplication`, `SymbolicOperatorTypeApplicationArgument`: no occurrence outside this file in `source-rust/` (Grep with glob `!**/operator_declarations.rs`, content mode, returned only `operator_calls.rs` lines); none in `tools/` (count mode: 0).
- `resolve_named_call` (752): only defined and re-exported (grepped `\b(resolve_named_call|...)\b` content: lines 752, 4456 only). `selected_named_statement_call_has_no_runtime_receiver` (1066): only the definition and re-export (1066, 4458). `named_statement_call_candidates`: 1 external caller (`expressions/references.rs:3907`).
- `builtin_integer_result_carrier|integer_peer_operand_carrier|is_sealed_toolchain_float_operator|primitive_float_binary_semantics|is_default_primitive_operator`: 18 hits in 3 files (2 in `pipeline/05.../read/expressions.rs`, 1 in `declarations.rs`, 15 here).
- `resolve_satisfied_boundary_operator_for_conformance|boundary_operator_requirement_identity|operator_operand_signature|trait_operator_*|selected_trait_operator_*|bound_trait_operator_meanings|trait_scope_operator_*`: 33 hits in 6 files (25 here).
- Named-call family (14 names): 75 hits in 21 files; none of `named_call_family_members|...` individually counted.

**Stated limits.** Comment 2433-2434: "Missing numeric syntax supplies a carrier only." The doc at 1142-1157 explains a family of machines with one path and "checking selects the member from the operand and result types".

**Fail-open risks.**
- `bindings_satisfy_known_multiplicities` (2597-2620): if the actual type is not a "closed multiplicity carrier", the `[copy]`/`[linear]` bound is not enforced (`!closed_multiplicity_carrier(..) || ...`, 2617-2618), and a missing binding also passes (2611-2616). Wrong program accepted: an operator candidate with a `[copy]` type parameter matched against an open generic, callable-without-identity, or reference-with-lifetime type. Open (no comment).
- `closed_boundary_application_nominal` (429-433) accepts any `Named` whose name parses as an integer as "closed", regardless of symbol. Open.
- `nominal_type_identity_matches` with `allow_name_fallback` compares unresolved types by name string (4391-4397); `type_reference_matches` (allow=true) is used by `indexing::borrowed_collection_match` (720-728), `operator_matches_receiver` (2828-2836) and `operand_bindings...` (2415-2422). Input: two unresolved references with the same spelling. Open.
- `abstract_requirement_may_have_operator_bound` returns `true` on malformed rosters (3946-3950, 3963-3967, 3958-3960) which makes `has_scoped_operator_meaning` true (2271-2275) and thereby blocks the builtin meaning (2309): fail-closed for builtins, but where else `has_scoped_operator_meaning` is read is not traced.
- `resolve_named_call` (752) returns `program.operator_definition_by_symbol(target_symbol)` when the target symbol is valid without checking arity or path (760-762); also `resolve_named_expression_call` (812-814). Input: a call whose target symbol is an operator but whose shape does not fit. Open (the first has no external callers).
- `fixed_array_lengths_match` (4303): falls back to `actual == expected` when the expected length is not a const parameter or the parameter is not found (4316-4323), and `allow_name_fallback && actual == expected` when the actual length is not closed (4329-4331).

---

## 22. `definitions/product_operand/applications.rs` (233 lines)

**Purpose.** Rows and queries for "Product operand applications": a prepared declaration operand (boundary subject, provider, requirement, root-grant subject) with the environment and scope it was prepared under.

**Shape.** `ProductApplicationDemand { FormationOnly, Selected }` (10), `ProductOperandApplication` (17-32), `TypedTrees` methods: `product_operand_application_identity` (35-115), `product_operand_application_role` (117), `product_operand_declaration_role` (124), `product_requirement_application` (145-181), `prepared_product_operand` (183), `retain_dependency_product_application` (201), `retain_product_operand_application` (217).

**How it decides.** Role `RootGrantSubject` resolves to `Provider` if `conformance_by_symbol(declaration)` exists, else `Requirement` if `trait_machine_signature(declaration)` exists, else `Err` (129-139). Identity text exists only for roles `BoundarySubject | Provider | Requirement` (44-53); a declaration with no parameters and no lifetimes (non-requirement) returns `Ok(None)` (58-63). `prepared_product_operand` (183) finds an existing row by linear scan on `(operand, environment, owner, role)` equality (190-198); `retain_product_operand_application` returns it if found, else inserts (221-231); `expect("prepared Product operand has a declaration role")` (227). `retain_dependency_product_application` dedups by linear scan (205-209).

**Cases by shape.** Three nominal roles (44-51).

**Unused/one-caller (grepped).** the 9 names: 80 hits in 22 files (22 in this file; 14 in `formation/mod.rs`, 7 in `formation/applications.rs`; others in driver `compile_product`, pipeline 05/07, build planning).

**Fail-open risks.** `product_operand_application_identity` returns `Ok(None)` for a parameterless, lifetime-free `BoundarySubject` or `Provider` (58-63): such an application gets no identity text. Stated by structure (a nominal declaration without arguments is its own identity), no comment. Open (consumer not traced).

---

## 23. `definitions/product_operand/declaration_lookup_tests.rs` (183 lines)

Test file. `use super::*;` pulls `ProductDeclarationLookup`, `resolve_product_path`, `resolve_build_request_author`, `ProductOperandKind` from `definitions/product_operand/mod.rs` (not in this unit; inferred from the identifiers used). Three tests: `product_declaration_lookup_workload` (41, 8 and 512 declarations x 4096 lookups, prints elapsed time with `eprintln!`, 58-61), `product_declaration_names_follow_mutation_and_exact_rosters` (66, asserts renames, root-span changes, ambiguity and a record round trip), `product_declaration_authors_keep_exact_span_and_ambiguity` (129, asserts `"ambiguous authored machine custody"` and `"no authored selecting machine"` message substrings, 171, 181).

Also test files in the unit: `declared_bindings/tests.rs` (925 lines; 13 tests comparing the indexed lookups to plain scans with stale-handle, replaced-storage, and frozen-copy cases), plus inline tests in `callables/*` and `definitions/*` listed above (`machine_specializations.rs:209-355` five tests, `runtime_callable_identity.rs:1531-1985` three tests, `static_callback_contracts.rs:409-610` one test, `compiler_intrinsics.rs:541-1052` nine tests, `declaration_reflection.rs:1481-1753` two tests, `declarations.rs:169-494, 1177-1203, 1509-1519, 2967-3221` tests, `operator_declarations.rs:1711-1836, 2839-3490` tests, `numeric_declarations.rs:62-262` five tests, `calls_and_control_flow.rs:1387-1408, 2497-2513` two tests).

---

## Coverage

Read in full (every line, in order), 25 ranges, 21,690 lines:
- `callables/calls_and_control_flow.rs` 1-2949 (2949)
- `callables/machine_instances.rs` 1-676 (676)
- `callables/machine_instances/map_captures.rs` 1-285 (285)
- `callables/machine_specializations.rs` 1-355 (355)
- `callables/mod.rs` 1-14 (14)
- `callables/private_callback_slots.rs` 1-207 (207)
- `callables/runtime_callable_identity.rs` 1-1986 (1986)
- `callables/source_call_occurrences.rs` 1-229 (229)
- `callables/static_callback_contracts.rs` 1-610 (610)
- `callables/static_operand_binding/mod.rs` 1-697 (697)
- `callables/static_operand_binding/nominal_callback.rs` 1-102 (102)
- `callables/static_operand_binding/requirement.rs` 1-120 (120)
- `definitions/build_selections.rs` 1-48 (48)
- `definitions/compiler_intrinsics.rs` 1-1053 (1053)
- `definitions/declaration_reflection.rs` 1-1754 (1754)
- `definitions/declarations.rs` 1-3364 (3364)
- `definitions/declared_bindings.rs` 1-610 (610)
- `definitions/declared_bindings/tests.rs` 1-925 (925)
- `definitions/mod.rs` 1-15 (15)
- `definitions/native_locators.rs` 1-482 (482)
- `definitions/numeric_declarations.rs` 1-263 (263)
- `definitions/operator_declarations.rs` 1-4530 (4530)
- `definitions/product_operand/applications.rs` 1-233 (233)
- `definitions/product_operand/declaration_lookup_tests.rs` 1-183 (183)

Skimmed: none. Not read: none. The large test modules (e.g. `operator_declarations.rs:2839-3490`, `declarations.rs:169-494`, `compiler_intrinsics.rs:541-1052`) were read but are summarised as test groups, not described test by test.

## Five things a maintainer most needs to know about this unit

1. `callables/source_call_occurrences.rs` (229 lines, `pub mod`, re-exported from `exports.rs:9`) has no implementer or caller anywhere in the checkout (8 matches for its traits and functions, all in the file; Grep over the repo excluding `.agents/`). Likewise `StatementTable::control_graph`/`finite_execution_order`, `direct_state_return_values`, `reachable_transition_prefix`, `copy_statement_nodes_deep_from`, `native_locators::{vocabulary, unused_coordinate_widths}`, `operator::resolve_named_call`, `selected_named_statement_call_has_no_runtime_receiver` and eight operator-application helpers (list in section 21) have no caller outside their own file.
2. Identities in this unit are mostly built as strings: `runtime_callable_identity.rs` (sorted, deduped string records), `compiler_intrinsics.rs:436` (a `format!` envelope that includes `{:?}` of enums), `declaration_reflection.rs:1231, 1402-1403, 1034` (`{:?}` of bounds, supply mode, multiplicity, symbol kind) and `declarations.rs:2705` (`display_type_reference(...) == "FloatClass"`). A change to a Rust `Debug` derive changes these identities.
3. Several recognisers key on authored names plus provenance rather than only sealed custody: `standard_calling_vocabularies` (`declarations.rs:504-615`, names `Calling`, `CallingPolicy`, `plan`, `BoundarySignature`, `BoundaryPlanResult`), `native_locators.rs` (case and field names, lines 229-262, 340, 386), `exact_process_exit_requirement` (member name `"exit_process"`, `declarations.rs:108`), `numeric_declarations.rs` (namespace/name spellings), `primitive_float_binary_semantics` (`"Float"` and operation names, `operator_declarations.rs:1646-1661`).
4. Static-binding judgments return three-way outcomes: `Stop::{Unserved, Rejected, Defect}` (`static_operand_binding/mod.rs:18`) and `Ok(None)` ("leave as written"). `requirement.rs:86-91` and `:56-58` return `Ok(None)` instead of refusing; `NamedConformanceCaptureIndex::new` discards insert errors (`map_captures.rs:44`); `machine_instantiation`/`static_machine_binding` return `Ok(None)`. Callers decide what `None` means.
5. Operator matching accepts in places where information is missing: `bindings_satisfy_known_multiplicities` skips non-closed carriers (`operator_declarations.rs:2617`), `closed_boundary_application_nominal` accepts integer-looking names (`:429-433`), and name-string fallback compares unresolved types (`:4391-4397`). `operator_declarations.rs` (4530 lines) also holds near-duplicate expression and statement versions of the named-call code (`:822/:982`, `:929/:1103`, `:896/:1066`).

## What you are unsure about

- Whether `callables/static_callback_contracts.rs` is declared as a module somewhere other than `callables/mod.rs` (`mod.rs:3-14` does not list it). I did not grep for `mod static_callback_contracts` or `#[path]` attachments.
- Whether the many "open" fail-open items reach wrong programs: I did not read the callers for `call_has_no_runtime_receiver_for_target`, `static_machine_binding`, `compiler_primitive_schema`, `domain_accepts_carrier`, `has_scoped_operator_meaning`, `declared_binding_type_by_symbol`.
- The counts for "outside this file" are word-boundary greps over `source-rust/` (and `tools/` where stated); calls through macros, re-export aliases or `#[path]` aliases with different spellings would not show up.
- `tools/ratchets/dense_id_maps.txt:102` lists `declarations.rs` with `2`; the file has 8 lines mentioning `HashMap`/`HashSet`/`BTreeMap`. I did not read the ratchet's counting rule.
- `product_operand/declaration_lookup_tests.rs` relies on items defined in `product_operand/mod.rs`, which is not in this unit.
