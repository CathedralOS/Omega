# Record: unit_057 gap_029 (area 02-source-stages-00-04, crate 03 symbol-resolved-trees-to-typed-trees)

Second pass over the ranges the first reader of unit 057 did not read. Root `T/` = `source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/`. Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Everything below was read unless marked "grepped" or "inferred". Counts come from `grep -rF <name> --include=*.rs` under `source-rust/` (lines; files).

---

## T/expression_lowering.rs lines 2933-4221

These lines sit inside nested modules (`table`, inside it `member_binding`). The start (2933) is mid-function, inside the end of a function that precedes `selected_member`.

1. **Purpose.** Two jobs. (a) `member_binding` (2937-3476): when a member access `a.b` has no field symbol yet, find the declared field by reading the receiver's declared type, so the typed member carries a field symbol or a collection measure. (b) The tail (3753-4221) is the public wrappers that run `ExpressionTableLowerer` in different modes (executable, fact position, scoped, transition pattern) and the lowering of static machine arguments, which keeps authored-selection occurrences.
2. **Shape.**
   - `selected_member` (`:2937`, pub(crate)) returns `(MemberSelection, Option<DiagnosticName>)`; `declared_member` (`:2988`); `declared_receiver_field` (`:3015`, depth limit 128); `selected_field` (`:3055`); `constant_use_type` (`:3094`, pub(super)); `declared_type` (`:3132`, pub(crate)); `declared_field` (`:3222`); `declared_data_field` (`:3232`); `sole_payload_field` (`:3317`); `inferred_capture_type` (`:3345`); `declared_symbol_type` (`:3383`, pub(crate)).
   - Test module `member_selection_tests` (`:3478-3750`): two tests (`:3622`, `:3676`) on `finalize_authored_member_selection` (defined before 2933, not in this range).
   - Wrappers in `table`: `lower_expression_handle_from_table_with_symbol_substitution` (`:3759`), `lower_scoped_expression` (`:3773`), `lower_executable_expression_handle_from_table` (`:3787`), `..._in_fact_position` (`:3803`), `lower_scoped_fact_expression` (`:3814`), `lower_transition_pattern` (`:3827`). Outer module wrappers: `lower_executable_expression_handle` (`:3841`), `lower_expression_handle` (`:3865`), a second `..._with_symbol_substitution` (`:3887`), `..._in_fact_position` (`:3905`), `lower_static_machine_argument` (`:3916`), `lower_static_machine_argument_with_occurrences` (`:3925`), `static_machine_argument_selection` (`:4063`), `lower_fact_expression` (`:4134`). Test `static_selection_tests` (`:4146-4220`, one test).
3. **Inputs and outputs.**
   - `member_binding` reads `SymbolResolvedTrees` (symbol table, `data_definitions`, `data_members`, `data_payload_fields`, `machine_states`, `state_local`, `state_parameters`, `domain_definitions`, `measures`, `const_declaration`, `authored_declaration_selections`) and the resolved `ExpressionTable`. It mutates nothing; it returns a selection or a type reference borrowed from the program.
   - Wrappers build `ExpressionTableLowerer` (defined elsewhere in the file, not in this range) with flags (`with_source_owner`, `admitting_policy_operators`, `collecting_blocks`, `in_fact_position`) and call `.lower(expression)`, writing into `TypedTrees`.
   - `lower_static_machine_argument_with_occurrences` reads a resolved `StaticMachineArgument`, lowers its type reference through `type_reference::lower_type_reference_into_trees_with_exposure` (`:3938`), writes a typed `StaticMachineArgument`, and inserts static members into `target.expression_table` (`:3982`).
4. **How it decides.**
   - Field selection is keyed on identity and on field name. `declared_member` first trusts `member.member_symbol` if valid (`:2993`). Otherwise `declared_data_field` matches fields by name string `field.name.as_str() == member.member.as_str()` (`:3269`, `:3290`, `:3328`), then requires a symbol-identity check `exact_field_symbol` (parent equals owner, and `symbols.name(field.symbol) == field.name.as_str()`, a name compare, `lowerer.rs:2110-2119`). If `member.member_symbol` is valid and differs from the found field, it returns `None` (`:3274-3277`, `:3302`).
   - A data definition is used only if exactly one definition has that symbol and `exact_top_level_data_symbol` holds (`:3073`, `:3242`). That function compares the symbol name to the definition name string and requires the parent to be the root (`lowerer.rs:2100-2108`).
   - A variant payload field is found by comparing `variant.name.as_str() == selected.as_str()` (`:3255`), where `selected` is `member.case_variant`.
   - Collection length is chosen by spelling: `CollectionMeasure::from_authored_spelling(member.member.as_str())` (`:2968`), only when the receiver's declared type, after `peel`, is `Slice` or `FixedArray` (`:2957-2966`) and `case.is_none()`.
   - `declared_type` keys on the expression node kind: `Name` via reserved result or constant or captured local, `Call`, `Borrow`, `Indexed`, `Member`, `SavedSubject` (`:3141-3219`). A call to a value callee reads the callable contract's return type (`:3156-3170`).
   - `inferred_capture_type` finds the owning state by `state.symbol == parent` (`:3364`), uses a local's declared type unless the type is inferred and the role is `Value` or `DispatchSubject`, in which case it recurses into the initializer (`:3369-3380`).
   - `static_machine_argument_selection` returns `None` unless the symbol kind is `Machine` or `State` and an exposure is supplied (`:4073-4083`). It builds a span from the first and last path segments (`:4090-4100`) and picks the occurrence matching span, exposure and `Kind::StaticArgument` (`:4102-4108`); two matches is an error (`:4110`); none falls back to a coordinate lookup (`:4118`).
5. **Cases by shape.**
   - `declared_type` returns `None` for any node not in the list above (`:3218`). Index expressions that are ranges return `None` (`:3181-3188`); only `FixedArray` and `Slice` collections yield an element type (`:3191-3195`).
   - `declared_symbol_type` handles `TypeParameter` (const or value kinds only), `Parameter` under `Domain`, `Data`, `Measure`, or any other parent (state parameter), `Local`, `Field`; `_ => None` (`:3391-3475`).
   - `selected_field` handles an owner that is `Data` or `Variant` only (`:3063-3067`).
   - `lower_static_machine_argument_with_occurrences` rewrites an application of a `Data` symbol into a plain type reference, and rewrites a type reference that is a const literal or a `Named` with invalid symbol into a static operand (`:4033-4059`).
   - Comment at `:3309-3316`: "A spelling two cases share stays unresolved here, and whoever rejects or selects it today still does."
6. **Same job elsewhere.**
   - `declared_symbol_type` and `inferred_capture_type` read a local's type from a state's local table; the first reader noted `member_binding::selected_member` on the typed side (`expression_meanings.rs:171`, per unit_057.md) uses `declared_projection_field` for the same member-by-name job; not read by me (suspected duplicate of `declared_data_field`).
   - Two outer wrappers of the same name: `lower_expression_handle_from_table_with_symbol_substitution` at `:3759` (in `table`) and `:3887` (outer, forwards to it), and likewise `..._in_fact_position` at `:3803` and `:3905`. Grepped: the first name 9 `lower_..._with_symbol_substitution`-style hits are not separable; `lower_expression_handle_from_table_in_fact_position` 8 lines in 3 files.
   - `lower_static_machine_argument` vs `lower_static_machine_argument_with_occurrences`: the first forwards with an empty occurrence slice (`:3922`).
   - `declared_type` name also exists in `02/declarations/machine_equations.rs:451` and `build/planning/package_review/telescope.rs:190` (grepped `fn declared_type`); I did not read them (suspected, different function).
7. **Unused or one-caller (grepped, `source-rust/`).**
   - `lower_static_machine_argument_with_occurrences`: 9 lines, 2 files. `lower_static_machine_argument(`: 22 lines, 8 files.
   - `constant_use_type`: 4 lines, 1 file (its definition, two uses in `declared_type`, one more). `declared_symbol_type`: 2 lines, 1 file (definition and one use at `:3356`); no outside caller despite pub(crate).
   - `declared_receiver_method_target`: 5 lines, 2 files. `lower_fact_expression`: 3 lines, 2 files. `lower_transition_pattern`: 4 lines, 2 files. `lower_expression_handle_from_table_in_fact_position`: 8 lines, 3 files.
   - `declared_type` in the typing crate: 30 lines of `declared_type(` in `T/*.rs`; the comment does not call it THE shared route.
8. **Stated limits.** `:3309-3316` above. `:2954-2956` "Only an actual builtin collection type gives this observation its meaning; unknown receivers stay unresolved." `:4120-4121` "A selected declaration path also uses this operand form."
9. **Fail-open risks.**
   - `selected_member` `:2985`: returns `Ok((Unresolved, case))` when no field is found, receiver is not a collection, or a collection member name is not source-backed (`:2975`). Wrong input: a member access on a receiver whose declared type this walk cannot read. Stated (`:2955-2956`); whether a later stage refuses an `Unresolved` member is not traced here (open).
   - `static_machine_argument_selection` `:4079`, `:4082`, `:4086`, `:4089`: returns `Ok(None)` (no authored occurrence) when exposure is absent, the argument has no path, or the path is not source-backed. A compiler-generated argument therefore carries no selection custody. Stated only at `:4120`; the earlier returns have no comment (open).
   - `:4122` `Missing` coordinate error becomes `Ok(None)`. Stated (`:4120-4121`).
   - `declared_type` `_ => None` (`:3218`) leaves callers with "type unknown"; callers decide. Not traced.
   - `selected_field` `:3059`, `:3066`: returns `None` for any non-Field symbol or non-Data/Variant owner; unresolved, not accepted.

---

## T/lowerer.rs lines 1101-2394

Start (1101) is the middle of a function that checks a candidate against a retained base (see `:1123`, which is the tail of a `candidate retains base` boolean).

1. **Purpose.** (a) Finishes the check that a seeded continuation's candidate keeps the retained base's declarations intact and joins the resolved tables back in (`join_resolved_semantics`). (b) `target_machine_families::append` records per-target machine bodies as one family and holds each body to the interface's signature. (c) The cohort driver `type_resolved_cohort` and `close_typed_cohort`, plus `lower_symbol_resolved_trees`, the `Lowerer` struct and its per-declaration `lower_*` methods, and `rebuild_fresh_typed_trees`.
2. **Shape.**
   - `join_resolved_semantics` (`:1144`), `resolved_root_prefix_is_retained` (`:1233`, `retains!` macro over 12 families), `retained_declaration_families_are_exact` (`:1258`).
   - `target_machine_families::append` (`:1285`), `SharedSignature { Held, NotCompared }` (`:1371`), `require_shared_signature` (`:1382`), one test (`:1536`).
   - `validate_resolved_semantics(_in)` (`:1567`, `:1573`), `lower_symbol_resolved_trees` (`:1585`, pub), `TypingCohort` (`:1628`, per-family frontiers `usize` plus `completed_keys: Vec<(u32,u32)>`), `continuing` (`:1651`), `completing` (`:1675`), `completes` (`:1682`), `symbol_key` (`:1689`).
   - `type_resolved_cohort` (`:1693`), `complete_retained_machine_body` (`:1804`), `check_static_value_shadowing` (`:1832`), `close_typed_cohort` (`:1867`), `lower_service_applications` (`:1989`), `lower_authored_service_reach_row` (`:2045`), `append_authored_service_reach_rows` (`:2063`), `lower_evidence_forwarding` (`:2077`), `exact_top_level_data_symbol` (`:2100`), `exact_field_symbol` (`:2110`), `declaration_exposure` (`:2121`), `Lowerer` (`:2133`; fields `typed_trees`, `source_trees`, `source_owner`, `type_reference_exposure`, `admits_contract_policy_operators`, `pending_blocks`, `block_captures`), `PendingBlock` (`:2161`), `impl Lowerer` methods (`:2167-2326`), `rebuild_fresh_typed_trees` (`:2328`).
3. **Inputs and outputs.**
   - Cohort functions read `SymbolResolvedTrees` families (`const_declarations`, `data_definitions`, `type_aliases`, `domain_definitions`, `propositions`, `mathematical_definitions`, `machines`, `measures`, `operators`, `traits`, `conformances`, `wire_schemas`) from the frontier `cohort.<family>` onward (`.skip(cohort.X)`, `:1699-1800`) and push typed rows into `TypedTrees`. Machines before the frontier are only completed if `cohort.completes` is true (`:1761-1767`).
   - `close_typed_cohort` (`:1867-1985`) runs 20-odd passes over `TypedTrees` in a fixed order: service applications, static-value shadowing, alias uses, `bind_calls`, satisfied declarations, progress premises, static argument contexts, nominal machine parameter contracts, const argument carriers, domain constraints, token shapes, constructor applications, data applications, construction destinations, qualification casts, construction destinations again (`:1937`), computed-argument validation, equality preparation, provider default calls, target families, `bind_calls` again, `admit_types`, indexed operand types, `defer_unbound_calls`.
   - `join_resolved_semantics` copies `symbols`, `service_reach_rows`, `external_bindings`, authored selections from `source` into `candidate` (`:1223-1228`).
4. **How it decides.**
   - Continuation retention is by prefix equality of whole rows: `iter().take(base.len()).eq(base.iter())` (`:1124-1140`, `:1239`) and exact equality for families that may not grow (`:1262-1269`). Retained semantic domains are keyed on ordinal identity `SemanticDomainId(ordinal)` and compared by `name` through `intern` (`:1210-1221`). Authored service ceilings matched by `owner` identity (`:1198-1208`).
   - Target families: group by `target_family` handle (`:1300`); a body is held to the interface by comparing type-parameter counts and lifetime counts (`:1401-1406`), state parameter count and `relevance` (`:1422-1428`), callable identity (`:1430-1437`), result type identity (`:1438`), `purpose` (`:1443`). Members are sorted by target name string (`:1349`), families sorted by `name.as_str()` (`:1357`).
   - `check_static_value_shadowing`: keyed on the parameter's bound type being a `Generic` whose `base_symbol` has a universe sort (`:1854-1857`); otherwise a name conflict error.
   - `exact_top_level_data_symbol` / `exact_field_symbol` decide by symbol kind, parent identity and symbol name equal to declared name (`:2105-2107`, `:2116-2118`).
   - `lower_evidence_forwarding` adjusts `statement_index` by counting proof-output calls with same machine and state symbols and a smaller `source_statement_index` (`:2081-2092`).
5. **Cases by shape.**
   - `require_shared_signature` compares only a fixed list of properties; the comment (`:1397-1400`): "Typing has no such comparison yet, and an unequal count is already a mismatch." Generic families return `NotCompared` (`:1407-1413`).
   - It compares only the first state of each machine (`machine_states(..).first()`, `:1414-1419`).
   - `append` skips declarations with no `authored_target` (`:1319-1321`), refuses a family with no members (`:1344`), and refuses appending to a family already retained (`:1308-1316`).
   - `type_resolved_cohort` handles exactly the 12 listed families in a fixed order; no other family is typed.
6. **Same job elsewhere.**
   - `resolved_root_prefix_is_retained` / `retained_declaration_families_are_exact` repeat the "prefix retained / exact retained" test on each family; the same prefix comparison also appears at `:1123-1140` for 3 families (`const_declarations`, `data_definitions`, `traits`) in the function that ends at `:1142` (read from `:1101`). So three families are prefix-compared in two places (`:1123-1140` and macro at `:1242-1254`; the macro also lists them).
   - `exact_top_level_data_symbol` is called by `expression_lowering.rs` (`:3073`, `:3242`) and `exact_field_symbol` there too. Grepped: `exact_top_level_data_symbol` 4 lines in 2 files; `exact_field_symbol` 9 lines in 3 files.
   - `declaration_exposure(is_public)`: 12 lines in 2 files (all call sites are in `lowerer.rs` and one other); same job as the per-declaration `is_public` mapping.
7. **Unused or one-caller (grepped, `source-rust/`).**
   - `append_authored_service_reach_rows`: 3 lines in 1 file. `lower_evidence_forwarding`: 6 lines in 1 file. `validate_resolved_semantics_in`: 4 lines in 1 file. `TypingCohort::continuing`: 2 lines in 1 file (definition plus one call). `symbol_key`: 82 lines in 17 files (the substring matches other crates' `symbol_key` too; not separated).
   - No comment here calls any item THE single/shared route.
8. **Stated limits.** `:1397-1400` (above). `:1374-1376` "The family binds generic parameters, which typing does not compare yet." `:1332-1334` "The body was not held to the interface, so neither it nor the interface a call is typed against may be read." `:1379-1381` "Whether the body's contracts hold is not asked here." `:1590-1591` "Type arguments are classified while expressions are lowered".
9. **Fail-open risks.**
   - `require_shared_signature` `:1407-1413` returns `NotCompared` for any generic family or any conformance bounds. Wrong program: a target body with a different signature inside a generic family. Stated and traced in the same file: `append` records a `DeferredRequest::TargetFamilyGenericSignature` refusal on both the interface and the body (`:1335-1337`, `:1359-1364`), so a demand for either raises it. Whether every use reaches a demand is not traced (open).
   - `require_shared_signature` compares only the first state's parameters (`:1414-1420`). Parameters beyond count and `relevance` are checked only through `callable_identity` (`:1430-1437`); I did not read how that identity is built. Open.
   - `join_resolved_semantics` and retention checks return `false`/`Err` on mismatch; none seen as fail-open.
   - `append` `:1319-1321`: a declaration with no `authored_target` is skipped (`continue`) without a member, then the empty-family check at `:1344` catches only a family where all are skipped. A mixed family with one targetless body would drop that body silently. Open (no comment at the `continue`; I did not trace whether resolution guarantees `authored_target` on every family member).
   - `check_static_value_shadowing` `:1848-1852`: a parameter that is not a `Const`/`Value` kind is skipped (`continue`). Wrong program: an overlap on a type-kind parameter passes this check. Open.
   - `complete_retained_machine_body` `:1814-1822`: errors if the base is lost; none fail-open.
   - `type_resolved_cohort` `:1763-1766`: a machine before the frontier whose symbol is not in `completed_keys` is skipped with no typing. Stated by design (retained rows); inferred.

---

## Coverage

- `expression_lowering.rs` lines 2933-4221 (1289 lines): read in full.
- `lowerer.rs` lines 1101-2394 (1294 lines): read in full.
- Skimmed: none. Not read: none.
- Greps: counts in item 7 sections from `grep -rF <name> --include=*.rs` run from `source-rust/` (lines via `wc -l`, files via `grep -rlF`).

## Five things a maintainer most needs to know

1. Member typing in `expression_lowering.rs` finds a field by name string plus symbol identity: `declared_data_field` matches `field.name.as_str() == member.member.as_str()` (`:3290`) and then `exact_field_symbol` checks parent and name equality (`lowerer.rs:2110`). A pre-set `member_symbol` that disagrees refuses the lookup (`:3302`).
2. `selected_member` returns `Unresolved` rather than an error when the receiver type cannot be read (`:2985`); the only error is a source-backed unknown member on a slice or fixed array (`:2975-2981`).
3. Target-machine bodies are held to their interface only for non-generic families, and only by first-state parameter count and relevance, callable identity, result type identity and purpose (`lowerer.rs:1382-1447`). Generic families get a deferred refusal instead (`:1359-1364`).
4. `close_typed_cohort` (`lowerer.rs:1867`) is the fixed ordered list of passes that finish typing; `construction_destinations::prepare` runs twice (`:1920`, `:1937`) and `bind_calls` twice (`:1879`, `:1968`).
5. `TypingCohort` frontiers are plain `usize` positions per resolved family (`:1628`), and continuation retention is by prefix equality of whole rows (`:1124-1140`, `:1239`), so an edit to a retained row is refused, not merged.

## What I am unsure about

- Whether an `Unresolved` member (`expression_lowering.rs:2985`) is always refused by a later stage.
- Whether `authored_target` is guaranteed present on every target-family member (`lowerer.rs:1319`).
- What `machine_callable_identity` covers (`lowerer.rs:1430`); not read.
- Which `declared_type(` hits in the typing crate (30 lines) are the `member_binding` one versus other functions with the same name.
- The start of `expression_lowering.rs:2933` and `lowerer.rs:1101` are mid-function; the enclosing function heads are outside my ranges.
