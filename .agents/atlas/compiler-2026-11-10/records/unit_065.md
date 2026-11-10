# Unit 65 notes

Files covered (1 file, 11585 lines, all read):
- `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs` lines 1-11585

Abbreviation: `VS:<line>` = that file. Other files are named in full (all under the same `src/` directory, so `values.rs` etc.).
The file is mounted by `#[path = "values_scalar.rs"]` at `values.rs:3463` (grepped: the only mention of `values_scalar` in `omega-rust`), so its modules appear as `crate::values::scalar::*`.

## 0. What the whole file is

Purpose. It lowers typed-tree scalar and Boolean expressions into the "checked scalar expression" form (`CheckedScalarExpression`, `CheckedBooleanExpression`). It does this for: executable statements (let, assign, call, transition, final expression), call arguments, and contract predicates (`requires`, crash conditions, `ensures`, parameter range refinements). It also builds the located plan rows (`CheckedScalarExpressionPlans`: `expressions`, `source_bindings`, `binding_symbols`) that later stages replay. The header comment (VS:1-24) says it is shared with `execution::scalar_computations`.

Layout (read; top-level `pub(crate) mod` blocks with line ranges):
| module | lines |
|---|---|
| header comment | 1-24 |
| `array_constructions` | 26-183 |
| `atomic_operands` | 184-256 |
| `boolean_lowering` | 257-825 |
| `call_arguments` | 826-1158 |
| `call_lowering` | 1159-2566 |
| `case_membership` | 2567-2790 |
| `constant_array_projection` | 2791-2858 |
| `contract_entry` (with nested `crash_entry`, which has nested `logical_matches`) | 2859-4298 |
| `expression_facts` | 4299-4503 |
| `expression_plans` (nested private mods `array_elements`, `assignment`, `call_statement`, `expression_statement`, `local_data`, `range_operands`, `transition`) | 4504-5539 |
| `machine_parameter_booleans` (nested `structural`, `structural_paths`) | 5540-6076 |
| `primitive_reference_read` | 6077-6247 |
| `result_contract` (nested `logical_matches`, `predicate_conditions`) | 6248-8277 |
| `scalar_lowering` (nested `element_selection`, `entry_lowering`, `integers`, `recast_reads`) | 8278-9843 |
| `selected_operator_operands` | 9844-9937 |
| `semantic_casts` | 9938-10085 |
| `structural_fields` | 10086-11249 |
| `subslice_endpoints` | 11250-11446 |
| `pub(crate) use` re-export block | 11447-11481 |
| private `erased_observations` | 11483-11585 |

Almost every function takes `program: &dyn ::validation::CheckingInput` plus `operators: &CheckedOperatorFacts`. Many take the same parameter bundle: `parameters` (dense scalar roster), `authored_parameters` (full roster), `parameter_types`, `locals: &[ScalarLocal]`, `exact_integer_casts` (VS:1245-1257, 8959-8969 are typical).

## 1. `array_constructions` (VS:26-183)

1. Purpose: finds the array-literal actuals of calls inside one statement, using the call facts already captured in flow, and emits `CallArrayConstruction` records. It schedules nothing and creates no synthetic call (comment VS:28-32).
2. Shape: `CallArrayConstruction {source: CheckedArrayConstructionSource, expression, type_reference}` (VS:39-43). `unique_state_flow` (VS:50) returns the state's single `FlowStateFact` or `None`. `call_array_constructions_in` (VS:62) is the main function.
3. I/O: reads `flow.control.calls` for the state (VS:70) and `validation::find_call_site` (VS:89). Returns a `Vec`; mutates nothing.
4. Decisions:
   - A call fact is skipped when a second fact at the same `(statement_index, call_ordinal)` exists (VS:78-88), when the call site cannot be found (VS:89-97), or when the entry symbol of the resolved target differs from the fact's `target_symbol` (VS:105-107).
   - A call that is not the statement's "direct root" and returns a primitive is skipped (VS:141-143).
   - A parameter is kept only when it is not const, not mutable, a closed primitive array type, and `scalar_array_elements` succeeds (VS:150-165). Keyed on identity (symbols, expression handles) and type shape.
5. Cases by shape: `direct_root` is a match on statement kind (VS:122-140): Call, LocalData (`call.authored_expression == local.initial_value`), Assignment (only if `call_is_boundary`), Expression, Transition (only `TransitionNamed`), `_ => false`. Arguments must equal parameters in length, or the call is skipped (VS:146-148).
6. Same job elsewhere: call-site enumeration per statement is done in three places: here (VS:70-97), `call_arguments::nested_call_sites` (VS:1030-1098), and `subslice_endpoints::sites` via `validation::collect_call_sites_in_statement` (VS:11333). Read all three; they differ in the filters applied.
7. Callers: `call_array_constructions_in` is used in-file at VS:898. `unique_state_flow` at VS:889 (grepped by name; only `values_scalar.rs` matches for `call_array_constructions_in`).
9. Fail-open: none seen. Every unresolved lookup does `continue`, which drops the construction row.

## 2. `atomic_operands` (VS:184-256)

1. Purpose: for a `place = Atomic { model }` assignment or atomic-once local, produces one scalar row per authored operand under role `AtomicOperand { operand_ordinal }` (comment VS:186-195).
2. Shape: `lower_carrier` (VS:208) returns `Option<Vec<(ExpressionHandle, Role, CheckedScalarExpression)>>`.
3. I/O: reads `carrier.place`, `carrier.operands` (from `validation::AtomicCarrierAccess`); calls `lower_return_expression` per operand at the place's primitive type (VS:226-246).
4. Decision: the primitive type comes from `declared_place_type_raw` of `carrier.place` (VS:226-228).
9. Fail-open candidate (open): the function's return type is `Option` but it only ever returns `Some(rows)` (VS:254); I read the body and found no `?` or `return None`. When the place's primitive type is `None`, the loop `break`s (VS:231-235) and `Some(empty)` is returned. An operand that fails to lower is skipped (VS:236-252). Both callers treat `Some` as "handled": `assignment::plan` returns early after retaining (VS:4622-4648) and `local_data::plan` returns (VS:4820-4843). So an atomic whose operands fail to lower yields fewer or zero rows and no failure here; I did not trace whether a later stage reports the missing rows.

## 3. `boolean_lowering` (VS:257-825)

1. Purpose: lower a typed Boolean expression or guard to `CheckedBooleanExpression` in the invocation's scalar namespace, including byte-carrier equality.
2. Shape: `evaluate_closed_boolean_expression` (VS:293); macro `lower_boolean_entry!` (VS:319) generating `lower_boolean_expression` (VS:345) and `lower_boolean_guard` (VS:824); `struct BooleanLowering` (VS:352) with `expression` (VS:385) and `guard` (VS:558); `byte_carrier_literal_equality` (VS:600); `byte_carrier_pair_equality` (VS:638); `comparable_byte_carrier` (VS:747); `construct_integer_comparison` (VS:770).
3. I/O: reads expression tables, `CheckedOperatorFacts`, parameter/local rosters. Produces a Boolean tree. `evaluate_closed_boolean_expression` lowers with empty rosters and then calls `crate::values::evaluate_checked_scalar` to fold to `Option<bool>` (VS:299-316).
4. Decisions in `BooleanLowering::expression` (VS:385-556), tried in this order: static type predicate (VS:386-395); primitive reference read (VS:396-403); recast Boolean operand (VS:404-415); case membership (VS:416-420); closed record scalar projection (VS:421-426); constant array projection (VS:427-434); scalar storage read (VS:435-446); then a `match` on node kind (VS:447-555).
   - Names resolve by `parameter_position` (symbol, with spelling fallback, see section 11) and `local_position`.
   - Operators are accepted when `operator_is_builtin` (VS:488, 496, 544).
   - `construct_integer_comparison` (VS:770-822) rejects non-integer types and Addr mixed with other integer types (VS:790-796). `>`/`>=` are lowered by swapping operands (VS:802-809).
5. Cases by shape:
   - `byte_carrier_literal_equality` tries (carrier, literal) in both orders (VS:605) and returns on the first literal it can read (VS:622-627). The extent limit is only known for `FixedArray` and `BoundedView`/`BoundedOwned` (VS:614-621), else `None`.
   - `byte_carrier_pair_equality` unrolls a per-byte comparison chain `0..min(capacity)` only when both sides are fixed arrays or both are `BoundedOwned` (VS:647-661, 699-727). Every other pair becomes a `ByteSequenceEqual` node (VS:662-673).
6. Same job elsewhere: there are four Boolean-algebra walkers over `Not`/`And`/`Or`/`Equal`/comparison, differing by namespace: `BooleanLowering::expression` (VS:385), `crash_entry::Reader::boolean` (VS:3387), `machine_parameter_booleans::structural::lower` (VS:5581), `result_contract::ContractPredicates::boolean` (VS:7602). Each has its own Not/And/Or arm (VS:486-553, 3434-3520, 5645-5747, 7710-7819).
7. Callers (grepped): `evaluate_closed_boolean_expression` has 3 outside call sites (`checks.rs:8735`, `checks_multiplicity_borrowed_windows.rs:795`, `flow_expression.rs:10954`). `construct_integer_comparison` is used inside this file by `machine_parameter_booleans` (VS:5698), `result_contract` (VS:7540, 7566, 7919, 8246, 8251), and `closed_integer_entry_range`.
9. Fail-open: none seen specific to this module beyond `operator_is_builtin` (section 11).

## 4. `call_arguments` (VS:826-1158)

1. Purpose: retains the scalar operands of calls nested inside statements (not the statement's own root call), including array-literal actuals, and decides which nested calls have a structural return type.
2. Shape: `is_scalar_return_call` (VS:854); `retain_nested_call_arguments` (VS:875); `nested_call_sites` (VS:1030); `nested_structural_call_return_type` (VS:1107).
3. I/O: `retain_nested_call_arguments` walks every machine and state (VS:883-884), keeps a running `locals` roster, and appends to `plans.source_bindings`, `plans.expressions`, `proof_terms`; ends with `plans.seal_expressions()` (VS:1021). It `.expect`s that scalar parameters carry primitive types (VS:886-887).
4. Decisions:
   - `nested_call_sites` keeps a call fact only when: its statement index matches; it is not ordinal 0 on the statement's root call; the authored expression is valid; no other fact shares its ordinal or expression; the target entry symbol matches; `selects_only_nominal_route()`; no evidence arguments; receiver presence and receiver symbol agree (VS:1050-1088). Keyed on identity (symbols, ordinals).
   - `nested_structural_call_return_type` (VS:1107): dynamic slot -> slot return type (VS:1118); machine head with positional args and body/bodyless signature (VS:1120-1132); machine-parameter signature owned by the caller (VS:1134-1141); or requirement signature, which must be `is_boundary` (VS:1142-1147). Then `checked_structural_result_type` must succeed (VS:1150-1155).
5. Cases by shape: the return-type function is a closed 3-way split (slot / machine head / signature) as above; anything else returns `None`.
6. Same job elsewhere: `retain_nested_call_arguments` locals-accumulation (VS:1003-1017) repeats the rule "primitive local -> `ScalarLocal::from_local_data`, else reference local -> `primitive_reference_read::local`" that also appears in `local_data::plan` (VS:4878-4966) and `unit_scalar_context` (VS:8879-8894).
7. Callers (grepped): `is_scalar_return_call` 1 outside call (`execution_terminal_unit_control.rs:4469`). `nested_structural_call_return_type` 1 outside call (`execution_terminal_unit_control.rs:6664`). Comment VS:1100-1103 says "One source eligibility judgment feeds pure/computed operands and structural scheduling"; the grep shows exactly one consumer outside the file.
9. Fail-open: none seen; a failed `lower_call_arguments` just skips (VS:979-1001).

## 5. `call_lowering` (VS:1159-2566)

1. Purpose: lowers each call site's arguments into located scalar rows (roles `UnitCallArgument`, `BoundaryCallArgument`, `ErasedUnitCallArgument`, `CallArgument`, `ErasedCallArgument`, `DynamicFamilyArgument`) and proof terms for erased contract-term formals.
2. Shape: `LoweredCallArguments {scalar_arguments, proof_terms}` (VS:1186); `retain_call_arguments` (VS:1191); `call_is_boundary` (VS:1214); `lower_call_arguments` (VS:1245); `family_argument_expression` (VS:1505); `lower_call_scalar_argument` (VS:1552); `lower_erased_scalar_argument` (VS:1602); `lower_proof_term` (VS:1651); helpers VS:1949-2386; `lower_direct_call_binding_arguments` (VS:2389); `value_call_expression` (VS:2521); `scalar_qualified_call_expression` (VS:2538).
3. I/O: reads `validation::call_target_parameters`, `call_site_argument_expressions`, `proof_only_classification`, prepared mathematics (`program.prepared_mathematics()`). Produces rows; `retain_call_arguments` appends a `CheckedScalarExpressionBindings` row (symbols = `located_binding_symbols`) and the expression row for each argument (VS:1200-1211).
4. Decisions:
   - `call_is_boundary` (VS:1214-1241): true when the target is a state of a bodyless machine, a boundary trait requirement, a boundary operator by symbol, or an exact external leaf boundary requirement. Keyed on symbol identity.
   - `lower_call_arguments`: assembly intrinsics are recognised by name: `AsmRegisterOperation::from_intrinsic_name(builtin.name())` (VS:1267-1275), and carriers are mapped back via `PrimitiveType::from_name(carrier.name())` (VS:1292). That is source spelling of a builtin name.
   - Erased parameter handling is skipped entirely for boundary calls (`if !is_boundary` at VS:1328, then `continue` at VS:1394).
   - Non-primitive non-erased formals are skipped (`continue`, VS:1396-1399). Const formals, or mutable formals without a scalar mutable type, make the whole call `None` (VS:1400-1404).
   - `lower_proof_term` order: source constructor equal to expected type (VS:1664-1682); closed or parameterised constructor (VS:1683-1743); source evidence (VS:1744-1753); primitive scalar (VS:1755-1767); then a `match` on node kind.
5. Cases by shape (`lower_proof_term` match, VS:1768-1945): `ArrayLiteral` only for `FixedArray` with literal length and matching element count (VS:1769-1780); `StructLiteral` with valid `type_symbol` (VS:1805-1843); `Borrow` (peeled, VS:1844); `Name` -> erased formal or erased local with initializer (VS:1857-1899); `Member`/`SavedSubject` -> `erased_payload_projection` (VS:1900); `Call` only when the callee is a mathematical definition whose body is exactly one of its own parameters, then the matching actual is lowered (VS:1909-1944); `_ => None`.
   - `erased_payload_projection` follows at most 8 steps (VS:1976) and accepts the projection when the final carrier "corresponds" to the expected type by any of four tests (VS:2030-2041).
   - `lower_direct_call_binding_arguments` (VS:2389) only handles a receiver-less, no-machine-argument call to a machine entry state (VS:2403-2406), and fails if any parameter is const/mutable-nonscalar or `strips_erased_parameter` is `None` (VS:2408-2424).
   - `scalar_qualified_call_expression` (VS:2538) peels only non-recast casts with empty `semantic_domain`, and requires every cast target to equal the callee's primitive return type, an integer other than `Addr` (VS:2547-2560).
6. Same job elsewhere: the proof-symbol matching helpers (`proof_symbols_correspond` VS:2226, `proof_symbol_derives_from` VS:2364 with an 8-step `generated_from` chain, `peel_type_shell` VS:2346 with 8 steps) do type-shell peeling that is repeated in `peeled_indexed_carrier` (VS:10846), `predicate_parameter_type` (VS:10338), `assignment_target_primitive_type` (VS:5522). Grep for `fn peel_type_shell|fn proof_symbol_derives_from|...` found each defined once; the peeling loops are separate bodies.
7. Callers (grepped): `lower_proof_term` is called in-file (VS:1365, 5137 and recursive); no `fn lower_proof_term` elsewhere. `call_is_boundary` in-file at VS:130, 1265. `value_call_expression` in-file at VS:1042, 1044, 4677, 4874. `lower_direct_call_binding_arguments` in-file at VS:4915.
8. Stated limits: VS:1686-1688 "A quantified source schema ... must be specialized before entering either receiving lane" (returns `None` when `level_arity != 0`, VS:1689).
9. Fail-open:
   - `lower_call_arguments` boundary calls with erased formals: the whole erased branch is skipped (VS:1328, 1394); no row, no failure. Stated? no comment at the site. Open.
   - `proof_symbols_correspond` (VS:2226-2234) accepts a pair if either derives from the other via `generated_from` up to 8 hops; feeds `corresponds` at VS:2032-2041, which decides whether a projection is kept. I did not trace what input makes two unrelated generated symbols match. Open.
   - `erased_payload_projection` line VS:2032-2041 treats the carrier as matching if `projection_result_matches` or `projection_instantiated_by_call` holds; the latter scans all machines and states for any call statement that has `expression` as an argument (VS:2073-2093). It does not check that the matching call is in the same state as `expression` other than by the parameter symbol being a parameter of that state. Open.

## 6. `case_membership` (VS:2567-2790)

1. Purpose: lowers `subject == Case` / `subject in Case` into `StructuralCaseMembership`, a tag observation. Three entry points: state parameters (`lower`, VS:2585), requirement signature declarations (`lower_in_declaration`, VS:2621), and result place (`lower_result`, VS:2678).
2. Shape: also `case_identity` (VS:2705) and `structural_case_name` (VS:2771).
4. Decision: `lower` finds the machine/state whose authored parameter roster equals the passed `parameters` slice by value (`authored == parameters`) and by symbol at the position (VS:2593-2602). `case_identity` accepts a right-hand `Name` via `applied_case_membership_variant`, or an empty-field struct literal whose type owns that payload-less variant and matches the subject's data definition (VS:2719-2761). `structural_case_name` searches ALL data definitions for a variant with that symbol (VS:2775-2784).
5. Cases by shape: right operand is `Name` or empty `StructLiteral` with `case_symbol`; anything else `None` (VS:2762).
6. Same job elsewhere: `machine_parameter_booleans::structural::lower_structural_case_membership` (VS:5752-5841) classifies case names against data definitions again with its own `classifier` closure (VS:5781-5818), including the two-way left/right choice (VS:5820-5826); `case_identity` does not allow the case on the left. Read both.
7. Callers: `lower_in_declaration` has 1 call (VS:7645, grepped). `lower_result` called at VS:7655. `structural_case_name` called in-file (VS:2724, 2638, 2760).
9. Fail-open: none seen.

## 7. `constant_array_projection` (VS:2791-2858)

1. Purpose: `CONST[i][j]` over a closed literal constant array resolves to the selected literal leaf handle.
2. Shape: `selected_leaf` (VS:2807).
4. Decision: every index must be an integer literal that is not `Addr`-landed and not non-`Exact` domain (VS:2835-2840), within literal length (VS:2844); the selected leaf must be `Integer`, `Boolean` or `Float` (VS:2852-2855). Comment VS:2795-2797: "Every leaf must be a literal".
5. Cases by shape: only arrays with `FixedArrayLength::Literal` (VS:2826-2832).
7. Callers: used at VS:427 and VS:9172.
9. Fail-open: none seen.

## 8. `contract_entry` (VS:2859-4298)

### 8a. `crash_entry::Reader` (VS:2874-4001)
1. Purpose: reads crash conditions and predicates over a callable's entry parameters (machine entry, current state, signature, operator) into `CheckedBooleanExpression`. A 4096-unit work budget and depth cap 64 bound every walk (`remaining: 4096` at VS:3039, 3071, 3100, 3159, 3273; `charge` VS:3378-3385).
2. Shape: public entries `lower_machine_entry_crash_contract_expression` (VS:3024), `lower_current_state_predicate` (VS:3052), `lower_entry_predicate` (VS:3085), `lower_entry_scalar` (VS:3120), `lower_signature_crash_contract_expression` (VS:3228), `lower_operator_crash_contract_expression` (VS:3288); `struct Reader` (VS:3212); `enum ComparisonOperand {Constant, Subject, IntegerLiteral, Unsupported}` (VS:3322); methods `boolean` (VS:3387), `comparison_operand` (VS:3527), `comparison_telescope` (VS:3603), `integer_comparison` (VS:3615), `float_comparison` (VS:3663), `resolved_subject` (VS:3685), `subject_scalar` (VS:3773), `scalar_position` (VS:3802), `subject` (VS:3813), `parameter` (VS:3861), `field_path` (VS:3880), `primitive` (VS:3921). Nested `logical_matches` (VS:2884-3010) handles `match` on a pure record subject, binding captures during an arm.
3. I/O: reads parameters, optional machine/state, optional `subjects` resolver closure (VS:3078-3079) that maps an expression to authenticated `(parameter position, path)`.
4. Decisions:
   - `primitive` (VS:3921-3981) accepts a primitive only if the type reference is `Named` with a name that `PrimitiveType::from_name` recognises, the symbol has a builtin atom, `symbols.name(symbol) == name`, and the atom maps to the same primitive (VS:3943-3975). This is a spelling check (name string) cross-checked with a symbol atom. Non-primitive named types return `Some(None)` (VS:3945).
   - `parameter` (VS:3861-3878) keys on the single-member name path's spelling AND symbol identity: `parameter.name == spelling && symbols.name(...) == parameter.name && name.symbol == parameter.symbol && name.head_symbol == parameter.symbol`.
   - `comparison_telescope` (VS:3603-3613) rejects the whole predicate if any parameter is const or its symbol's name differs from `parameter.name`, or its type is not readable by `primitive`.
   - `boolean` Binary arm (VS:3442-3520) requires `operator_is_builtin` and, when a machine exists, `has_builtin_binary_expression_meaning`. When `machine` is `None` (declaration contexts), the meaning check at VS:3454 is skipped (`is_none_or`), and an extra check applies only to `Equal`/`NotEqual` in the Boolean-operand path (VS:3490-3508); comparisons route to `lower_integer_contract_comparison`/`lower_float_contract_comparison`, which check builtin meaning themselves (VS:7894, 7947). Traced.
5. Cases by shape: `ComparisonOperand::integer_comparison` (VS:3615-3655) accepts only `Subject`/`Constant` with `integer: true` or an integer literal; comment VS:3636-3639 "This slice establishes no totality for arithmetic, calls, casts, result values, or current body storage." `fixed_integer_atom` (VS:3984) = I8..I64, U8..U64 (no Addr); `ieee_float_atom` (VS:3998) = F32, F64. IEEE comparison admits only static subjects (comment VS:3329-3332).
   - `lower_operator_crash_contract_expression` (VS:3288) returns `None` for any operator with type parameters or lifetime parameters (VS:3294-3299): "A generic operator has no closed telescope yet" (VS:3285-3286).
   - `lower_declaration_crash_contract_expression` returns `None` on invalid or duplicate parameter symbols (VS:3257-3265).
6. Same job elsewhere: `Reader::scalar_position` (VS:3802-3811), `scalar_position_count` (VS:4360), and the inline counter in `lower_callable_parameter_range_requirements` (VS:8078-8084) are three computations of the dense scalar position. `Reader::parameter` (VS:3861), `expression_facts::parameter_position` (VS:4370), `primitive_reference_read::parameter` (VS:6128), `ContractPredicates::subject` (VS:7052) and `structural::lower` Name arm (VS:5610-5622) are five name-to-parameter resolvers keyed differently (symbol only; symbol-or-spelling; symbol plus spelling).
7. Callers (grepped): `lower_current_state_predicate` 1 outside call (`published_facts_boolean_predicates.rs:1564`). `lower_entry_predicate`, `lower_entry_scalar` are re-exported (VS:4004-4007) and consumed via `values.rs:3495-3496`. `lower_machine_entry_crash_contract_expression` unused parameter `_exact_integer_casts` (VS:3029).
8. Stated limits: VS:3636-3639 (above); VS:3285-3286.
9. Fail-open: `Reader::primitive` returns `Some(None)` for non-primitive named types (VS:3945) and `_ => Some(None)` (VS:3977); callers use `??` or compare to `Some(...)`. In `comparison_operand`, any unrecognised node becomes `Unsupported` (VS:3595) and `integer_comparison` then returns `None`. Fail-closed. None seen.

### 8b. `authored_entry_parameters` (VS:4014-4066) and entry rebinding (VS:4068-4297)
1. Purpose: `authored_entry_parameters` returns the entry state's parameter slice only after checking identity consistency (machine symbol kind, exactly one machine with that symbol, attached data symbol matches a unique data definition by symbol and name, entry state's parent, parameters' kinds, parents, uniqueness by symbol AND by name, VS:4018-4064). `lower_machine_entry_boolean_expression` (VS:4068) and `lower_machine_entry_scalar_argument` (VS:4098) lower and then rewrite body-storage reads of `mut` formals to entry positions via `EntryOperands` (VS:4122).
2. Shape: `EntryOperands {program, parameters}` with `position` (VS:4128), `erased_position` (VS:4157), `scalar` (VS:4191), `boolean` (VS:4261), both mutating the tree in place.
4. Decision: `scalar`/`boolean` rewrite `StorageRead` to `Parameter` when `position()` finds exactly one parameter with the symbol that is non-erased and has matching `mutable_scalar_parameter_type`, else to `ErasedParameter`, else `None` (VS:4193-4208, 4263-4271). `Local` and `IntegerMathComparison` return `None` (VS:4236-4238, 4273-4274).
5. Cases by shape: the `scalar` match lists every `CheckedScalarExpression` variant explicitly (VS:4192-4257); `RuntimeIndex` selectors are rebound recursively.
6. Same job elsewhere: `erased_position` (VS:4157-4189) vs `primitive_reference_read::erased_observation` (VS:6166-6200) vs inline erased counting in `ContractPredicates::subject` (VS:7073-7082): three computations of the dense erased index.
7. Callers (grepped): `authored_entry_parameters` has 1 outside call (`published_facts.rs:3047`) plus 4 in-file (VS:3031, 3093, 3158, 6721). The comment at VS:4009-4012 says scalar contracts and structural crash predicates "share exact authored ... identity" through it; that matches the in-file use. `lower_machine_entry_boolean_expression` has 3 outside calls (`published_facts.rs:4057, 4131, 4300`). `lower_machine_entry_scalar_argument` 1 outside call (`published_facts.rs:4067`).
9. Fail-open: none seen (all `?`).

## 9. `expression_facts` (VS:4299-4503)

1. Purpose: small shared facts: scalar position, parameter/local lookup, zero initializer, builtin-operator test, arithmetic domain combination.
2. Shape: `occupies_scalar_position` (VS:4324), `scalar_parameter_roster` (VS:4340), `scalar_position_count` (VS:4360), `parameter_position` (VS:4370), `local_position` (VS:4395), `scalar_expression_type` (VS:4424), `zeroed_scalar_local_initializer` (VS:4434), `is_integer` (VS:4473; true for everything except Bool/F32/F64, so it includes `Addr` and `Selector`), `operator_is_builtin` (VS:4480), `combine_arithmetic_domains` (VS:4491), re-export `checked_integer_binary_kind` (VS:4502).
4. Decisions:
   - `occupies_scalar_position` (VS:4324-4332): not erased AND primitive type. Keyed on relevance and type.
   - `parameter_position` (VS:4370-4393): single-member path; if `path.symbol` valid compare symbol; else if `head_symbol` valid compare head symbol; else compare `parameter.name` to the spelling (VS:4383-4389).
   - `local_position` (VS:4395-4422): mutable locals match by symbol only (VS:4408-4411); immutable by `path.symbol`, else `head_symbol`, else `local.name == expression_table.display_name(expression)` (VS:4417). It scans `rposition`, so the latest local wins.
   - `combine_arithmetic_domains` (VS:4491-4500): Exact yields the other; equal yields the same; different non-Exact returns `None`.
   - `zeroed_scalar_local_initializer` (VS:4434): a declared local with no initializer and plain scalar storage gets a literal 0 / false / 0.0 (VS:4439-4470).
5. Cases by shape: zero initializer is a match on `Bool`/`F32`/`F64`/other (VS:4445-4462).
7. Single-source claims (comments): VS:4335-4339 says "Every lowerer that reads the dense scalar namespace takes its roster from here rather than rebuilding the filter." Grepped `scalar_parameter_roster` across `omega-rust`: 12 occurrences, all in `values_scalar.rs` (counted with `output_mode: count`; one file). Outside this file the same filter is rebuilt: grep for `occupies_scalar_position(` shows `.filter(... occupies_scalar_position ...)` at `values.rs:117`, `published_facts_crash_calls.rs:2207, 3937, 4059`, `published_facts_boolean_predicates.rs:161, 170, 1464`, `execution_terminal_unit_types.rs:1836`, `execution_terminal_unit_structural_scalar_store.rs:3224`, `execution_scalar_computations.rs:10534` (10 filter sites). I did not read those sites.
   `operator_is_builtin(` call count: 39 across 8 files (grepped, `count` mode): 18 in this file, 9 in `execution_scalar_computations.rs`, 4 `flow_transfers.rs`, 3 `flow_exits.rs`, 2 `flow_expression.rs`, 1 each in `execution_terminal_unit_structural_scalar_store.rs`, `checks_ranges_facts.rs`, `checks_contracts_writes.rs`. (The count includes the definition and one call-style mention per `fn` signature line.)
9. Fail-open:
   - `operator_is_builtin` (VS:4480-4489): `expression_use(expression).is_none_or(status == BuiltinFallback)`, so an expression with NO recorded operator use is treated as builtin. A wrong program that would get through: a binary or unary expression whose user-defined operator resolution was never recorded in `CheckedOperatorFacts`. Open (not traced; some callers add an independent meaning check, e.g. VS:3454, 7894; the lowerers at VS:488, 496, 544, 9333 rely on it alone).
   - `parameter_position`/`local_position` fall back to spelling when no symbol is attached (VS:4388, 4417): an unresolved name that spells a parameter or local name would resolve to it. Open.

## 10. `expression_plans` (VS:4504-5539)

1. Purpose: walks every state's statements once and builds `CheckedScalarExpressionPlans` (located expressions, source bindings, binding symbols, proof terms). Each statement kind has its own private planner module.
2. Shape: `StatementPlanner` (VS:5281-5300) with `retain_expression_call_arguments` (VS:5309) and `record_scalar_value_pair` (VS:5343). `ScalarLocal {is_mutable, symbol, name: String, primitive_type, arithmetic_domain, is_reference}` (VS:5211-5222), `from_local_data` (VS:5229), `is_value_binding` (VS:5252 = `!is_mutable && !is_reference`), `located_binding_symbols` (VS:5261). Planner mods: `array_elements::plan` (VS:4545), `assignment::plan` (VS:4618), `call_statement::plan` (VS:4730), `expression_statement::plan` (VS:4767), `local_data::plan` (VS:4819), `range_operands::plan` (VS:4979), `transition::plan` (VS:5039). Entry `build_checked_scalar_expression_plans` (VS:5370); `retain_subslice_endpoints` (VS:5459); `lower_closed_integer_literal_guard` (VS:5492); `assignment_target_primitive_type` (VS:5522).
3. I/O: reads the whole program; writes `expressions`, `source_bindings`, `binding_symbols`, `proof_terms`.
4. Decisions:
   - `build_checked_scalar_expression_plans` ends with a filter (VS:5434-5445): for each source binding, `semantic_casts::requires_custody` true removes the matching located expression row (same state, ordinal, role) and drops the binding; otherwise the binding is kept. Header comment VS:2-6 says semantic-cast bindings "belong to the graph". The retain call inside the loop makes this O(bindings x expressions).
   - Statement kind dispatch (VS:5417-5430): LocalData, Expression, Assignment, Call, Transition; `_ => {}` (other statement kinds produce nothing).
   - `local_data::plan` (VS:4819): atomic-once carrier first (VS:4820-4843); selected-operator operands (VS:4844-4867); value-call args (VS:4868-4877); a local with a slice-view result type and non-mutable skips the scalar row (VS:4878-4891); role is `StorageInitializer` (mutable) or `LocalInitializer {binding_ordinal}` (VS:4909-4913); a direct entry-state call binding is tried first (VS:4914-4928), else the zero initializer or `lower_return_expression` (VS:4938-4961).
   - `transition::plan` (VS:5039-5207): per successor argument, target type is erased scalar observation or primitive; an erased proof-carrier parameter takes `lower_proof_term` (VS:5127-5167); any other non-scalar formal is skipped (`continue`, VS:5169-5172).
   - `assignment_target_primitive_type` (VS:5522-5538) peels Constrained and ONE Reference level (`crossed_reference`), a second reference returns `None` (VS:5534).
   - `lower_closed_integer_literal_guard` (VS:5492): both sides integer literals -> constant via `evaluate_comparison` on bignums (VS:5514-5519).
5. Cases by shape: `transition::plan` has a dead branch: `if continuation { continue; } continue;` (VS:5169-5172), both arms identical. `array_elements::plan` destination is a 3-way match: immutable local, expression statement (state return type), assignment with closed array store type (VS:4546-4564).
6. Same job elsewhere: see section 4 item 6 (local-roster rule repeated).
7. Callers (grepped): `build_checked_scalar_expression_plans` re-exported (VS:11462-11464) and in `values.rs:3493`. `lower_closed_integer_literal_guard` is used at VS:560.
8. Stated limits: none in this module.
9. Fail-open:
   - `semantic_casts::requires_custody` (VS:10041-10060) uses `result_type(...)` `.is_some_and(...)`, so when the operand's result type cannot be resolved (`find_state` fails or `expression_result_type_reference` returns `None`) the cast counts as NOT needing custody and its pure plan row is kept (VS:5442-5443). A wrong program: a cast from a declared-domain value whose type lookup fails would keep a payload-only plan row. Open (not traced).
   - Statement kinds outside the five matched are ignored by design (VS:5429).
   - Failed lowerings are skipped with `return`/`continue` all through the planners, so a statement can end with no scalar row; the consumer is outside this file. Open.

## 11. `machine_parameter_booleans` (VS:5540-6076)

1. Purpose: lowers contract predicates in a machine's entry-parameter namespace: first `structural::lower` (Boolean fields, case membership, numeric comparisons over structural reads), else the shared `lower_boolean_expression` over dense scalar parameters.
2. Shape: `structural::lower` (VS:5581), `lower_structural_case_membership` (VS:5752); `structural_paths::{path_type_reference (VS:5868), path_primitive_type (VS:5956), field_type (VS:5854)}`; `lower_machine_parameter_boolean_expression` (VS:5982), `lower_parameter_boolean_expression` (VS:6019), `lower_parameter_scalar_expression` (VS:6050).
4. Decisions:
   - `structural::lower` Name arm (VS:5610-5644): resolves the name by symbol OR by text: `parameters.iter().position(|p| name_symbol == p.symbol || name_text == p.name)` (VS:5619-5622). Erased -> `None`. Mutable -> `StorageRead` only if symbols match. Otherwise a Bool parameter becomes `Parameter {position: scalar_position_count(...)}`.
   - Comparison arm (VS:5657-5728): floats through `lower_float_contract_comparison` with result types from `parameter_expression_result_type_reference`; `Addr` rejected (VS:5674-5678); else `construct_integer_comparison`; Equal/NotEqual fall back to Boolean-of-Boolean equality (VS:5707-5727).
   - `lower_machine_parameter_boolean_expression` (VS:5982-6012): if every parameter has a primitive type (VS:5997-6000), lowers with the scalar roster; a machine with any non-primitive parameter only gets the structural result.
   - `lower_parameter_*` require `has_builtin_parameter_bound_expression_meaning` (VS:6027, 6058).
5. Cases by shape: `path_type_reference` (VS:5868-5954) walks `Referent`, `FixedIndex`, `RuntimeIndex`, `Case`, `Field` and enforces "case must be followed by a field" via `selected_case`; it does NOT check erased relevance or symbol validity of fields (compare `resolve_structural_path`, VS:10723).
6. Same job elsewhere: `path_type_reference` (VS:5868) and `resolve_structural_path` (VS:10634-10741) both walk the same path-segment list to a type; they differ in checks (the latter checks `field.symbol.is_valid()`, erased relevance, and returns place segments). Suspected duplication; I read both. Also `lower_structural_case_membership` vs `case_identity` (section 6).
7. Callers: `lower_machine_parameter_boolean_expression` used at VS:4075; `lower_parameter_boolean_expression`, `lower_parameter_scalar_expression` re-exported (VS:11465-11468). `lower_parameter_scalar_expression` has 1 outside call (`checks_contracts_prover.rs:6490`, grepped).
9. Fail-open: the name-by-text fallback at VS:5619-5622 matches a name with no resolved symbol to any parameter with the same spelling. Open (not traced).

## 12. `primitive_reference_read` (VS:6077-6247)

1. Purpose: whole readable primitive referents (`&u32` formals, reference locals) are storage reads, not scalar formals. Also exact erased-formal observation.
2. Shape: `lower` (VS:6089), `declared` (VS:6104), `parameter` (VS:6128), `erased_observation` (VS:6166), `local` (VS:6207), `readable_type` (VS:6222).
4. Decisions: `declared` requires a single-member name whose first member symbol (if any) equals `name.symbol` (VS:6113-6120). `parameter` needs a unique, non-const match by symbol (VS:6136-6142). `readable_type` accepts `Shared` or `Mutable` references only (not write-only), to a primitive that is not `Addr` (VS:6226-6245). Bool becomes `Boolean(StorageRead)`, others `StorageRead` (VS:6147-6156).
5. Cases by shape: only non-`Addr` primitives behind `&`/`&mut`.
6. Same job elsewhere: see section 8b item 6 (erased index).
7. Callers: `local` used at VS:1013, 4894, 8884. `lower` at VS:396.
9. Fail-open: none seen.

## 13. `result_contract` (VS:6248-8277)

1. Purpose: lowers scalar contract predicates (`requires`, `ensures`, state arrival requires, membership facts, parameter ranges and domains) into the closed scalar contract namespace: entry formals by dense position, the result at position `scalar_position_count(parameters)`, structural leaves by authored formal position.
2. Shape: `ContractPredicates` (VS:6816, fields `program, operators, owner, result_type, parameters, allow_result, remaining: &Cell<usize>, logical_captures`); `ContractBudget` (VS:6796) with `Drop` writing the remaining budget back (VS:6810-6814). Entries: `lower_scalar_contract_predicate` (VS:6713), `lower_state_scalar_contract_predicate` (VS:6741), `lower_callable_scalar_contract_predicate` (VS:6768), `lower_callable_scalar_membership` (VS:6832), `lower_callable_scalar_membership_value` (VS:6850). Methods: `selector_term` (VS:6897), `structural_observation` (VS:6922), `subject` (VS:7052), `numeric_term` (VS:7109), `integer_math_expression`/`integer_math_term` (VS:7323, 7333), `boolean_type` (VS:7405), `numeric_subject` (VS:7427), `projected_scalar_subject` (VS:7440), `exact_predicate` (VS:7523), `boolean` (VS:7602). Free: `contract_storage_root` (VS:7832), `lower_integer_contract_comparison` (VS:7859), `lower_float_contract_comparison` (VS:7928), `lower_contract_integer_literal` (VS:7985), `ParameterRangeRequirements` (VS:8011), `lower_scalar_parameter_range_requirements` (VS:8049), `lower_callable_parameter_range_requirements` (VS:8068), `lower_state_domain_predicate` (VS:8183), `closed_integer_entry_range` (VS:8213), `lower_scalar_parameter_refinement_predicates` (VS:8271). Nested `logical_matches` (VS:6255-6503), `predicate_conditions` (VS:6504-6676).
3. I/O: reads typed program, prepared mathematics (`prepared_mathematics()`, `application`), predicate-explorer results (`exact_predicate_ordering`, `exact_predicate_conjunction`, `exact_predicate_intervals`). Produces Boolean trees, `ParameterRangeRequirements`.
4. Decisions:
   - `boolean` (VS:7602-7822) order: static boolean normal form and type predicate (VS:7607-7617); mathematical application -> `scalar_predicate` (VS:7629-7641); case membership by parameters / declaration / result (VS:7642-7661); `exact_predicate` (VS:7662); structural Boolean observation (VS:7665-7673); then a node-kind `match` (VS:7674-7821): Match, saved subject / member / indexed, Boolean literal, Call (only byte predicates via `byte_predicate_call`, VS:7689-7700), Name (Bool parameter by position, erased or not), Not, Binary.
   - Binary arm (VS:7718-7819): float comparison first; then `operator_is_builtin` (VS:7729); proof-integer operands -> `IntegerMathComparison` (VS:7732-7771); then `lower_integer_contract_comparison`; then And/Or/Equal/NotEqual on Booleans with `has_builtin_spelled_expression_meaning` over the Boolean carrier types (VS:7792-7816).
   - `numeric_term` (VS:7109-7320) accepts: constant recipe (VS:7115-7155), Match, Float literal, Name, Binary Add/Subtract/Multiply (VS:7194-7268), Member/Indexed sequence length or observation, exact non-recast casts that are total widenings (VS:7299-7318). Trapping arithmetic is refused (VS:7255-7259). Comment VS:7099-7108 says anything else "stays outside the closed language".
   - `contract_storage_root` (VS:7832-7853): shared reference accepted unless `allow_result && parameter.is_mutable`; `&mut` accepted; other reference access rejected; any non-reference parameter accepted only when `!allow_result`. Comment VS:7825-7831 explains the phase distinction (requires vs normal guarantees).
   - `lower_integer_contract_comparison` (VS:7859): any operator use that is not `BuiltinFallback` rejects (VS:7867-7872); contextual type from a typed subject or landed literal (VS:7889); requires `has_builtin_spelled_expression_meaning` (VS:7894).
   - `lower_float_contract_comparison` (VS:7928): both sides must be the same float primitive and `has_builtin_or_sealed_float_comparison_meaning`; `>`/`>=` swap operands (VS:7962-7969).
   - `lower_callable_parameter_range_requirements` (VS:8068-8145): per parameter it peels references and constraints, and for each `TypeConstraintNode::Domain`: `Value` subject -> predicate via `scalar_value_domain_predicate` (VS:8094-8115); other domain -> exact interval via `exact_declared_domain_carrier_interval`, only for integer primitives (VS:8116-8135).
5. Cases by shape: `numeric_term` Binary arm admits only Add, Subtract, Multiply (VS:7197-7202). `integer_math_term` admits Add/Subtract/Multiply, builtin type members, embedding arguments, and anonymous literals (VS:7350-7401). `closed_integer_entry_range` only for `Exact` arithmetic domain integers and `low <= high` (VS:8223-8231).
6. Same job elsewhere: `ContractPredicates::boolean` is a fourth Boolean walker (section 3 item 6). `logical_matches::logical_subject` (VS:6315-6384) and `crash_entry::logical_matches::record_binding` (VS:2919-2936) both handle record `match` captures with separate implementations over `validation::logical_record_match_capture`. `structural_observation` result-place projection (VS:6933-6981) and `logical_subject` result-place projection (VS:7337-7367, 6337-6367) repeat the same `reference_shell`/`project_type_reference_from_segments` loop (3 copies: VS:6347-6358, 6943-6955, 7458-7462 uses `owned_projection_type_reference` instead).
7. Callers (grepped): `lower_callable_scalar_membership` 3 outside calls (`execution_terminal_unit_control.rs:142`, `execution_terminal_unit_types.rs:313`, `published_facts_boolean_predicates.rs:1847`); `_value` 1 outside (`execution_terminal_unit_control.rs:140`). `lower_state_domain_predicate` 1 outside (`execution_terminal_unit_state_graph.rs:9731`). `lower_scalar_parameter_refinement_predicates` 1 outside (`published_facts.rs:4089`). `lower_callable_parameter_range_requirements` 1 outside (`published_facts.rs:3262`). `lower_scalar_parameter_range_requirements` (VS:8049) has 1 call site (VS:8275) and is not re-exported (not in VS:11469-11473).
8. Stated limits: VS:7099-7108 (Trapping arithmetic "stays outside the closed language"); VS:8020 "Legacy floating ranges retain `None` because their separate endpoint roster has not joined this predicate owner"; VS:8268-8269 "Legacy floating interval rows keep an unsupported slot until their endpoint owner joins".
9. Fail-open:
   - `lower_callable_parameter_range_requirements`, VS:8119-8135: a non-`Value` `Domain` constraint on a parameter that is not an integer primitive, or whose exact interval is not derivable, hits `continue` with NO slot pushed to `scalar_predicates`/`scalar_clauses` and `integer_entry_ranges` NOT voided. The struct doc says "one slot per authored constraint" (VS:8013-8014) and `None` marks a range that "could not be retained" (VS:8023-8024). So a non-interval domain constraint produces no slot, and the roster still looks complete. What gets through: a parameter constraint the plan cannot express yields no row and no incomplete marker. Open (I did not trace consumers; semantic-atom domains may be handled by a separate nominal path).
   - `lower_callable_scalar_membership_value` (VS:6880-6882): `predicates.next().unwrap_or(Constant(true))`: an empty predicate list yields `true`. Open (a comment at VS:6846-6849 says this is a numeric projection that "must never stand in for the nominal admission obligation").
   - `contract_storage_root` `_ => return !allow_result` (VS:7850): stated, VS:7825-7831.
   - `exact_predicate` (VS:7523-7600): when no relation matches, an `exact_predicate_intervals` row list that is empty yields `Constant(true)` (VS:7552 starting value with no iteration). Whether `exact_predicate_intervals` can return an empty `Some` is outside this unit. Open.

## 14. `scalar_lowering` (VS:8278-9843)

### 14a. `element_selection` (VS:8301-8692)
1. Purpose: classify a storage read (field/index chain) into `StructuralParameterField`, `StructuralParameterIndexedRead`, or erased observation, keeping each runtime selector's value and occurrence.
2. Shape: `ScalarStorageProjection {root, path, selected_type}` (VS:8326); `scalar_storage_read` (VS:8340); `scalar_storage_read_with_selectors` (VS:8375); `scalar_storage_read_projection` (VS:8400); `storage_projection` (VS:8519); `pure_selector` (VS:8633); `landed_integer_literal_fits` (VS:8664).
4. Decisions: `storage_projection` peels members and indexes from the expression until `structural_storage_place` succeeds on a prefix with no FixedIndex/RuntimeIndex segment (VS:8539-8552); range indexes return `None` (VS:8555-8560); each index requires `indexed_read_is_builtin` and a selector value with integer, non-`Addr` carrier (VS:8596-8610). `scalar_storage_read_projection` accepts integer, Bool, F32/F64 element carriers only (VS:8462-8466, 8498-8505); `landed_integer_literal_fits` tests bit-width bounds by `is_signed`, `bit_width` (VS:8668-8684).
6. Same job elsewhere: literal-fit logic in `landed_integer_literal_fits` (VS:8664) is used by `retag_exact_integer_literal` (VS:9579) and `land_contextual_integer_literal` (VS:9596); `closed_integer_entry_range` uses `validation::land_integer_value` (VS:8233) instead. Two landing routes.
7. Callers: `scalar_storage_read` at VS:435, 9116, 9203. `pure_selector` at VS:9186.
9. Fail-open: none seen.

### 14b. `entry_lowering` (VS:8693-9357)
1. Purpose: the core scalar lowerer `lower_scalar_expression` (VS:9089-9356) plus entry points for return values (`lower_return_expression`, VS:8959), index values (`lower_index_expression`, VS:8929), unit-state arguments (`lower_unit_scalar_argument`, VS:8780, `..._for_places`, VS:8809, `unit_scalar_context`, VS:8866, `unit_scalar_binding_symbols`, VS:8903), state scalar expression (VS:8734), operand pairs (`lower_scalar_operands`, VS:9035) and anonymous landing (`land_anonymous_scalar_expression`, VS:9005).
4. Decisions in `lower_scalar_expression` (tried in order): recast view operand (VS:9113-9135); Float literal (needs a landing, VS:9136-9143); primitive reference read (VS:9144); closed record scalar projection (VS:9149-9171); constant array projection (VS:9172-9179); structural sequence length (VS:9180-9199); inline literal subslice length (VS:9200-9202); scalar storage read (VS:9203-9214); then `match` on node: `Name` (VS:9216-9284: dense parameter, mutable -> `StorageRead`, erased observation, local), `Integer` (VS:9285), `Cast` (non-recast, no semantic domain: `Selector` target -> `ImplementationSelection` (VS:9298-9308); float target requires same-type operand (VS:9309-9313); integer target via `construct_integer_cast` (VS:9314-9324)), `Unary` BitwiseNot (VS:9326), `Binary` (builtin: `construct_integer_binary`, VS:9333-9352), `_ => None`.
   - `lower_return_expression` (VS:8959-9003): anonymous landing first; Bool via `lower_boolean_expression`; else `lower_scalar_expression` and the type must equal the result type, or an untyped literal is landed.
   - `lower_state_scalar_expression` (VS:8734-8773): only when the state's prefix before the statement consists solely of immutable local-data statements (`statement.as_local_data()?`, VS:8752), and all parameters have primitive types (VS:8744-8747). Comment VS:8730-8733: "any wider state shape stays explicit as `None`".
   - `lower_unit_scalar_argument_for_places` (VS:8809): lowers, then rewrites each operand through `map_scalar` into a canonical place and calls a binder closure `bind`; the bound result's primitive type must equal the operand's (VS:8856-8860).
5. Cases by shape: `Cast` arm: Selector / F32|F64 / integer; any other target returns `None` (VS:9314-9316).
6. Same job elsewhere: `unit_scalar_context` (VS:8866) builds locals from prefix statements, skipping non-local statements (VS:8879-8881) whereas `lower_state_scalar_expression` rejects non-local prefix statements (VS:8752). Two different prefix rules for the same job ("scalar namespace before statement N").
7. Callers (grepped): `lower_state_scalar_expression` 1 outside (`published_facts_crash_calls.rs:1727`). `unit_scalar_binding_symbols` 4 outside (`checks_contracts_prover.rs:6011`, `execution_terminal_unit_structural_scalar_store.rs:2213`, `flow_expression.rs:5609`, `published_facts_boolean_predicates.rs:1685`).
9. Fail-open: `lower_return_expression` (VS:8999-9002): when the lowered expression has no primitive type (untyped literal) it is landed at the result type via `land_contextual_integer_literal`; this is the intended contextual landing. `land_anonymous_scalar_expression` (VS:9024-9031): when `operators.expression_use` is `None` it falls back to `has_anonymous_operator_meaning` (VS:9029), not to "accept". None seen.

### 14c. `integers` (VS:9358-9604)
1. Purpose: builds checked integer binary ops, bitwise-not, casts (exact, widening, retag), literal landing, inline literal subslice length.
4. Decisions: `construct_integer_binary` (VS:9380-9438): domain is left's for shifts, else `combine_arithmetic_domains`; kind from `checked_integer_binary_kind`; an anonymous `0` on the left of `-` lands as the right's type (VS:9401-9404); `Addr` with non-Exact domain is rejected (VS:9422-9426). `construct_integer_cast` (VS:9459-9517): same type -> operand; literal retag; total widen; or a matching `ExactIntegerCastFact` found by `expression` handle and matching source/target types (VS:9497-9515), else `None`. `exact_inline_literal_subslice_length` (VS:9525): both bounds integer literals, length as u64.
7. Callers: `construct_integer_binary` at VS:7246, 9344.
9. Fail-open: none seen. Note `exact_integer_casts` lookup is by linear `iter().find` per cast (VS:9497-9499).

### 14d. `recast_reads` (VS:9605-9842)
1. Purpose: `&x as &T` re-views of a whole scalar place as another primitive carrier (`RepresentationView`).
4. Decisions: `recast_view_operand` (VS:9630) peels parentheses and requires the cast form to be a recast and the source to be Name/Member/Indexed. `view_place_read` (VS:9728-9774): same-carrier is the read itself; a Bool view of a Boolean read stays; otherwise widths must match (`scalar_byte_width`, VS:9777: Bool/U8/I8=1, U16/I16=2, U32/I32/F32=4, U64/I64/F64=8, `Addr`/`Selector` -> `None`) unless the source is a runtime-indexed `u8` element, which may view a region of any non-Bool width (VS:9732-9739, 9764-9769).
5. Cases by shape: comment VS:9713-9727 describes the first-byte-of-a-byte-region case; "its native replay is still owed a Boolean re-view form" (VS:9718-9719).
8. Stated limits: VS:9718-9719 "its native replay is still owed a Boolean re-view form".
9. Fail-open: `view_place_read` returns `Some(read)` unchanged when `primitive_type == Bool` and the read is already a `Boolean` expression (VS:9752-9756) regardless of the source carrier of the recast. The Boolean case: a recast of a non-Bool place to `bool` is handled in `lower_recast_boolean_operand` (VS:9813-9841), which rejects a non-Bool read (VS:9837). In `lower_scalar_expression` the recast path (VS:9113-9135) calls `view_place_read` on whatever the source reads as; if a Boolean-typed source were re-viewed as Bool it passes. Not a wrong-program risk I can name. None seen.

## 15. `selected_operator_operands` (VS:9844-9937)

1. Purpose: for a non-mutable `let` whose whole initializer applies a boundary operator, lowers each scalar operand under `SelectedOperatorOperand {operand_ordinal}` before settlement rewrites the operator node.
2. Shape: `lower_selected_operator_operands` (VS:9876).
4. Decision: asks `operators.boundary_application_operands(program, local.initial_value, origin)` with origin `StateStatement{..., role: LocalInitializer}` (VS:9895-9903); each operand lowered at the operand's primitive type, skipping those with `None` type (`(*primitive_type)?`, VS:9920).
7. Callers: used at VS:4846 only (grepped by name).
9. Fail-open: none seen.

## 16. `semantic_casts` (VS:9938-10085)

1. Purpose: decide when a cast needs "qualification custody" (the cast changes declared meaning), independent of the payload grammar.
2. Shape: `result_type` (VS:9949), `has_declared_domains` (VS:9965), `is_interval_domain` (VS:9989), `has_only_vacuous_tags` (VS:10003), `requires_custody` (VS:10041), `same_carrier_operand` (VS:10067).
4. Decisions: `has_declared_domains` is true if any `Domain` constraint with subject `Declared` is not an exact integer interval (VS:9969-9984); recurses through `base_type` but not through references. `requires_custody` (VS:10041-10060) walks the expression's structural subtree for a `Cast` with non-empty `semantic_domain` or whose operand's result type has declared domains. `same_carrier_operand` (VS:10067-10084) returns the cast value only for non-recast, no semantic domain, no declared domains on target or operand, and equal primitive carriers.
7. Callers (grepped): `requires_custody` is called at VS:5436 and at `execution_scalar_computations.rs:4384, 9615, 10059, 10103`. `same_carrier_operand`: `execution_terminal_unit_state_graph.rs:709`, `execution_scalar_computations.rs:9858`. `has_only_vacuous_tags`: 1 outside call (`execution_scalar_computations.rs:10000`). `result_type`: many calls in `execution_scalar_computations.rs`.
9. Fail-open: see section 10: `requires_custody` returns false when `result_type` is `None`. Open.

## 17. `structural_fields` (VS:10086-11249)

1. Purpose: resolves observations of structural storage (parameters, owned locals, view locals) to `(root, projection path, selected type)` using canonical places and declaration walks, plus length observation and path translation between place segments and checked predicate path segments.
2. Shape: `structural_sequence_length` (VS:10137), `structural_sequence_receiver_length` (VS:10156), `view_local_root` (VS:10210), `owned_local_root` (VS:10234), `indexed_read_is_builtin` (VS:10258), `indexed_read_has_builtin_meaning` (VS:10282), `predicate_parameter_type` (VS:10338), `predicate_referent_path` (VS:10371), `structural_parameter_field_path` (VS:10401), `structural_parameter_case_path` (VS:10410), `explicit_observation_path` (VS:10421), `parameter_projection` (VS:10452), `resolve_structural_parameter_path` (VS:10518), `readable_entry_parameter_path` (VS:10551), `resolve_structural_path` (VS:10634), `exclusive_reference` (VS:10747), `storage_observation_is_frozen` (VS:10768), `peeled_indexed_carrier` (VS:10846), `fixed_index_element_type` (VS:10866), `runtime_index_element_type` (VS:10896), `structural_data` (VS:10914), `structural_parameter_place` (VS:10945), `structural_storage_place` (VS:10967), `applied_projection_segments` (VS:11109), `structural_projection_path` (VS:11122), `member_projection` (VS:11176), `whole_local_binding` (VS:11220).
3. I/O: reads data definitions by index, `canonical_place_from_expression`, symbol tables. Produces `CheckedStorageRoot` (`Parameter {index}`, `ViewLocal {symbol}`, `OwnedLocal {symbol}`) and path vectors.
4. Decisions:
   - `structural_storage_place` (VS:10967-11102): the canonical place root must be a symbol (VS:11039); name must have a valid symbol and head symbol; for a single-member name, `symbol == head_symbol` (VS:11049-11052); a `self` spelling with no matching parameter is rejected (VS:11042-11048, spelling test via `is_self_receiver()`); parameter root requires a unique parameter with the head symbol, kind `Parameter`, and `place.root == Symbol(parameter.symbol)` (VS:11055-11065); otherwise a whole local binding (view or owned). The resolved `segments` must equal `place.segments` exactly (VS:11092-11094). Member selection compares the retained `member_symbol` against the place's field symbol; on mismatch it tries `closed_data_member_field` and accepts if that field's symbol equals the selected one (VS:11013-11035).
   - `whole_local_binding` (VS:11220-11248): `local_data_positions_for_symbol(path.symbol)` must give exactly one position; indexes machines/states/statements by those positions (VS:11236-11246).
   - `structural_data` (VS:10914-10942): for a `Named` or `Generic` reference tries `closed_data_definition`, else by symbol, else by NAME: `data_definition_positions_for_name(name.as_str()).first()` when the symbol is invalid (VS:10938-10941). Spelling-keyed lookup, takes the first match.
   - `resolve_structural_path` (VS:10634-10741): segments Referent, FixedIndex, Case, Field; `RuntimeIndex` returns `None` (VS:10731). Fields must have valid symbol and not be erased (VS:10723). `frozen` flag: root not mutable and no exclusive reference crossing (VS:10645-10646, 10733-10735).
   - `readable_entry_parameter_path` (VS:10551-10627): rejects const / erased parameters and paths of length 64 or more; per step loops up to 64 peeling Constrained/readable References, decrementing `remaining`; `FixedIndex` requires `FixedArray` with `Literal` length (VS:10593-10604); `Field`/`Case` require a closed data definition of kind `Data` (VS:10605-10618).
   - `structural_sequence_receiver_length` (VS:10156): fixed array -> literal length (refused if any runtime selector contains a trapping operation, VS:10170-10177); else the carrier must be `BoundedOwned` or `BorrowedView` byte sequence, or a borrowed slice view element (VS:10181-10196).
   - `fixed_index_element_type` (VS:10866): `FixedArray` with literal length (index in range) or `ConstCall` length with a constant recipe; `Slice`; else `None`. `peeled_indexed_carrier` returns `None` for byte carriers (comment VS:10839-10845).
   - `storage_observation_is_frozen` (VS:10768-10837): root symbol must be an immutable parameter or an earlier immutable local; every projection step must cross only shared references; and `has_contents(..., StableObservation)`.
5. Cases by shape: `indexed_read_has_builtin_meaning` (VS:10282-10332) has two shapes for finding the owner: parameters belong to a known state (via `state_position_for_parameter_symbols`), or all parameters share one parent scope; a scope of kind `State` must be a trait-machine signature whose parameter slice equals `parameters` (VS:10314-10322). Anything else returns false.
   - `owned_local_root` (VS:10234-10252): owned means `FixedArray` or a closed data definition; `Reference`/`Slice` are not.
6. Same job elsewhere: `path_type_reference` (VS:5868) duplicates `resolve_structural_path` (section 11). Reference-shell peeling is written at VS:5887-5901, 10246, 10656-10660, 10752-10758, 10779-10791 (each a distinct loop; grep of `fn exclusive_reference|fn shared_reference_chain|fn predicate_parameter_type` found one definition each).
7. Callers (grepped): `storage_observation_is_frozen` 1 outside call (`checks_contracts_writes.rs:792`, import `:353`). `view_local_root` and `owned_local_root` plus `scalar_byte_width`, `view_place_read`, `place_read_site`, `recast_view_operand`, `lower_recast_boolean_operand`, `exact_inline_literal_subslice_length`: 23 occurrences across 2 files (21 in this file, 2 in `execution_terminal_unit_structural_scalar_store.rs`; counted with `output_mode: count`, includes definitions).
8. Stated limits: VS:10547-10549 "Static selectors still require fixed arrays, so no dynamic bounds obligation hides inside this declaration walk."
9. Fail-open:
   - `structural_data` name fallback (VS:10938-10941): with an invalid symbol it picks the first data definition with that spelling. What gets through: a type reference with no symbol and a name shared by two data definitions resolves to the first. Open.
   - `peeled_indexed_carrier` returns `None` on byte carriers so they "select no static element" (stated at VS:10839-10845).
   - Other paths return `None` on any missing step: none seen.

## 18. `subslice_endpoints` (VS:11250-11446)

1. Purpose: lowers present endpoints of a builtin range used to narrow a view at any site (let initializer, call argument, transition argument, any expression), under roles `SubsliceStart`/`SubsliceEnd {site}`; and enumerates the sites.
2. Shape: `subslice_endpoints` (VS:11277, generic over the lowered `Value`), `sites` (VS:11322), `structural_call_arguments` (VS:11409).
4. Decision: operator use for the range expression must be Missing/BuiltinFallback with spelling `Range`, no selected operator symbol, zero candidates (VS:11293-11307). Omitted endpoints produce no row (VS:11313). `sites` collects: local initial value (VS:11330-11332); every call site's structural (non-scalar, non-erased) arguments with their dense structural ordinal (VS:11333-11355; `TransitionNamed` sites whose target has no terminal callable are skipped, VS:11341-11345); transition arguments for non-erased non-primitive formals (VS:11356-11376); and every expression in the statement's expression roots (traversed with `visited: Vec` linear contains check, VS:11377-11401).
5. Cases by shape: `structural_call_arguments` returns an empty list if parameter and argument counts differ (VS:11423-11425) or `strips_erased_parameter` is `None` (VS:11428-11431).
6. Same job elsewhere: `retain_subslice_endpoints` (VS:5459) vs `retain_call_arguments` (VS:1191): same two-row append (bindings + expression).
7. Callers (grepped): `subslice_endpoints` + `sites` are used by `execution_scalar_computations.rs:24-25, 10614` and `execution_terminal_unit_control.rs:2285`; comment VS:11319-11320 says "Both scalar plan producers consume this inventory": matches (this file at VS:4980, 4987 and `execution_scalar_computations.rs`).
9. Fail-open: none seen (each missing piece returns an empty `Vec`, i.e. no row).

## 19. Re-export block (VS:11447-11481) and `erased_observations` (VS:11483-11585)

- Re-export block lists `pub(crate) use` of the module functions listed above; `values.rs:3493-3507` re-exports many of them again. Not code that makes decisions.
- `erased_observations::resolve` (VS:11495): follows an observation expression through immutable locals' initializers: static place syntax only (`Name`/`Member`/`Indexed`, VS:11571-11584); a local declared at or after the call statement or without an initializer gives `None` (VS:11523-11525); mutable locals stop (`Some(current)`, VS:11526-11528); whole primitive local -> closed scalar projection leaf if present, else itself (VS:11530-11543); shared-reference local initialised by a `Shared` borrow -> follow the borrow target (VS:11544-11559); with segments -> closed scalar leaf or `None` (VS:11561-11567). A `visited` guard caps loops at `expression_count()` (VS:11505-11509).
- Fail-open (stated): VS:11512-11519 return `Some(current)` for non-place syntax, no canonical place, no symbol root, or no local declaration, i.e. pass the expression unchanged. Comment VS:11490-11491: "Source type/loan/scope/precondition validation remains the admission owner." Stated.
- Only caller: `call_lowering::lower_erased_scalar_argument` (VS:1623, via `super::erased_observations`).

## 20. Notes on cross-cutting patterns

- Bounded walks use hard-coded caps: depth 64 (`charge` VS:3379; many `depth >= 64` checks, e.g. VS:6320, 6521, 6587, 6614, 6902, 7339, 7446, 7521, 7618) and 4096 work units (VS:3039 etc., passed via `remaining` from callers at `lower_scalar_contract_predicate` VS:6719); symbol-derivation chain caps of 8 (VS:1976, 2210, 2350, 2375).
- Judgments keyed on spelling/names: `PrimitiveType::from_name(name)` and `symbols.name(...) == name` (VS:3943-3953); `parameter.name` vs spelling (VS:3606, 3872, 4388, 5621); `local.name == display_name` (VS:4417); `data_definition_positions_for_name` (VS:10938); builtin intrinsic name (`from_intrinsic_name(builtin.name())`, VS:1271); `PrimitiveType::from_name(carrier.name())` (VS:1292); `spelling.is_self_receiver()` (VS:11042).

## Coverage

- Read in full: `values_scalar.rs` lines 1-11585 (11585 lines), in six Read calls (1-1800, 1801-3600, 3600-5399, 5400-7199, 7200-8999, 9000-10799, 10800-11585, with line 3600 read twice).
- Skimmed: none.
- Not read: none.
- Greps run (all against `<checkout>/omega-rust`, `*.rs`): `values_scalar`; `scalar_parameter_roster` (count mode); `occupies_scalar_position(` content; a function-name alternation for `lower_in_declaration|lower_state_scalar_expression|lower_machine_entry_scalar_argument|lower_current_state_predicate|is_scalar_return_call|storage_observation_is_frozen|lower_scalar_parameter_refinement_predicates`; another for `lower_callable_scalar_membership|lower_parameter_scalar_expression|lower_state_domain_predicate|unit_scalar_binding_symbols|lower_machine_entry_boolean_expression|evaluate_closed_boolean_expression|lower_callable_parameter_range_requirements|lower_scalar_parameter_range_requirements`; `fn lower_proof_term|...` definitions; `fn peel_type_shell|...` definitions; `semantic_casts::|has_only_vacuous_tags|same_carrier_operand|has_declared_domains`; `operator_is_builtin\(` (count mode); `view_local_root|owned_local_root|...` (count mode); `authored_entry_parameters|nested_structural_call_return_type|subslice_endpoints...`; `values::scalar|crate::values::` in `execution_scalar_computations.rs` (count: 62). I did not read any site outside this file.

## Five things a maintainer most needs to know about this unit

1. One file holds many independent lowerers over the same predicate algebra. There are four Boolean-expression walkers with separate Not/And/Or/Equal/comparison arms: `BooleanLowering` (VS:385), crash `Reader::boolean` (VS:3387), `machine_parameter_booleans::structural::lower` (VS:5581), `ContractPredicates::boolean` (VS:7602). They differ in how they name parameters and which operator-meaning checks they run.
2. "Which parameter is this name?" is answered by at least five different routines with different keys (symbol only, symbol-or-spelling, symbol and spelling): VS:3861, 4370, 5610-5622, 6128, 7052. `parameter_position` (VS:4383-4389), the structural Name arm (VS:5619-5622) and `local_position` (VS:4417) fall back to spelling; `structural_data` falls back to a name lookup (VS:10938-10941).
3. Plan builders skip silently. Throughout (VS:979-1001, 2493-2506 via `?`, 4585-4601, 4657-4672, 5174-5191) a failed lowering drops the row rather than reporting; consumers outside this file must detect the gap. `atomic_operands::lower_carrier` always returns `Some` (VS:254) even when it produced nothing, and its callers stop planning that statement (VS:4622-4648, 4820-4843).
4. `operator_is_builtin` (VS:4480-4489) treats "no recorded operator use" as builtin; most lowerers (e.g. VS:488, 9333) rely on it alone, while contract lowerers add `has_builtin_*_meaning` checks (VS:7894, 7947, 7805).
5. Many `pub(crate)` entry points have exactly one outside caller (grepped): `lower_current_state_predicate`, `lower_machine_entry_scalar_argument`, `lower_scalar_parameter_refinement_predicates`, `lower_state_scalar_expression`, `is_scalar_return_call`, `nested_structural_call_return_type`, `lower_parameter_scalar_expression`, `lower_state_domain_predicate`, `storage_observation_is_frozen`, `lower_callable_parameter_range_requirements`. `scalar_parameter_roster`, documented as the shared roster source (VS:4335-4339), has no user outside this file; 10 other files re-derive the same filter.

## What I am unsure about

- Whether consumers outside this file reject the missing rows left by the silent-skip pattern (item 3 above), and whether `exact_predicate_intervals`/`exact_predicate_conjunction` can return empty lists that reach the `Constant(true)` starting values (VS:7552, 6880-6882).
- Whether the `continue` at VS:8119-8135 (non-interval `Domain` constraints add no slot) is intentional because another owner handles semantic-atom domains; I saw no comment saying so.
- Which of the `proof_symbols_correspond` matches (VS:2226-2234) are reachable with unrelated symbols; I read the helper but not `generated_from` assignment.
- The 10 outside filter sites for `occupies_scalar_position` were identified only by grep line text; I did not read them to see whether each is a roster rebuild or something else (e.g. a count).
- Counts for `operator_is_builtin(` include the definition line and lines in other files that may name a differently-scoped function; I counted lines matching the text, not resolved call targets.
