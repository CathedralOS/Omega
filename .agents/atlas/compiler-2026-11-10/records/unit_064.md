# Unit 64 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`):

1. `published_facts_crash_entry_values.rs` (lines 1-2546)
2. `shared.rs` (lines 1-4805)
3. `values.rs` (lines 1-3589)

Short names below: PEV = published_facts_crash_entry_values.rs, SH = shared.rs, VA = values.rs. Everything was read in full (Read tool). Counts come from the Grep tool (count or content mode) across `omega-rust` unless noted.

---------------------------------------------------------------------------

## 1. PEV: `published_facts_crash_entry_values.rs`

### Purpose
When a callee's crash predicate (a guard over the callee's formal parameters) is moved into the caller, the caller's actual operand has to be expressed in terms of the values the *invocation entry* received. This file answers "what does this operand name at entry to the state, and is it still true at the read point?". It returns `CrashPredicateExpression` trees (`Parameter(n)`, `Member`, `Indexed`, `Binary`...) or `None` when provenance cannot be proven. It also holds the syntactic "may this statement overwrite this place" scan that decides whether a mutable binding still holds its bound snapshot (read; header comment PEV:1-32).

### Shape
Module layout (inline `mod`s, read):
- `checked_projection::entry_value` (PEV:60) - index values for a projection come from the ordinary scalar plan (`lower_unit_scalar_argument`, `evaluate_checked_scalar`) at the statement's entry contexts. Rejects if `statement_may_overwrite_place` for any read place (PEV:141-148).
- `literal_projection` (PEV:168-364): `entry_value` (PEV:195), `literal_index` (PEV:218, `pub(crate)`), `projected_entry_value` (PEV:232) - walks `Indexed`/`Member`/single-segment `Name`, going back to the local's initializer via `crate::flow::literal_expression_projections` (PEV:329) and requiring exactly one projection (PEV:336).
- `places` (PEV:365-711): `entry_place_before_statement` (PEV:381), `entry_place_at_write_boundary` (PEV:391), `entry_parameter_place` (PEV:469), `entry_parameter_place_before_statement` (PEV:490), `entry_parameter_place_at_write_boundary` (PEV:507), `projection_requires_write_scan` (PEV:644), `traversed_storage_requires_write_scan` (PEV:681). The two `*_at_write_boundary` differ only in `write_scan_end` (`statement` vs `statement + 1`).
- `mutable` (PEV:716-1315): `enum PlaceSegment { Field, Case, FixedIndex(usize), FixedRange{start,end}, Opaque }` (PEV:761); `canonical_place_segment` (PEV:781); `fixed_index_segment` (PEV:806); `paths_interfere` (PEV:841); `rooted_place_path` (PEV:895); `place_interferes` (PEV:947); `statement_may_overwrite_place` (PEV:966); `storage_holds_bound_value` (PEV:989); `statement_may_overwrite` (PEV:1006); `call_receiver_path_interferes` (PEV:1091); `transition_may_overwrite` (PEV:1115); `expression_may_overwrite` (PEV:1177); `receiver_call_writes` (PEV:1262); `call_arguments_may_overwrite` (PEV:1280).
- Top level: `MAX_ENTRY_PROVENANCE_DEPTH = 64` (PEV:1324); `entry_operand` (PEV:1326, pub(super)); `entry_operand_at` (PEV:1742, the main expression walk); `operand_entry_provenance` (PEV:1358, returns bool); `entry_operand_projected` / `_at` (PEV:1482/1501); `member_hop_path` (PEV:1645); `formal_member_projection` (PEV:1673); `formal_member_place_projection` (PEV:1726); `entry_operand_name_at` (PEV:1971); `local_read_initializer` (PEV:2010); `local_initial_value_before_statement` (PEV:2047); `entry_operand_symbol_at` (PEV:2071); `ieee_comparison_has_entry_meaning` (PEV:2132); `builtin_binary_meaning` (PEV:2168); `builtin_index_meaning` (PEV:2185); `observation_requires_write_scan` (PEV:2206); `state_parameter_entry_operand` (PEV:2260); `named_transition_arguments` (PEV:2361); `substitute_entry` (PEV:2412); `substitute_entry_projected` (PEV:2434).

### Inputs and outputs
Reads typed trees through `&dyn CheckingInput` (expression table, statement table, state/machine tables, type reference table), `validation::*` helpers, `facts::PlaceSegment`/`CanonicalPlace`, and (checked_projection only) `FlowFacts`, `FactPlan`, `CheckedOperatorFacts`. Mutates nothing; returns `Option<CrashPredicateExpression>`, `Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>`, `bool`, or `Option<ExpressionHandle>`. Imports `field_entry_name`, `member_entry_name` from `super::crash_calls` (PEV:34) - those are defined elsewhere (not read).

### How it decides
- Whether a binding still holds its entry value: keyed on **statement position windows** (`state[start..end]` statements) and on **projected place path** (`PlaceSegment` vectors). `storage_holds_bound_value` (PEV:989-1004) returns false for a malformed window. A write interferes if `paths_interfere` (PEV:841) cannot separate the paths by symbol identity of fields/cases or numeric index/window.
- Which expressions can be an entry operand: keyed on **expression node kind** in `entry_operand_at` (PEV:1781-1962): `Match` (all arms must give the same value, PEV:1782-1792), Boolean/Integer/Float literals, recast `Cast` (same-size fixed integers only, PEV:1801-1828), widening `Cast` (`integer_widen_is_total`, PEV:1829-1845), `Unary` only `LogicalNot` (PEV:1846-1853), shared `Borrow`, `Member`, `Indexed`, `Range`, 18 listed `Binary` operators (PEV:1917-1951), `Name`. Everything else -> `None` (PEV:1961).
- Member identity: `effective_member_symbol` then `facts::payload_variant_for_field` (PEV:1649-1661); i.e. keyed on **symbol identity**, not spelling. Exception: builtin `len` detected by **source spelling** `CollectionMeasure::from_authored_spelling(member.member.as_str()) == Length` with no case variant (PEV:1651-1654) - the one hop that has no member symbol.
- `entry_parameter_place_at_write_boundary` rebuilds the path by comparing `field_entry_name(...)` **presentation strings** to the `member` string (PEV:582-588), rejecting duplicates or erased fields (PEV:589-591). So a place->path conversion is keyed on the rendered member name matched against each field's rendered name, then re-authenticated via `resolve_structural_parameter_path` identities (PEV:619-624).
- State parameters (PEV:2260-2351): entry state contributes `Parameter(ordinal)` (PEV:2298-2304); every named transition edge into the state must resolve to the *same* operand (equality of `CrashPredicateExpression`, PEV:2342-2347); a self-edge forwarding the same parameter by name is skipped (PEV:2308-2332), with a pristine-storage check when mutable.

### Cases by shape
- `entry_operand_at` accepts only the fixed list above (PEV:1781-1962), refuses the rest.
- `literal_projection::projected_entry_value` handles only `Indexed`, `Member`, single-segment `Name` (PEV:247-362); `_ => None`.
- `fixed_index_segment` (PEV:806-824): only a literal-folded index or `a..b` range with both ends constants becomes `FixedIndex/FixedRange`; everything else `Opaque`.
- `call_receiver_path_interferes` (PEV:1091-1108): receiver name path of 0 or 1 members = whole binding; exactly 2 members with a valid `receiver_symbol` = one `Field`; deeper = `Opaque`. Comment: "a deeper or unresolvable projection cannot be separated below its root" (PEV:1088-1090).
- `places::entry_place_at_write_boundary` (PEV:404-433): accepts a `Case` (once, not nested), `Field` (must belong to the selected case), `FixedIndex` (only when no case pending); any other segment -> `None`. Dynamic `Index`/ranges are refused.
- `operand_entry_provenance`/`entry_operand_projected_at`: operand spine may be `Member`, `Indexed`, shared `Borrow`, `Name`; any other node with a non-empty projection -> refuse (PEV:1456-1467, 1594-1607).

### Same job elsewhere
- `paths_interfere` + `PlaceSegment` here vs `canonical_place_segment_pair_may_overlap` in `omega-rust/psi/compilation/validation/src/places.rs:665` (grepped, read only the call sites listing; not read). The PEV comment at PEV:834 says it mirrors that function. Two implementations of "can these two place paths overlap". Suspected duplicate (not read).
- `rooted_place_path` (PEV:895) mirrors `expression_root_symbol` rooting per its own comment (PEV:889-893); `expression_root_symbol` not located/read.
- `entry_operand_projected_at` (PEV:1501) and `operand_entry_provenance` (PEV:1358) walk the same spine twice, once returning bool, once returning the expression (comment PEV:1472-1481 says "the expression-returning twin").
- `entry_operand_at` Member arm (PEV:1859-1902) re-implements the member-chain walk that `entry_operand_projected_at` also does (PEV:1521-1531).
- `entry_place_before_statement`/`entry_parameter_place_before_statement` vs `entry_place_at_write_boundary` with `write_scan_end` = statement or statement+1: three thin wrappers over two functions (PEV:388, 476-484, 497-505).

### Unused / one-caller
Grep across `omega-rust` outside this file:
- `entry_operand` (pub(super), PEV:1326): 3 external callers of the free function (`published_facts_crash_calls.rs:1684`, `:3995`, `published_facts.rs:10880`); the other `entry_operand` hits are a different method (`published_facts_crash_calls.rs:1887, 2825, 3356`).
- `checked_projected_entry_value`: 1 external call (crash_calls:1704).
- `statement_may_overwrite_place`: 6 external uses (crash_calls:1759, 2112, 3570 import, 3837, 3986; counting the `use` line once) -> 5 call sites + 1 import.
- `formal_member_place_projection`: 1 (crash_calls:1895). `formal_member_projection`: 1 external use (published_facts.rs:8548) plus the import at :7888.
- `entry_place_before_statement`: 1 (crash_calls:1911).
- `entry_parameter_place_before_statement`: 6 call sites (checks_contracts.rs:7829, 7856; crash_calls:1998; published_facts.rs:7113; flow_context.rs:705, 786).
- `entry_parameter_place` (non-"before"): 1 (published_facts_boolean_predicates.rs:1385).
- `local_initial_value_before_statement`: 1 (crash_calls:2040).
- `operand_entry_provenance`: 1 (published_facts.rs:8219). `entry_operand_projected`: 1 (published_facts.rs:8549).
- `substitute_entry`: 3 (crash_calls:2850, 3338; + definition). `substitute_entry_projected`: 4 (crash_calls:2822, 3353; published_facts.rs:8618, import :7888).
- `fixed_index_segment`, `PlaceSegment` (mutable) are used only inside this file plus the above.
- `receiver_call_writes` (PEV:1262): private, 2 call sites (PEV:1055, 1223). Its body is `parameters.iter().find(|_parameter| false).is_none_or(|p| p.is_mutable)`; the `find` predicate is the constant `false`, so `find` is always `None`, and `is_none_or` is therefore always `true` when the callee's parameters resolve (and `true` when they do not). The function returns `true` on every input (read, inferred from the closure).
- `state_parameter_entry_operand` has a parameter `_before_statement` that is never used (PEV:2266).
- Comment at PEV:2118-2125 (about binding `self` to the state's `is_self` parameter) sits above `ieee_comparison_has_entry_meaning` with no code under it for that purpose (read) - orphaned comment.
- The doc claim "`crash_entry_values` stays the only owner of that answer" appears in published_facts.rs:10870 and README:232; the file has the callers listed above, so the claim has users.

### Stated limits
- "Mutable bindings with unstable contents, divergent arrivals and unresolvable cycles retain no entry identity." (PEV:21-22)
- "Authored indexing and unknown selectors never become builtin constructor substitutions." (PEV:180-181)
- "The scan is deliberately syntactic and conservative: anything it cannot rule out dirties the snapshot" (PEV:724-726)
- "This is source provenance, not a Terminal certificate." (PEV:31)
- Depth bound: "Exhaustion is unproven provenance, never an affirmed origin." (PEV:1321-1322)

### Fail-open risks
1. `statement_may_overwrite` returns `false` for `StatementNode::Assembly(_)` (PEV:1014). No comment at the site. A wrong program would be: an assembly statement that writes a local or parameter between the binding and the read; the entry snapshot would then be kept. Not traced whether assembly statements can write bindings or are separately fenced. **open**.
2. `observation_requires_write_scan` returns `Some(false)` for a float-typed binding with an empty path (PEV:2211-2221) using `peeled_type_reference`, which strips `Reference` and `Constrained` wrappers (read: `typed-trees/src/type_system.rs:3708-3720`). So a binding of type `&mut f64` (or `&f64`) takes the early return with "no write scan needed" and `mutable = parameter.is_mutable` (PEV:2283) / `local.is_mutable` (PEV:2024). If the binding itself is not `mut`, no `storage_holds_bound_value` scan runs, so a write through the reference would not end the entry snapshot. The comment above the function (PEV:2201-2205) describes mutable references as only holding "while the selected storage remains unwritten" and does not mention the float exemption. Whether float reference types reach here was not traced. **open**.
3. `call_arguments_may_overwrite` pairs arguments with callee parameters positionally via `zip` (PEV:1296-1299) without the selector alignment that `arguments_aligned_to_formals` does elsewhere (SH:3399). For a dynamic-slot call with a leading selector the pairing is shifted; and surplus arguments beyond the parameters are never paired. (The per-argument `expression_may_overwrite` over all arguments still runs for the statement call arm, PEV:1065-1077, covering exclusive borrows spelled in the argument.) A write via an already-held `&mut` passed as a shifted argument could be mis-paired. Not traced. **open**.
4. `expression_may_overwrite` relies on `program.subtree_children(node, SubtreeEdges::VALUES, ..)` (PEV:1246-1251) for every node other than `Borrow`/`Call`/`Atomic`; a child edge not in `VALUES` is not scanned. Not traced. **open**.
5. Fail-closed defaults seen: invalid expression handle -> `true` (PEV:1186-1188); unknown callee -> assume args may write (PEV:1293-1295); non-symbol place root -> `true` (PEV:972-974); `paths_interfere` default `_ => return true` (PEV:883) and falls out `true` (PEV:886).

---------------------------------------------------------------------------

## 2. SH: `shared.rs`

### Purpose
A grab-bag of helpers "every checking step reads" (SH:1-7), declared as six `pub(crate) mod`s: `authored_selections` (finalizes the late-bound declaration selections of authored source into checked targets), `call_formals` (one function), `collections` (ordered-unique rosters), `machine_waves` (SCC/wave scheduler), `operators` (boundary operator applications, body candidates, provider requirements). Header lists five, the file has six; `operators` is listed, `collections` listed - all six are listed (read).

### Shape

#### `authored_selections` (SH:9-3386)
Sub-mods and entry points:
- `build_selections::validate` (SH:48): checks Build binding calls against `BuildBindingOperations` boundary trait signatures; produces `Result<(), Diagnostic>`.
- `call_targets` (SH:267-602): `checked_operator_conformance_targets` (SH:281), `checked_statement_call_target` (SH:304), `checked_call_target` (SH:338), `checked_name_path_segment_target` (SH:446), `authored_contextual_root` (SH:517), `implementation_call_selection` (SH:581), `expression_receiver_symbol` (SH:590).
- `contract_resolution` (SH:603-917): `CheckedContractOperatorResolution {Declaration(SymbolHandle), Builtin}` (SH:617), `checked_operator_resolution` (SH:626), `checked_operand_type` (SH:647), `checked_named_operator_call` (SH:683), `checked_spelled_operator_resolution` (SH:748), `checked_float_meaning_equality` (SH:785), `checked_resultless_law_equality` (SH:811).
- `finalization` (SH:918-1591): `finalize_checked_authored_selections` (SH:973, the entry; called at checking.rs:1215), `late_binding_ordinal` (SH:1428), `selection_is_derived_wire_codec` (SH:1446), `undeclared_checked_call_callee` (SH:1482), `top_level_declaration_exists` (SH:1518), `undeclared_statement_call_callee` (SH:1554).
- `member_targets` (SH:1592-2010): `checked_member_target` (SH:1628), `checked_value_member_target` (SH:1675), `authored_member_target` (SH:1703), `for_each_statement_containing` (SH:1717), `contextual_statement_member_target` (SH:1739), `member_symbol_from_type_reference` (SH:1768), `collection_measure_target` (SH:1816), `expression_is_contextual_statement_primitive` (SH:1835), `expression_is_intrinsic_primitive_without_origin` (SH:1887), `expression_is_anonymous_arithmetic` (SH:1988).
- `operator_targets` (SH:2011-2745): `GenericOperatorValueOrigins` (SH:2082, handle-keyed index with `BuildSymbolHasher`), `checked_generic_operator_target` (SH:2108), `checked_operator_target` (SH:2174), `checked_operator_target_for_occurrence` (SH:2295), `checked_structural_equality_call` (SH:2365), `resolve_authored_operator_without_use_fact` (SH:2401), `authored_operand_descriptor` (SH:2460), `authored_operand_carrier` (SH:2484), `authored_operand_type` (SH:2551), `operator_contract_value_type` (SH:2580), `operator_has_no_authored_spelling_candidate` (SH:2625), `typed_operator_has_no_authored_selection` (SH:2665), `typed_operator_authored_selection_candidates` (SH:2675), `candidate_operators` (SH:2691).
- `selection_collection` (SH:2746-3091): `checked_struct_literal_type_symbol` (SH:2777), `collect_checked_proof_membership_selections` (SH:2796), `collect_checked_proof_view_call_selections` (SH:2885), `collect_checked_statement_selections` (SH:3008).
- `static_inputs::collect` (SH:3109): finalizes static-input and interrupt-registrar selections; `collect_inputs` (SH:3203), `collect_registrar` (SH:3277).
- Top-level: `derive_checked_nominal_call_target` (SH:3343), `exact_owner_member_declaration` (SH:3367).

#### `call_formals` (SH:3387-3411)
`arguments_aligned_to_formals(program, call_target, formal_count, arguments)` (SH:3399): drops the first operand when the call target is a dynamic slot, but only if the remaining count equals `formal_count`; else returns the list unchanged.

#### `collections` (SH:3412-3539)
`push_unique` (SH:3437), `unique_in_order` (SH:3444), `UniqueRoster<T>` (SH:3458: `values: Vec<T>`, `positions_by_hash: HashMap<u64, Vec<usize>>` using `DefaultHasher`). Tests at SH:3496-3538.

#### `machine_waves` (SH:3540-3776)
`MachineCallComponents` (SH:3566: `members`, `callee_first_waves`, `caller_first_waves`), `new(reads)` (SH:3580) via iterative Tarjan `strongly_connected_components` (SH:3686), `waves` (SH:3676), `run_callee_first` (SH:3623), `run_caller_first` (SH:3637, `#[allow(dead_code)]`), `run_waves` (SH:3645; runs a wave in parallel via `validation::checking_thread_pool()` and rayon if the wave has more than one component, SH:3652-3660).

#### `operators` (SH:3777-4805)
- `applications` (SH:3784-4523): `bind_boundary_operator_application_demands` (SH:3829) fills `operators.boundary_applications` and `symbolic_boundary_applications`; helpers `checked_spelled_boundary_application` (SH:4099), `spelled_symbolic_boundary_application` (SH:4145), `checked_boundary_application_from_bindings` (SH:4185), `rejoin_validated_arguments` (SH:4230), `rejoin_validated_symbolic_arguments` (SH:4356), `named_expression_operand_types` (SH:4465), `spelled_operand_types` (SH:4494).
- `body_candidates` (SH:4524-4737): `retain_boundary_body_candidates` (SH:4541), `declarations` (SH:4574), `candidates_for_application` (SH:4646).
- `provider_requirements::retain_provider_requirements` (SH:4748).

### Inputs and outputs
- `finalize_checked_authored_selections` reads `program.authored_declaration_selections()`, the expression table, `facts` (operators, values, proof contract facts, flow control, `fact_call_projections`, `semantic.facts`), and mutates `facts.finalized_selection_occurrences` (SH:1424); returns a cloned-and-finalized `AuthoredDeclarationSelections` (SH:1291-1306, 1425). Per-expression work runs in parallel with rayon (`par_iter().with_min_len(256)`, SH:1004-1008).
- `bind_boundary_operator_application_demands` mutates `CheckedOperatorFacts` (SH:4094-4095) from `ValidatedBoundaryOperatorApplication`s plus operator uses.
- `retain_boundary_body_candidates` / `retain_provider_requirements` mutate `CheckedTrees.facts.operators` (SH:4544, 4570, 4798).

### How it decides
- **Which selection to finalize**: only `SelectionTarget::LateBound(binding)` occurrences (SH:1024-1026); those in `product_operands` skipped (SH:1028). The match key is `(LateBinding, ExpressionNode kind)` (SH:1051-1241), nine arms.
- **Call target resolution order** in `checked_call_target` (SH:338-444): authored target; named-operator uses (conflict -> invalid); `exact_named_operator_call`; contract-resolution named operator; exact-owner machine call; flow call facts matched by `find_call_site` expression identity (SH:394-411); then flow call facts matched by **source span equality** `call.authored_source_span != Some(authored_source_span)` (SH:412-426); then `fact_call_projections` by call expression (SH:430-441). Two matching but different targets -> invalid.
- **Operator target**: `checked_operator_target` (SH:2174-2293) tries in order: unconditional Builtin for `And/BitwiseAnd/BitwiseOr/BitwiseXor/Or/ShiftLeft/ShiftRight` (SH:2181-2198); a resolved use; a `BuiltinFallback` use or a `Missing` use with primitive operand (SH:2215-2229); contract resolution; `resolve_authored_operator_without_use_fact` (single candidate); operand primitive-type heuristics (SH:2253-2286); finally `operator_has_no_authored_spelling_candidate` (SH:2287-2292).
- **String-compare keys** (spelling/display): `definition.name.as_str() == "BuildBindingOperations"` (SH:57) and `file.path == Path::new("<build-prelude>")` (SH:66); operation names `"prepare"`, `"await_provider"`, `"exclude"`, `"bind"`, `"bind_description"`, `"select"`, `"bind_native"`, `"select_provider"` (SH:138-186, 216, 251); `exact_build_prelude_data(..., "Build")` (SH:242) and `"ProductEntryRef"` (SH:256); `display_path(conformance.trait_symbol, "::") == "Equatable"` and `row.requirement_name.as_str() == "equals"` (SH:2375-2377); callee name `contains("::")` and top-level name lookup plus `machine.name.as_str() == name` / `state.name.as_str() == name` (SH:1506-1509, 1578-1588); `member_symbol_from_type_reference` finds a field/payload field by name `field.name.as_str() == member_name` and takes the **first** variant whose payload has that name (SH:1795-1808); `operator_contract_value_type` matches operator parameter by `parameter.name.as_str() == name` where name is the last segment of the name path (SH:2585-2597).
- **Unresolved-selection error** (SH:1385-1423): any selection still `LateBound`, not in `descriptor_occurrences`, not a derived wire codec, and whose source file is not `SourceOrigin::Toolchain` (or has no source file) is an error.
- Body candidates (SH:4646-4736): non-generic `CheckedBody` machines; with a specialization, the template is the candidate only if some `operator_realizations` row matches the demand (SH:4661-4669), else the demand must have no arguments (SH:4674-4676); exactly one declaration per (machine, requirement) or an error (SH:4688-4690).
- Provider requirements (SH:4770-4797): machines whose `IntrinsicRequirement::from_requirement` exists, in product, published termination interface, and path of form `owner::name`.

### Cases by shape
- `build_selections::validate`: closed list of `BuildSelectionRequest` variants and, for each, the allowed operation names and the expected ordinary-argument count (1, 2 or 3) (SH:121-204).
- `checked_operator_target` hard-lists seven binary operators as Builtin (SH:2181-2198); `candidate_operators`/`resolve_authored_operator_without_use_fact` handle `Binary` and `Indexed` only; `Unary` returns an empty list (SH:2734, 2436) and `operator_has_no_authored_spelling_candidate` returns `true` for every `Unary` (SH:2648) - so an authored unary operator is never a candidate in this module (read).
- `authored_operand_type` knows `Integer`, `Atomic`, `Borrow`, `Cast`, `Member`, `Call`, `Name` (SH:2558-2577), `_ => None`.
- `authored_operand_carrier`: `Boolean`, `Binary`, `Unary` only (SH:2505-2548).
- `expression_is_anonymous_arithmetic`: `Float`/`Integer` literal without landing; `Add/Subtract/Multiply/Divide/Modulo` only (SH:1993-2007).
- `checked_member_target_from..`: the `Variant` fallback in `member_symbol_from_type_reference` and the type-reference match list (`Reference`, `Constrained`, `Generic`, `Named` handled; `ConstExpression`, `DynamicTrait`, `FixedArray`, `Slice`, `Unit` -> None, SH:1775-1793).
- `collection_measure_target` (SH:1816): "A `len` or `capacity` member on a fixed array or slice" (SH:1811-1815).
- `rejoin_validated_arguments` (SH:4237-4252): only type and const parameters, no lifetime parameters. `rejoin_validated_symbolic_arguments` (SH:4367-4379): "type-only operator telescope".
- `static_inputs::collect_inputs` accepts only `Kind::StaticArgument | Conformance | TypeReference | StaticPathSegment` (SH:3227-3238) and `Binding::CheckedStaticArgument | CheckedStaticPathSegment | CheckedConformance` (SH:3245-3249).

### Same job elsewhere
- `checked_call_target` (SH:338) vs `validation::machine_calls_core.rs:6243 checked_call_target_candidate` and `06_checked-trees-to-lowered-psi/src/projections.rs:1926 checked_call_target` (not read; suspected overlapping role).
- `expression_receiver_symbol` (SH:590) vs `validation/src/calls.rs:8493 expression_receiver_symbol` (read the first 15 lines): the validation one returns `invalid` when a single-member name has `head_symbol != symbol` (calls.rs:8497-8506), SH:590 returns `path.symbol` unconditionally (SH:598). Same name, differing behaviour. SH has 1 external user listed by my grep (selection_collection/finalization imports); the validation one is used at calls.rs:8085.
- Operator-type reconstruction appears three times in SH: `contract_resolution::checked_operand_type` (SH:647), `operator_targets::authored_operand_type` (SH:2551) and `member_targets::expression_is_intrinsic_primitive_without_origin` (SH:1887, which again walks `Binary` operands with `authored_operand_descriptor`). `checked_named_operator_call` (SH:683) repeats the operand-type + candidate filter that `resolve_authored_operator_without_use_fact` (SH:2401) does for binary/indexed.
- Statement-scan helper `for_each_statement_containing` (SH:1717) vs `OwnerEnvironmentIndex::containing_statement_sites` (used at SH:205, 1843) - the latter is the indexed replacement; SH keeps both (SH:1829-1832 comment).
- `UniqueRoster`/`push_unique` (SH:3437, 3458) vs local `push_unique*` definitions elsewhere (grep `fn push_unique\w*`): `published_facts.rs:225, 384`, `published.rs:7456 push_unique_place`, `checks_multiplicity_claim_outcomes.rs:2285`, `checks_content.rs:6668 push_unique_domain_application`, `build-time-evaluation/src/lib.rs:866`, `validation/src/proof_contracts_core.rs:2450`, `terminal-psi/semantics/src/substitution.rs:416, 584`, `validation/src/calls.rs:18641, 20492`. The module comment (SH:3412-3431) says it is "one owner for first-occurrence accumulation" and that the "one-off `push_unique_*` wrappers" are replaced; 13 other definitions named `push_unique*` exist in `omega-rust` (counted from the grep listing: 12 besides SH:3437; one is the SH test fn at SH:3502, excluded -> 12 other definitions incl. other crates).
- `MachineCallComponents`: one SCC implementation here; other SCC code not searched.

### Unused / one-caller
- `collections` is the "one owner" claimed at SH:3412: used in this crate 176 times across 39 files (grep `collections::(push_unique|unique_in_order|UniqueRoster)` in the crate directory), so documented-shared and actually shared.
- `UniqueRoster|unique_in_order`: 25 occurrences in 8 files of the crate including 9 in `shared.rs` (definition and tests); users outside SH: checks_multiplicity_claim_outcomes (4), checks_current_scalar_values (2), checks_contracts (4), checks_content (1), checks (1), flow_mutation (2), flow (2). (grep count mode.)
- `run_caller_first` (SH:3637): grep over `omega-rust` returns only the definition and the in-file test (SH:3769); carries `#[allow(dead_code)]` and the comment "No phase uses this order yet" (SH:3633). 0 production callers.
- `MachineCallComponents`: the module comment (SH:3541-3563) says it serves "termination, mutation, crash and reach summaries"; grep finds one outside user, `checks_termination_progress.rs:370/387` (`new` + `run_callee_first`). Other summary phases are not users.
- `arguments_aligned_to_formals`: 5 call sites outside its definition (checks_contracts.rs:3720; values.rs:63; crash_calls.rs:2729, 5263; plus the `use` at :4913).
- `finalize_checked_authored_selections`: 1 caller (checking.rs:1215). `retain_boundary_body_candidates`: 1 (checking.rs:1396). `retain_provider_requirements`: 1 (checking.rs:1398). `bind_boundary_operator_application_demands`: 1 (published_facts.rs:11028).
- `typed_operator_has_no_authored_selection`: external users flow_exits.rs:1122, 1128; checks_contracts.rs:2164, 2404, 2412, 2459, 2529; products.rs wrapper; build-time-evaluation machine_execution.rs:2914. `typed_operator_authored_selection_candidates`: products.rs:56 wrapper, machine_execution.rs:2979. `derive_checked_nominal_call_target`: products.rs:133, review-evidence (several). `exact_owner_member_declaration`: products.rs:43 only (1 caller). `checked_operator_resolution` and `checked_named_operator_call`: checks_operators.rs:1061, 1077 (1 site each). `CheckedContractOperatorResolution`: checks_operators.rs:1075.
- `expression_is_contextual_statement_primitive` / `expression_is_intrinsic_primitive_without_origin`: 11 occurrences, all in `shared.rs` (grep count) - both are `pub(crate)` with no users outside this file.
- `checked_statement_call_target`, `checked_call_target(`, `checked_name_path_segment_target`, `checked_operator_conformance_targets`, `implementation_call_selection`: no uses outside `shared.rs` in the crate (grep excluding shared.rs returned only unrelated same-named functions in `validation/src/calls.rs` and `06_.../projections.rs`). They are `pub(crate)` but only used inside SH.
- `shared.rs:3332-3339` re-exports: `checked_named_operator_call`, `checked_operator_resolution`, `CheckedContractOperatorResolution` (users above), `typed_operator_*` (users above).
- `GenericOperatorValueOrigins` (SH:2082): used only inside SH (finalization, SH:999, 1194).

### Stated limits
- "symbolic statement boundary applications are not yet supported" (SH:4013) - a statement call whose validated arguments include `TypeBinder` is a hard diagnostic.
- "No phase uses this order yet; the flow inputs and private-entry passes are its consumers." (SH:3633-3634) - `run_caller_first`.
- "Implicit match-equality comparisons carry no authored operator token of their own, so they stay out of this cohort for now." (SH:4142-4144)
- "ambiguous reconstruction stays closed." (SH:608); "partial or ambiguous reconstruction remains unresolved and therefore rejects package custody." (SH:2398-2399)
- "The row is demand only ... authorizes no coverage until a consumer's final substitution closes the application." (SH:4139-4141)
- "late-bound selection in compiler toolchain source, a target sibling's body or a build descriptor: those stay late-bound" (SH:921-925).

### Fail-open risks
1. `finalize_checked_authored_selections` leaves unresolved a late-bound selection whose source file has `origin == Toolchain` (SH:1394-1397) and ones in `descriptor_occurrences` (SH:1387) or derived wire codecs (SH:1391). **stated**: "except a late-bound selection in compiler toolchain source, a target sibling's body or a build descriptor: those stay late-bound for the realization or build evaluation that binds them" (SH:921-924). What gets through: an unresolved selection spelled in toolchain source. Selections with no source file are *not* excluded (`is_none_or`, SH:1397) so they error.
2. `product_operands.contains(&occurrence)` returns `Ok((found, None))` (SH:1028-1030): the occurrence is left unfinalized here. The final unresolved check (SH:1385-1398) excludes `descriptor_occurrences` but not `product_operands`; I did not trace how those occurrences get finalized or excluded (`descriptor_selection_occurrences`, `product_request_operand_occurrences` are in `validation::execution_projection`, not read). **open**.
3. `checked_operator_target` final fallbacks (SH:2253-2292): (a) operand is a primitive according to any one of four independent heuristics -> Builtin; (b) `operator_has_no_authored_spelling_candidate` -> Builtin, which for any `Unary` node is always `true` (SH:2648) and for a binary operator with no spelling is `true` (SH:2633-2635). Also `typed_operator_has_no_authored_selection` -> true for `Unary` via the same function and `expression_is_intrinsic_primitive_without_origin` returns `true` for `Unary` (SH:1967) and for binary with no spelling (SH:1926-1930). What gets through: a unary operator expression on a non-primitive operand that has a user-declared unary operator would be settled as `BuiltinOperator` rather than the declaration. I did not check whether the language has authored unary operators. **open**.
4. Seven binary operators are `Builtin` regardless of operand type (SH:2181-2198). A user-declared overload for any of them would not be selected here. Whether such overloads exist in the language was not checked. **open**.
5. `authored_operand_carrier` / `authored_operand_descriptor` return `OperandType::Unknown` when no carrier is known, and Unknown matches every declared parameter (comment SH:2455-2458: "reported as unknown they match every declared parameter as a wildcard"). Candidate lists are therefore supersets; `resolve_authored_operator_without_use_fact` then needs exactly one (`let [candidate]`, SH:2444). Over-approximation of candidates only makes selection refuse, not accept. **traced** (read).
6. `checked_resultless_law_equality` (SH:811-875): returns `found` only if every containing contract fact is resultless; with no containing facts returns `false`. fail-closed.
7. `expression_is_contextual_statement_primitive` with `owners` supplied: returns `found` which is `false` if there are no sites (SH:1861). fail-closed.
8. `checked_name_path_segment_target` returns `direct` (possibly valid) when no contextual root can be computed (SH:489-491). The `direct` symbol is the resolver's own result, so not a loosening. **open** on whether `direct` can be stale for specialized machines; `authored_contextual_root` maps instance symbols back to template symbols (SH:517-573) and returns `selected` unchanged on any lookup failure (SH:524-545, 572).
9. `collect_checked_proof_view_call_selections` (SH:2885-3006) finalizes **every** receiverless call in a proof/contract position with no valid `target_symbol` as the `ProofView` intrinsic (SH:2951-2988), keyed on "call has no target and no receiver". **stated** at SH:2867-2884: "Contract-position calls whose spelled target selects no declaration are admitted proof views ... Executable positions never reach this collector". Whether an executable position can reach it via `AssemblyFact` roots (SH:2912-2923, which feed the same collector) was not traced. **open** for the assembly-fact roots.
10. `body_candidates::candidates_for_application` `continue`s when no declaration matches (SH:4685-4687) - that only drops a candidate; consumers decide. **open**.

---------------------------------------------------------------------------

## 3. VA: `values.rs`

### Purpose
Value facts and scalar plans for the check pass (VA:1-16). Records a `CheckedValueFact` per executable expression occurrence (`build_value_facts`, VA:3510), evaluates already-selected scalar operations over supplied values (`evaluation`), reads current assigned values from the fact plan (`snapshots`), serves live interval facts to validation's interval domain (`place_intervals`, `current_integer_bounds`, `captured_*_intervals`), normalizes IEEE comparisons into `FloatPredicate`s (`float_predicates`), and builds reference-capture plans for pointer formation (`reference_sources`). It re-exports a large list from `values_scalar.rs` (VA:3463-3508; that file is not in my unit).

### Shape
- `captured_call_intervals::CapturedCallIntervals` (VA:36; implements `validation::SelectedScalarIntervalSource` VA:111): integer bounds of call arguments as evaluated; `binding` (VA:112), `structural_field` (VA:137); `storage` and `indexed_read` return `None` (VA:129-161).
- `captured_intervals::CapturedIntegerIntervals::bounds` (VA:199): per captured value, intersects bounds from the observation and the value (both: literal or current place bounds), then unions (hull) across alternative values.
- `current_integer_bounds::CurrentIntegerBounds` (VA:346): `at_statement` (VA:385), `selected_read` (VA:407), `selected_path` (VA:427, budget 4096), `new` (VA:516), `rows_position` (VA:537), `at_place` (VA:570), `projected` (VA:594), `exact` (VA:636); `IntegerReadStep {Static, Index}` (VA:379).
- `evaluation` (VA:684-1127): `ScalarValueSource` trait and `BoundScalarValues` (VA:703, 731); `evaluate` (VA:750), `integer` (VA:786), `binary` (VA:932), `boolean` (VA:995), `integer_type` (VA:1080), `representation_view_value` (VA:1089), `admitted_integer` (VA:1110), `integer_magnitude` (VA:1121).
- `float_predicates` (VA:1131-1320): `collect_true` (VA:1162), `literal` (VA:1240), `has_ieee_meaning` (VA:1266), `comparison` (VA:1287), `reverse` (VA:1317).
- `place_intervals::LivePlaceIntervals` (VA:1346): `interval_at_place` (VA:1376), `standing_interval` (VA:1402), `field` (VA:1440), implements `SelectedScalarIntervalSource` (VA:1453: `binding`, `storage`, `structural_field`, `byte_length`, `indexed_read`), `literal_indexed_field` (VA:1595).
- `projection_evaluations::authored_projection_selectors[_below]` (VA:1653, 1664).
- `reference_sources` (VA:1704-2257): `ReferenceCapture {source, storage, backing_type}` (VA:1726), `transported_value_type` (VA:1738), `formed_reference_type` (VA:1765), `capture` (VA:1793), `capture_source` (VA:1820), `access_can_supply` (VA:2144), `qualification_contract` (VA:2157), `qualification_contract_at_site` (VA:2205).
- `snapshots` (VA:2258-3449): `RootAssignmentObservations` (VA:2286; `HashMap<PlaceRoot, RootAssignmentInputBatch>`, lazy per-root), `AssignmentObservationBatch` (VA:2275; eager), `PlaceScalarValues` (VA:2531), `scalar_value_at_place` (VA:2573), `scalar_value_from_payloads` (VA:2592), `literal_at_place` (VA:2627), `integer_bounds_at_place` (VA:2649), `integer_bounds_from_payloads` (VA:2662), `ieee_equal_spelling_at_place` (VA:2719), `payloads_at_place` (VA:2763), `has_completed_call_snapshot` (VA:2792), `is_assignment_literal` (VA:2830), `literal_assignment_payload` (VA:2845), `prepare_assignment_observations` (VA:2869), `immutable_owned_place_root` (VA:2921), `scalar_literal` (VA:2964). Tests VA:2978-3448.
- Top level: `mutable_scalar_parameter_type` (VA:3475), `build_value_facts` (VA:3510).

### Inputs and outputs
- `build_value_facts(program, proof_plan)` -> `CheckedValueFacts`: for every primitive type reference, retains scalar-type refinement predicates (VA:3516-3530); for each executable occurrence visited by `validation::visit_executable_value_occurrences`, records `type_reference`, `primitive_type` and an `integer_range` only in two cases: a literal call argument with an expected primitive (VA:3546-3556) and an assignment value, via `proof::checker::proved_assignment_integer_range_with_context` (VA:3557-3576). Called once (published_facts.rs:10984).
- Interval sources read `FactPlan` contexts, `CheckedOperatorFacts`, `FlowFacts`, and produce `Option<IntegerRange>`.
- `evaluate` reads a `ScalarValueSource` (binding, storage, structural_field) and returns `Option<ScalarValue>`.
- `capture_source` reads typed trees via `validation::*` and produces `ReferenceCapture` or `None`.

### How it decides
- **Value of a place**: all payloads at a place must agree to one scalar (`scalar_value_from_payloads`, VA:2592-2625: first `Unknown`/non-scalar payload -> `None` via `?`; two different values -> `None`). Keyed on **canonical place agreement** (`crate::flow::canonical_places_agree`, VA:2423, 2512, 2740, 2781).
- **Integer bounds**: hull (min of minima, max of maxima) over payloads (VA:2699-2705); within one value, bounds from the two sources are intersected (VA:305-313); `exact` intersects carrier range, assignment snapshot and each retained predicate's consequence (VA:642-680).
- **Completed-call snapshot suppression**: an `AssignedValue` fact whose expression is a `Call` is dropped if another assignment in the same context has the same `place`, `point` and `origin` with a known scalar/bounds payload (`has_completed_call_snapshot`, VA:2792-2823) - keyed on handle identity/point/origin.
- **Literal admission** (`is_assignment_literal`, VA:2830-2843): `literal_bytes(...)` is some, or node kind is Integer/Float/Boolean/String.
- **Float predicates** (`collect_true`, VA:1162-1238): only `F32`/`F64` subjects (VA:1204-1208); needs `has_ieee_meaning` (sealed/builtin selection) and a place with static identity (VA:1221); bound must be a literal (`literal`, VA:1240) or the same place for `==` (non-NaN, VA:1224-1229).
- **reference capture**: keyed on type identities (`stored_structural_type_identity`), access (`access_can_supply`, VA:2144-2150: equal access, or source is `Owned`/`MutableBorrow`), and symbol kind (parameter vs local, VA:2001-2115).
- `integer(expression)` evaluates only the listed `CheckedScalarExpression` variants; `Addr` literal refused (VA:835-837); type mismatch between operands refused (VA:862-878).

### Cases by shape
- `evaluation::integer` explicit refusal list (VA:791-792, 832, 925-928): `StructuralParameterIndexedRead`, `StructuralSequenceLength`, `ErasedParameter`, `ConstantApplication`, `IeeeFloatLiteral`, `ImplementationSelection`, `Boolean(_)` -> `None`. `evaluation::boolean` refusal list (VA:1061-1066): `IntegerMathComparison`, `IeeeFloatComparison`, `ScalarIeeeFloatComparison`, `ByteSequenceEqual`, `ByteSequencePredicate`, `StructuralCaseMembership` -> `None`.
- `representation_view_value` (VA:1089-1108): only 8/16/32/64-bit signed/unsigned targets; others `None`. `admitted_integer` goes through `to_i64`/`to_u64` (VA:1115-1116) so values beyond 64-bit are refused; `IntegerType` supports `i128/u128` in `IntegerValue`, but admission limits to 64-bit.
- `binary` (VA:932-993): all 40 `BinaryKind`s mapped; note the mapping `BinaryKind::SaturatingShiftRight => exact_shift_right` (VA:966-968) and `SaturatingShiftLeft` computing exact shift right first as a validity gate (VA:969-982). Read as written; no comment explains the right-shift gate.
- `CurrentIntegerBounds::at_place`: only when the place contains a `PlaceSegment::Index` is the `projected` hull intersected with the exact bounds (VA:576-592). `projected` / `selected_path` expand every element of the collection, capped by a shared budget of 4096 (VA:583, 422, 600, 615, 492).
- `LivePlaceIntervals::literal_indexed_field` only for `u8` elements of fixed array/slice (VA:1609-1613); `indexed_read` literal refinement only when `primitive == U8` and no element path (VA:1579-1582).
- `capture_source` source plans: `StructuralExpression`, `ScalarParameter`, `Parameter`, `StructuralLocal`, `StructuralLocalBorrow`, `PrimitiveLocal`; scalar parameter capture requires `!is_mutable` and `SharedBorrow` access (VA:2009-2011); const or erased parameter refused (VA:2002-2004); other roots `_ => return None` (VA:2117).
- `mutable_scalar_parameter_type` (VA:3475-3489): `Bool`, `F32`, `F64` or fixed integers only.
- `immutable_owned_place_root` (VA:2921-2962): only `Field`/`FixedIndex` segments, no `Reference` shell crossing, root must be an immutable local declared before the statement.

### Same job elsewhere
- Two assignment-observation readers: `AssignmentObservationBatch` (eager, VA:2275) and `RootAssignmentObservations` (lazy, per-root, VA:2286), plus free-function stream `payloads_at_place` (VA:2763). All three do "payloads at a place whose canonical place agrees, minus completed-call snapshots" (VA:2498-2520, 2410-2449, 2763-2790). Header comment VA:2281-2285 says each operand/epoch "supplies its own reader rather than joining rosters".
- Integer-hull code (`min` of minimum, `max` of maximum) is written inline at least eight times in this file: VA:295-300, 319-325, 504-510, 585-589 (intersection), 625-631, 2699-2705, and similar patterns in 1393-1398, 1496-1501, 1585-1588.
- Float comparison normalization: `values::float_predicates` (VA:1131) vs `validation::float_predicates` (`validation/src/float_and_mathematical_contracts.rs:3326` and `domains_and_predicates.rs:11710 data_float_predicate`), different modules that share the name. Not read.
- `captured_call_intervals::parameter_bounds` (VA:85) and `place_intervals::standing_interval` (VA:1402) both derive the bounds of a state parameter; the first goes through `CapturedIntegerIntervals`, the second through `validation::state_parameter_integer_interval`.
- `reference_sources::access_can_supply` (VA:2144) used at execution_scalar_computations.rs:5830, 7938 and here; one implementation.
- `ScalarValueSource` (VA:703) has implementors `BoundScalarValues` (VA:731) and `PlaceScalarValues` (VA:2538) plus a blanket closure impl (VA:721).

### Unused / one-caller
Grep across `omega-rust/psi`:
- `build_value_facts`: 1 caller (published_facts.rs:10984).
- `BoundScalarValues`: 3 construction sites (checks_contracts_exits.rs:8123, 9440; checks_contracts_prover.rs:6507).
- `evaluate_checked_boolean`: 2 call sites (checks.rs:10482, 10986). `evaluate_checked_scalar`: 1 in PEV:134 (and others from values_scalar, not grepped).
- `AssignmentObservationBatch::prepare_for_root`: 1 call (flow_expression.rs:5792). `AssignmentObservationBatch::prepare`: 1 call (checks_borrows_overlap.rs:1521). `prepare_assignment_observations`: 1 external call (flow.rs:106) + 1 inside `prepare`.
- `RootAssignmentObservations`: 22 occurrences in 4 files other than the definition file's tests: checks_contracts_prover (4), checks_contracts (4), flow_state_values (1), values.rs (13).
- `immutable_owned_place_root`: 2 callers (checks_borrows_overlap.rs:1508, checks_ranges_facts.rs:118).
- `ieee_equal_spelling_at_place`: 1 caller (crash_calls.rs:4508).
- `mutable_scalar_parameter_type`: about 20 uses across execution_*, flow_transfers, values_scalar (see grep).
- `authored_projection_selectors`: 3 calls (execution_terminal_unit_calls.rs:6877; execution_scalar_computations.rs:8450; execution_terminal_unit_control.rs:2226); `_below`: 1 call (execution_scalar_computations.rs:3414).
- `CurrentIntegerBounds|LivePlaceIntervals|CapturedCallIntervals|CapturedIntegerIntervals|BoundScalarValues` outside values.rs: 19 occurrences in 7 files (checks_contracts_prover 1, checks_contracts_exits 6, checks_ranges_facts 1, flow_expression 1, flow_state_values 1, flow_transfers 8, crash_calls 1).
- `PlaceScalarValues|scalar_value_at_place|integer_bounds_at_place|literal_at_place|scalar_value_from_payloads|is_assignment_literal|literal_assignment_payload`: 81 occurrences in 17 files outside values.rs.
- `float_predicates` (values): `comparison`, `has_ieee_meaning`, `literal`, `reverse` imported at checks.rs:960 and (`comparison`, `has_ieee_meaning`, `literal`) at checks_contracts.rs:9366; `collect_true` used at flow_exits.rs:256. `collect_true` is `pub(crate)` with 1 external caller; the module also has a local `macro_rules! keep` (VA:1156-1160) that only forwards to `collect_true` with a `=>`-separated argument list (read; used 4 times VA:1171, 1181, 1182, 1190 - the macro adds no behaviour).
- `reference_sources`: `access_can_supply` 3 call sites (execution_scalar_computations.rs:5830, 7938; + definition use inside `capture_source` VA:1893, 1915, 1939); `capture_source` imported at execution_scalar_computations.rs:7162; `formed_reference_type` imported at execution_scalar_computations.rs:7163 and execution_terminal_unit_control.rs:5881; `transported_value_type` imported at :7164 and :5882; `capture` 1 external (execution_terminal_unit_control.rs:5506); `qualification_contract` 3 external (execution_scalar_computations.rs:9232; execution_terminal_unit_control_statement_sequence.rs:1646; + inside `qualification_contract_at_site`); `qualification_contract_at_site` 2 (execution_terminal_unit_state_graph.rs:6707, 7376); `ReferenceCapture` re-exported at execution_terminal_unit_control.rs:5452.
- `AssignmentObservation` (struct): used in flow.rs:56.
- Pub(crate) items imported from `values_scalar.rs` via `pub(crate) use scalar::{...}` (VA:3491-3508) were not checked for users here.

### Stated limits
- "No source binding, initializer, call, or range premise." (VA:686-687) - the evaluator takes only supplied values.
- "These consequences do not grant initialization, index formation or access." (VA:335-336)
- "Unavailable inventory remains a refusal for the consuming lowerer." (VA:2155-2156)
- "State arrivals may materialize a pointer after their operand frame completes. Retain its supplying type now; lowering must not reopen the source expression." (VA:2202-2203)
- Debug `eprintln!` statements in a `#[cfg(test)]` block (VA:3082-3087, 3243-3248) - not shipped.
- `AssignmentInputWork` counters are `#[cfg(test)]` (VA:2317-2319, 2360-2370).

### Fail-open risks
1. `place_intervals::byte_length` returns the full `u64` carrier range when no literal length is known (VA:1527). That is a wide but sound over-approximation (bounds can only be too wide), and `.or_else` means `None` is never returned for a known `u8`-like path. **traced** (read); sound for upper-bound reasoning, but any consumer that treats the range as a proof of a *lower* bound would only get 0.
2. `literal_indexed_field` falls back to the `U8` carrier range (VA:1641) and `structural_field` falls back to the declared carrier range (VA:1490-1492). Same over-approximation. **traced**.
3. `CurrentIntegerBounds::exact` starts from the primitive carrier range (VA:642) and narrows by snapshots and predicates; with no information returns the carrier, not `None`. Over-approximation. For a non-integer primitive `carrier_integer_range(primitive)?` returns `None`. **traced**.
4. `scalar_value_from_payloads` returns `retained` (None if no payloads) (VA:2624); `FactPayload::AssignedValue` of a non-literal expression gives `scalar_literal` = `None` which short-circuits the whole result via `?` (VA:2606, 2615). fail-closed.
5. `evaluation::boolean` `StructuralIdentity { left, right }` returns `Some(true)` when `left == right` (VA:1016-1018) and `None` otherwise (never `Some(false)`), so the evaluator can prove identity but never non-identity. Equality is on `CheckedStructuralIdentity`-like operand values (not read); what gets through: two expressions that compare equal as trees but denote different runtime values (for example two reads of the same mutable storage at different times) would be reported true. The enum's payload semantic was not read. **open**.
6. `has_completed_call_snapshot` drops a call-valued assignment only when a snapshot row at the same (place, point, origin) exists (VA:2807-2822); otherwise the call-valued `AssignedValue` payload remains and `scalar_literal` gives `None` for it, which makes the whole query `None` (VA:2606). fail-closed. **traced**.
7. `transported_value_type` returns `None` for named homes, i.e. "Named homes retain their existing projected-loan route" (VA:1731-1735). **stated**; it declines rather than accepts.
8. `capture_source` fallback when `calls::projected_evaluated_path` is `None`: accepted only for an `Expression` root with no segments (VA:1860-1872). The returned path claims `declared_type: referent, storage_type: referent` i.e. no crossings checked at that point; subsequent loop (VA:1926-1945) checks type identity. **traced** (read, limited).
9. `qualification_contract` yields `SeparateStorage` for any storage other than `Declared | View` or when `stores_reference_shape` is false (VA:2164-2170) - treats it as not needing source qualification. What gets through: a reference stored in a shape whose qualification should still bind the source. Not traced further. **open**.

---------------------------------------------------------------------------

## Cross-file observations
- Both PEV and VA declare ownership of "the same canonical place algebra" for places; PEV keeps its own `PlaceSegment` enum (PEV:761) distinct from `facts::PlaceSegment`, with conversion functions in both directions (PEV:781-796, 273-288, 649-657, 1733-1739).
- `std::collections::HashMap<facts::PlaceRoot, ...>` at VA:2290, `HashMap<u64, Vec<usize>>` in SH:3430/3460 with `DefaultHasher`, and `BuildSymbolHasher` maps in SH:2016. AGENTS.md says dense IDs should index flat tables; `PlaceRoot` is an enum (symbol or expression handle), so not directly a dense id. `tools/dense_id_maps.txt` was not read.
- Many functions take `&dyn CheckingInput` as `program` and re-derive machine/state by symbol each call (`validation::machine_state_by_symbols`, PEV:1802, 1985, 2084, 2054); counted 6 such lookups in PEV (read).

---------------------------------------------------------------------------

## Coverage
- `published_facts_crash_entry_values.rs`: lines 1-2546 read in full (2546 lines).
- `shared.rs`: lines 1-4805 read in full (4805 lines).
- `values.rs`: lines 1-3589 read in full (3589 lines).
- Skimmed: none. Not read: none. Total read 10940 of 10940. Tests inside the files (SH:3496-3538, 3737-3775; VA:2978-3448) were read but are described only briefly.
- Not read (outside the unit, mentioned only by grep/limited read): `values_scalar.rs`, `published_facts_crash_calls.rs` (home of `field_entry_name`/`member_entry_name`), `validation/src/places.rs`, most `validation::*` helpers. One read outside the unit: `validation/src/calls.rs:8488-8507` and `typed-trees/src/type_system.rs:3708-3720`.

## Five things a maintainer most needs to know about this unit
1. PEV is a hand-written, syntactic "did anything write this place between the binding and the read" scanner (PEV:716-1315) with its own `PlaceSegment` enum. Its default is "interferes"; the exceptions that return `false` are `StatementNode::Assembly` (PEV:1014), `TransitionTargetNode::Terminal` (PEV:1166), and (by shape) value-typed parameters (PEV:1301-1312). `receiver_call_writes` has a constant-false `find` and always answers `true` (PEV:1262-1272).
2. Float bindings skip the write scan entirely: `observation_requires_write_scan` returns `Some(false)` for any float type after `peeled_type_reference` strips references (PEV:2211-2221 with type_system.rs:3708-3720), so only a `mut` binding itself gets scanned for float references. Not traced to a reachable wrong program.
3. `finalize_checked_authored_selections` (SH:973) is the single place that turns late-bound authored selections into checked targets; its target resolution is a long fallback chain (`checked_call_target` SH:338, `checked_operator_target` SH:2174) keyed in part on source spans (SH:412-426), display strings ("Equatable", "equals", "BuildBindingOperations", operation names) and first-match-by-name for payload fields (SH:1795-1808). Unary operators always settle as builtin in the last fallbacks (SH:2648, 1967).
4. `machine_waves` is documented as the scheduler for termination, mutation, crash and reach summaries, but the only outside user is `checks_termination_progress.rs:370/387`; `run_caller_first` has no production caller (`#[allow(dead_code)]`, SH:3636-3637). Parallelism is rayon via `validation::checking_thread_pool()`.
5. `values.rs` has three overlapping readers of "assigned values at a place" (`AssignmentObservationBatch`, `RootAssignmentObservations`, `payloads_at_place`) that apply the same completed-call-snapshot exclusion (VA:2792) and the same place agreement; each has 1 to a few callers. Interval reads always start from the carrier range and narrow, so missing facts widen rather than reject.

## What you are unsure about
- Whether `product_operands` occurrences skipped at SH:1028 are finalized elsewhere or excluded from the final unresolved check (SH:1385-1398).
- Whether `StatementNode::Assembly` can write bindings (PEV:1014), and whether float reference-typed bindings reach PEV:2211.
- Whether the language has overloadable unary operators or overloads of `& | ^ << >> && ||`; SH:2181-2198 and SH:2648 assume none.
- The exact semantics of `CheckedBooleanExpression::StructuralIdentity { left, right }` equality at VA:1016-1018 (payload types not read).
- Whether `subtree_children(.., VALUES, ..)` covers every child that may contain a write (PEV:1246).
- `places.rs:665` `canonical_place_segment_pair_may_overlap` was not read, so I do not know whether PEV `paths_interfere` is equivalent or narrower.
- The orphan comment at PEV:2118-2125 may belong to code that was moved; I could not find its subject in the file.
