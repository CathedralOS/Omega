# Area 13: accepts when it does not know

Read-only trace of the open entries in `.agents/atlas/compiler-2026-11-10/digests/area_13.md`. Citations are the current lines; the validation files are unchanged since `457adf4a11f`. "Read" means the line was opened. "Inferred" means a search hit was not opened.

| # | Site | Verdict | Reason |
| --- | --- | --- | --- |
| 1 | `representation_selection.rs:136` | REACHABLE | A dynamic-trait field is classified as the primitive Selector and accepted as inert storage. |
| 2 | `access_views_and_borrows.rs:27` | UNDECIDED | Several skips emit nothing; plan construction was not read. |
| 3 | `access_views_and_borrows.rs:1415` | UNDECIDED | A missing declared type skips the accessor check; no program was shown that is still an accessor. |
| 4 | `access_views_and_borrows.rs:1444` | UNDECIDED | Binding-private is skipped when both lookups fail; the expression walk was not followed. |
| 5 | `access_views_and_borrows.rs:2993` | UNDECIDED | This function emits no diagnostic; later `None` results were not followed to a diagnostic. |
| 6 | `access_views_and_borrows.rs:3365` | UNDECIDED | Unresolved targets return with no diagnostic; earlier rejection was not read. |
| 7 | `arithmetic_domains.rs:7791` | UNDECIDED | The cited returns were opened; the enclosing functions and their callers were not read in full. |
| 8 | `arithmetic_domains.rs:838` | UNDECIDED | Diagnostics go to a discarded list; whether the same facts are reported on another path was not read. |
| 9 | `arithmetic_domains.rs:795` | UNDECIDED | A missing interval end skips the narrowing diagnostic; a second obligation was not searched. |
| 10 | `arithmetic_domains.rs:6463` | UNDECIDED | An obligation is queued and no diagnostic is emitted; the consumer was not read. |
| 11 | `arithmetic_domains.rs:4975` | CLOSED | The only signed integer primitives are I8, I16, I32, and I64, and `signed_minimum` covers those four. |
| 12 | `arithmetic_domains.rs:5246` | CLOSED | The hard-coded flag is reached only for proof `Int`, which is not a fixed-width signed primitive. |
| 13 | `arithmetic_domains.rs:10885` | UNDECIDED | The round cap keeps the last environment; the transfer function was not read. |
| 14 | `arithmetic_domains.rs:13395` | UNDECIDED | A contradictory environment is replaced; later consumers of `low > high` were not read. |
| 15 | `arithmetic_domains.rs:3170` | UNDECIDED | Callee `ensures` are seeded; `seed_conjunct` and callee-body verification were not read. |
| 16 | `arithmetic_domains.rs:13451` | UNDECIDED | Literals and builtin members are state-independent; the constant-recipe early true was not read. |
| 17 | `arithmetic_domains.rs:15936` | CLOSED | Every comparison caller also compares the offset, which holds the integer or the symbol displacement. |
| 18 | `arithmetic_domains.rs:23738` | UNDECIDED | The skip also fires when no layout plan has an integer field; plan filling was not read. |
| 19 | `borrow_and_reflection.rs:2045` | UNDECIDED | Dynamic-trait nodes carry no lifetime list and their arguments are not walked; whether an argument may be a reference was not read. |
| 20 | `borrow_and_reflection.rs:2877` | UNDECIDED | A missing required lifetime name succeeds; when that name is absent on a real reference was not read. |
| 21 | `call_cycles.rs:2765` | UNDECIDED | Assembly statements skip the rank walk; whether assembly can write a rank parameter was not read. |
| 22 | `call_cycles.rs:2624` | UNDECIDED | A member with no witness contributes no mapping; how a missing mapping is judged was not read. |
| 23 | `call_cycles.rs:620` | UNDECIDED | An out-of-range index becomes "no unranked component"; adjacency construction was not read. |
| 24 | `calls.rs:2062` | UNDECIDED | `encode`/`decode` on a data-type spelling is not reported; earlier rejection of an invalid target was not read. |
| 25 | `calls.rs:3057` | UNDECIDED | A quotient operation returns before argument validation; other quotient checks were not read. |
| 26 | `calls.rs:3171` | UNDECIDED | Build-declaration and exact-embed calls return before later argument checks; those predicates were not read. |
| 27 | `calls.rs:3819` | UNDECIDED | An unresolved argument type skips the property bound; whether that type is already an error was not read. |
| 28 | `calls.rs:4153` | UNDECIDED | Missing types and missing const bindings are skipped; the surrounding functions were not read in full. |
| 29 | `calls.rs:6059` | UNDECIDED | A discarded result is allowed on the first proof machine with the same spelling; name uniqueness was not read. |
| 30 | `calls.rs:2523` | UNDECIDED | Saved subjects, type predicates, and case-membership right operands are not scanned; the effect of the skipped scan was not read. |
| 31 | `calls.rs:1795` | UNDECIDED | An unlisted node counts as composing and can skip recording a scalar root; other recording of that node was not read. |
| 32 | `calls.rs:10863` | UNDECIDED | No prefix evidence calls the resolver with no alias context; those resolvers were not read. |
| 33 | `calls.rs:8844` | UNDECIDED | A shared reference parameter is skipped; the rest of the route and its callers were not read. |
| 34 | `calls.rs:12128` | UNDECIDED | A substitution source whose root is a local is skipped; the enclosing function was not read in full. |
| 35 | `calls.rs:11870` | UNDECIDED | A `Value` transition target returns an empty write list; callers were not read. |
| 36 | `calls.rs:16373` | UNDECIDED | Cast, unary, binary, and match arguments are omitted from the syntactic write set; when that set is final was not read. |
| 37 | `calls.rs:16158` | UNDECIDED | A builtin empty-write flag can satisfy the guard; the rest of the branch and its callers were not read. |
| 38 | `calls.rs:15923` | UNDECIDED | A statement root with no `Call` node gets an empty frame; `expression_has_calls` does not count atomics, and the frame's consumer was not read. |
| 39 | `calls.rs:17528` | UNDECIDED | A dynamic-trait type returns true from the isolation walk; the write-frame caller was not read. |
| 40 | `calls.rs:17681` | UNDECIDED | An alias that matches nothing is returned unchanged; the callers were not read. |
| 41–84 | see digest | NOT REACHED | The cited lines were not opened. |

## 1. `representation_selection.rs:136`

Verdict: REACHABLE.

`CarrierClosure::validate_type` returns success when `primitive_type_reference` is `Some`, before the match that rejects a dynamic trait (read `representation_selection.rs:129-214`). `primitive_type_reference` forwards to `TypeReferenceNode::primitive_type` (read `declaration_tables_and_index.rs:5838-5842`). That method returns `Some(Selector)` for every `DynamicTrait` node (read `type_system.rs:4441-4456`). A checked-shape record used as a carrier can contain a field of that type: `form_opaque_representation_request` requires the carrier root to be one concrete checked-shape data declaration, then calls `validate_inert_carrier` on the representation type (read `representation_selection.rs:354-374`). The walk then accepts the field.

Search `validate_inert_carrier(`: 2 matches, the definition and the call at line 370. Both were read. `collect_opaque_representation_selections` calls `form_opaque_representation_request` (read line 417). `rederive_opaque_representation_selections` calls `collect_opaque_representation_selections` (read lines 467-483) and does not filter field types. `source_checking.rs:630` and `source_compilation.rs:440` were not read; the search shows they call collect or rederive.

The smallest program is a boundary-opaque datum whose representation carrier is an ordinary record with a field whose type is a dynamic trait. That field is accepted as inert storage. The same function's later arm, which says a dynamic trait contains dynamic external provider state, does not run.

The digest understates the site. It is not only "any primitive". `from_name` also treats the names `AtomicBool`, `AtomicU32`, `AtomicU64`, and the three `Atomic*#AccessField<` prefixes as primitives (read `type_system.rs:4648-4675`). Those name matches were not followed to a user-written declaration.

Functions read in full: `validate_type`, `primitive_type` on `TypeReferenceNode`, `from_name`, `form_opaque_representation_request`. `rederive_opaque_representation_selections` was read through the collect call. `source_checking.rs` and `source_compilation.rs` were not read.

## 2. `access_views_and_borrows.rs:27`

Verdict: UNDECIDED.

`validate_agreements` was read in full (lines 27-189). Search `validate_agreements(`: 2 matches. The caller is `validate_plans` at line 1170, called from `program_validation.rs:481-482` with no `complete` gate (read those lines). A missing trait and a wrong argument count are diagnosed by `validate_named_implementation` (read `traits.rs:6462-6498`) only when `complete(inputs)` is true (`program_validation.rs:315` and `454`, read). The toolchain trait `AccessCustody` has an empty body (read `source/library/core/layout.omg:808-809`). A repository search for `AccessCustody` found no other checker.

The continue at lines 100-102 emits no diagnostic when a non-erased schema field has no layout entry. Whether a user program can produce that plan was not read. Plan construction would decide it: if every retained plan lists every live schema field, the continue is unreachable from a program; if not, this entry is REACHABLE.

## 3. `access_views_and_borrows.rs:1415`

Verdict: UNDECIDED.

`access_view_field_for_place` was read (lines 1406-1421). It returns `None` when `declared_place_type` returns `None`. `declared_place_type` and `declared_place_type_raw` were read through line 1394 of `places.rs`. Callers in this file are `validate_expression` (line 1294) and `validate_assignment` (line 1215), both read. A `None` result skips the atomic-permission and binding-private checks. No program was found in which the place is still an accessor after the declared type fails. That example, or a proof that an accessor place always has a declared type, would decide it.

## 4. `access_views_and_borrows.rs:1444`

Verdict: UNDECIDED.

`access_view_field_for_statement_call` was read (lines 1444-1462). Search `access_view_field_for_statement_call(`: 2 matches. The only caller is `validate_statement` at lines 1183-1188 (read). If the call target matches no accessor state and `declared_member_path_type` returns `None`, `validate_binding_private_use` (read lines 1484-1501) does not run. `statement_expression_roots` was not read, so it is unknown whether the same call is also seen as an expression place.

## 5. `access_views_and_borrows.rs:2993`

Verdict: UNDECIDED.

`validate_write_only_fixed_array_range_assignment` was read from line 2921 through 2996. When both bounds resolve and `start > end` or `end > collection_len`, it returns `true` and pushes no diagnostic. Search of the function name: 2 matches. The caller at lines 2762-2777 treats `true` as handled and does not call `diagnose_unsupported_write_only_assignment_target` (read). `canonical_collection_element_place` returns `None` for an inverted range (read `places.rs:5544-5600`). `qualification_storage_type_reference` returns `None` when `end > length` (read `places.rs:8279-8286`). `array_region_representation` returns `None` for the same shape (read `recasts.rs:3522-3536`). The callers of those `None` results were not read, so it is unknown whether the assignment is rejected later.

## 6. `access_views_and_borrows.rs:3365`

Verdict: UNDECIDED.

`validate_opaque_call_inputs` was read (lines 3352-3436). It returns with no diagnostic when the callee has a checked write-only implementation (3365), when type-argument bindings are `None` (3368-3376 and 3406-3408), or when the target is neither a machine state nor a boundary requirement (3416-3418). Search `validate_opaque_call_inputs(`: 3 matches, the definition and the calls at 2829 and 3446, both read. Whether an unresolved target is already a diagnostic was not read.

## 7. `arithmetic_domains.rs:7791`

Verdict: UNDECIDED.

The cited return was opened: `place::analyze` returns `NEUTRAL` when `declared_type` is `None` (read lines 7782-7793). The division gate at lines 5658-5668 runs only when a result primitive is `Some` (read). The `require_representable_result` call at 5717 is inside `if let Some(primitive)` (read 5717-5720). The enclosing functions were not read in full, and their callers were not searched. Those two readings would decide it.

## 8. `arithmetic_domains.rs:838`

Verdict: UNDECIDED.

`check_value_narrowing` was read (lines 806-864). It passes a fresh vector to `validate_value_range` and, when that vector is non-empty, does not call `check_validated_value_narrowing`. `validate_value_range` uses `ArithmeticDecisionSink::Discard` (read lines 15011-15033). Search `check_value_narrowing(`: 4 matches. Callers read: `program_validation.rs:1766` (only when the statement is atomic and `value != established_value`) and `calls.rs:500-531` (the range call there also uses a local vector, then calls `check_value_narrowing`). The texts pushed into the discarded vector, and whether the retaining path reports them, were not read.

## 9. `arithmetic_domains.rs:795`

Verdict: UNDECIDED.

`check_narrowing_assignment` was read (lines 756-804). The fit diagnostic runs only when both interval ends are `Some` (line 795). A missing end returns with no diagnostic and no queued obligation. Search `check_narrowing_assignment(`: 6 matches. The caller at `program_validation.rs:1851-1859` was read; the calls at 2229, 2523, 2938, 888, and 22509 were not. A search for a second obligation on the same store was not done.

## 10. `arithmetic_domains.rs:6463`

Verdict: UNDECIDED.

`require_representable_result` was read (lines 6426-6504). When the interval is not definitely outside the carrier, a recipe causes `integer_range_obligations.push` and `return false` with no diagnostic (6463-6481). The pending-formation branch does the same (6485-6500). Otherwise `report_unrepresentable` runs. The consumer of `integer_range_obligations` was not read. That consumer decides whether a queued obligation can be dropped.

## 11. `arithmetic_domains.rs:4975`

Verdict: CLOSED.

`signed_minimum` was read (lines 4975-4982). It returns a value for `I8`, `I16`, `I32`, and `I64` only. `PrimitiveType` was read (`type_system.rs:4591-4604`): those four are the only signed integer primitives. `from_name` (read 4648-4675) does not add another signed integer. `ProvenDefinedness::from_intervals` sets `signed_pair_excluded` when `signed_minimum` is `None` (read 5023-5033), so the MIN/-1 pair is not demanded for the other primitives. There is no signed integer primitive outside the four, so the `_ => None` arm is not a missed signed type.

The digest's own disagreement note is right. `is_signed_integer` (read `type_system.rs:4722-4727`) is true for `Bool`, `F32`, and `F64` as well, because it is the complement of the unsigned set. `signed_minimum` does not use that predicate.

## 12. `arithmetic_domains.rs:5246`

Verdict: CLOSED.

`validate_proof_integer` was read (lines 5225-5251). It passes `signed_pair_excluded: true`. Search `validate_proof_integer(`: 2 matches. The only caller is `contract_entailment.rs:23774`, inside the divide/modulo arm that requires `mathematical_integer_operands` (read 23767-23777). That predicate requires a proof-integer operand (read 23609-23620). `proof_integer_expression` is true for an exact embed, for arithmetic built from proof integers, or for an expression whose type is the proof builtin `Int` (read 23561-23591). The MIN/-1 pair is defined only for I8-I64 by `signed_minimum` (entry 11). The nonzero-divisor flag is still computed. A fixed-width signed division does not enter this function.

## 13. `arithmetic_domains.rs:10885`

Verdict: UNDECIDED.

`incoming_environments` was read from line 10781 through 10948. The loop is `for round in 0..=states.len()` and, if the environments do not become equal, it keeps `current` and returns it. `ValueEnvironment::join` unions intervals (read 26653-26700), so a stopped iteration is a smaller interval than the least fixpoint. `ArrivalWalk::state` was not read. If one visit already unions the full reachable interval, the cap is harmless; if a loop needs more visits, a too-small interval can later prove that a value fits.

## 14. `arithmetic_domains.rs:13395`

Verdict: UNDECIDED.

`RequirementSeeds::apply` was read (lines 13375-13417). If any interval has `low > high`, the environment is replaced by a fresh one that contains only the requirement conditions. Callers of `apply` were not all read. Whether a later check treats `low > high` as proof of an arbitrary goal was not read.

## 15. `arithmetic_domains.rs:3170`

Verdict: UNDECIDED.

`seed_cited_element_bounds` was read (lines 3127-3183) and `checked_callee` was read (lines 3185-3200). For a checked callee it copies each `ensures` expression through `seed_conjunct`. Search `seed_cited_element_bounds(`: 3 matches (definition, `program_validation.rs:2369`, and `arithmetic_domains.rs:11864`). `seed_conjunct` and the verification of the callee body were not read.

## 16. `arithmetic_domains.rs:13451`

Verdict: UNDECIDED.

`condition_belongs_to_state` was read (lines 13444-13553). It returns true for a valid constant recipe, for `literal_i64`, for a builtin type member, and for integer, boolean, float, and undetermined-NaN nodes. A name belongs only when it is a state parameter or a const/value type parameter (read 13465-13481). Search `condition_belongs_to_state(`: the definition, the recursive calls, and three uses in `RequirementSeeds::new` (read 13283-13299). `constant_recipe_for_contract_expression` was not read. If that recipe can be valid for a fact that is not constant, the fact is seeded into every state.

The digest is right about literals and builtin members. Those two trues are not, by themselves, acceptance of a mismatched state fact.

## 17. `arithmetic_domains.rs:15936`

Verdict: CLOSED.

`integer_bound_terms_equal` was read (lines 15933-15980). Two `Integer` bounds compare equal regardless of value, and two `Symbol` bounds compare equal regardless of `offset`. `integer_bound_offset` returns the integer value or the symbol offset (read 15983-15992). Search `integer_bound_terms_equal`: the definition, two reexports, the import in `checks_borrows_overlap.rs:490`, and three uses. The uses are `structural_bound_is_at_or_before`, `structural_bound_is_strictly_before`, and `structural_bound_equal` (read `checks_borrows_overlap.rs:768-829`). Each one requires the terms to compare equal and then compares the offsets. Different integers, or the same symbol at different offsets, do not compare as ordered or equal on this path. No other use appeared in the search.

The digest describes the helper and does not mention the offset comparison in every caller.

## 18. `arithmetic_domains.rs:23738`

Verdict: UNDECIDED.

`validate_stored_integer_write` was read from line 23721 through 23748. It returns an empty vector when the destination primitive does not accept an integer literal, or when no `plan_laid` layout has an integer field. `accepts_integer_literal` is the fixed integers and `Addr` (read `type_system.rs:4710-4752`). Search `validate_stored_integer_write(`: 2 matches. The caller at `program_validation.rs:1919-1932` extends the output with that vector and was read. When `plan_laid_layouts` is filled was not read. The digest omits the plan disjunct.

## 19. `borrow_and_reflection.rs:2045`

Verdict: UNDECIDED.

`collect_type` returns `true` for `Unit` and `DynamicTrait` without pushing a carried lifetime and without walking arguments (read line 2045, and the function start at 1477). `true` means the frontier is complete (`declaration_lifetime_frontier`, read 1045-1081; `substituted_result_is_view_free`, read 1134-1157, which also requires an empty leaf list). The `DynamicTrait` node has `symbol`, `name`, and `arguments`, and no lifetime list (read `type_system.rs:4374-4378`). Whether a dynamic-trait argument is allowed to be a reference was not read. If it is, a returned dynamic trait applied to a reference is classified view-free. If it is not, the branch is harmless.

## 20. `borrow_and_reflection.rs:2877`

Verdict: UNDECIDED.

In `infer_inputs`, the local `constrain` closure returns `true` when the required lifetime identifier is `None` (read 2870-2878). The closure in `validate` does the same (read 2998-3006). Both functions were not read past those closures' surrounding match. `type_reference_region_applications`, which supplies the `Option`, was not read. That function would show whether `None` occurs for a reference that still has a region.

## 21. `call_cycles.rs:2765`

Verdict: UNDECIDED.

The statement match continues for `Assembly` and `AssemblyFact` (read 2746-2765) after `preserves_rank` has already returned false. The rest of the match, and whether an assembly statement can write a rank parameter, was not read.

## 22. `call_cycles.rs:2624`

Verdict: UNDECIDED.

The witness loop was read (lines 2600-2626). A component member with no source-body witness returns `Ok(None)` instead of a mapping. The same shape at 2719-2721 was not opened in this pass; the search that found 2624 did not include a separate read of 2719. How a `None` mapping is combined with the other members was not read.

## 23. `call_cycles.rs:620`

Verdict: UNDECIDED.

`unranked_component_reached_from` was read (lines 606-636). `seen.get_mut(position)?` and `adjacency.get(origin)?` turn an out-of-range index into `None`. Search of the function name: 2 matches. The caller `checks_termination.rs:8151-8156` was read: `Some` starts a failure, and `None` leaves the diagnostic empty. Adjacency construction was not read. If the adjacency length is always the machine count, the `?` does not fire for a program.

## 24. `calls.rs:2062`

Verdict: UNDECIDED.

`report_unresolved_value_call` returns with no diagnostic when the target text is `encode` or `decode` and any data definition has the receiver's name (read 2062-2068). Search `report_unresolved_value_call(`: 3 matches. Both call sites (3470 and 3548) were read; they run only after `machine_state_by_symbol` fails. Whether an invalid target symbol is rejected by an earlier type check was not read.

## 25. `calls.rs:3057`

Verdict: UNDECIDED.

A call with `quotient_operation` set returns immediately (read 3056-3058), before the argument checks that follow in the same function. The function was not read in full. Other validation of quotient operations was not searched.

## 26. `calls.rs:3171`

Verdict: UNDECIDED.

`admitted_build_declaration_call` and `is_exact_embed_call` each return before the later argument checks (read 3171-3176). Those two predicates were not read, and the enclosing function was not read in full.

## 27. `calls.rs:3819`

Verdict: UNDECIDED.

`validate_type_parameter_instantiation_bounds` was read (lines 3788-3841). When `declared_place_type` returns `None`, the loop continues and the property bound is not checked. Callers were not searched. Whether a missing argument type is already an error would decide it.

## 28. `calls.rs:4153`

Verdict: UNDECIDED.

At 4152-4154, a requirement argument with no declared type is skipped after `validate_requirement_call_arguments` (read 4140-4154). At 4708-4718, a const selection with no binding is skipped and the function can still return `Some(bindings)` (read those lines). The enclosing functions were not read in full, and callers were not searched.

## 29. `calls.rs:6059`

Verdict: UNDECIDED.

`validate_result_use` was read (lines 6040-6076). A discarded non-unit result returns with no diagnostic when `machines().iter().find` hits a machine with no attached data, the same `name` text as `call.target`, that is classified as a proof machine. The match is the first such name, not the resolved symbol. Search `validate_result_use(`: 7 matches, including the definition. The call sites were not read. Whether two machines can share that name text was not read.

## 30. `calls.rs:2523`

Verdict: UNDECIDED.

The scan match was opened at 2522-2523 (`SavedSubject` does nothing), 2701-2702 (a type-predicate binary does nothing), and 2716-2723 (a case-membership binary returns before the right operand is scanned). The enclosing `scan` function was not read in full. What the right-operand scan would have checked was not read.

## 31. `calls.rs:1795`

Verdict: UNDECIDED.

`nested_boundary_applications_compose` was read (lines 1720-1800). The `_` arm of `composes` returns `true`, so an unlisted node is treated as composing without looking inside it. Search of the function name: 2 matches. The only caller is `execution_scalar_computations.rs:10805-10845` (read). A true result sets `argument_roots` and skips `record_root` for a primitive local initializer. Whether that unlisted argument is recorded on another path in the same builder was not read.

## 32. `calls.rs:10863`

Verdict: UNDECIDED.

`with_caller_origin_context` was read (lines 10853-10878). When `collected_prefix_at` returns `None`, it calls `resolve` with `FrameInference::default()` and `None`. Search `with_caller_origin_context(`: 6 matches (the definition and calls at 12573, 12648, 12760, 15627, 15764). Those call sites were not read, so it is unknown whether `None` is treated as a write.

## 33. `calls.rs:8844`

Verdict: UNDECIDED.

The parameter loop was opened at 8828-8845. After the type is known to be a reference, `if !access.is_exclusive() { continue; }` leaves a shared reference out of `origins`. A non-reference returns `Opaque` when `type_may_carry_write_in` is true, and otherwise continues. The enclosing function was not read in full, and its callers were not searched.

## 34. `calls.rs:12128`

Verdict: UNDECIDED.

The substitution loop was opened at 12118-12131. When the place root's name is in `equation.locals` and is not a parameter, the loop `continue`s and does not insert a substitution. The enclosing function was not read in full, and callers were not searched.

## 35. `calls.rs:11870`

Verdict: UNDECIDED.

The transition-target match was opened at 11854-11870. `TransitionTargetNode::Value` returns `Some` of an empty vector. The enclosing function was not read in full, and callers were not searched.

## 36. `calls.rs:16373`

Verdict: UNDECIDED.

`syntactic_call_written_paths` was read from line 16341 through 16380. Exclusive borrows and name, member, and indexed arguments are recorded. A shared borrow is skipped. A struct literal, array literal, or call returns `None`. Every other node, including cast, unary, binary, and match, hits `_ => continue` and is left out of a `Some` write list. Search `syntactic_call_written_paths(`: 5 matches (definition and 11727, 16305, 16394, 27179). Those callers were not read.

The digest is wrong that a `Call` argument is merely not recorded. A `Call` makes this function return `None`. Cast, unary, binary, and match are the shapes that are omitted from a still-`Some` list.

## 37. `calls.rs:16158`

Verdict: UNDECIDED.

The call arm was opened at 16144-16189. A byte-predicate call or `value_builtin_has_empty_write_frame` can enter the branch when several boundary and machine-state lookups are `None`. `value_builtin_has_empty_write_frame` was read (16325-16338): it is true when the call has no receiver and the builtin flag `has_empty_write_frame` is set, or the builtin is a flag-retaining asm intrinsic. The rest of the branch after line 16189, and the callers, were not read.

## 38. `calls.rs:15923`

Verdict: UNDECIDED.

`statement_expression_roots_write_frame` was read at 15916-15928. If no expression root satisfies `expression_has_calls`, it returns a complete empty frame. `expression_has_calls` was read (11319-11328): it is true only for an `ExpressionNode::Call`, not for an atomic node. The caller of `statement_expression_roots_write_frame` was not read, so it is unknown whether an atomic write is recorded on another path.

## 39. `calls.rs:17528`

Verdict: UNDECIDED.

`type_is_caller_isolated_local_inner` was read from 17378 through 17532. `TypeReferenceNode::DynamicTrait` returns `true`. A reference, const expression, or unit returns `false`. Callers of `type_is_caller_isolated` were not read, so the step from this `true` to a missing write-frame entry was not traced.

## 40. `calls.rs:17681`

Verdict: UNDECIDED.

`rebase_local_alias_path` was read in full (17669-17682). If no alias's name equals the path root, it returns the input string unchanged. Search `rebase_local_alias_path(`: 5 matches (definition and 7315, 12097, 34403, 37343). Those callers were not read.

## Counts

| Verdict | Count |
| --- | --- |
| REACHABLE | 1 |
| CLOSED | 3 |
| UNDECIDED | 36 |
| NOT REACHED | 44 |

REACHABLE: 1. CLOSED: 11, 12, 17. UNDECIDED: 2–10, 13–16, 18–40. NOT REACHED: 41–84.

## Entries not reached

41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84.

The cited lines for these entries were not opened. Nothing below is a summary of them.

## What I did not read

- Entries 41–84, including `calls.rs` after the sites named above, all of `completed_storage_and_conservation.rs`, and all of `contract_entailment.rs` except `proof_integer_expression`, `mathematical_integer_operands`, `proof_nat_cast` (opened only at its signature), and the divide/modulo caller of `validate_proof_integer` at lines 23747-23778.
- The callers of `with_caller_origin_context`, `syntactic_call_written_paths`, `rebase_local_alias_path`, and `type_is_caller_isolated`.
- `ArrivalWalk::state`, `seed_conjunct`, `constant_recipe_for_contract_expression`, `type_reference_region_applications`, quotient-operation validation, `admitted_build_declaration_call`, `is_exact_embed_call`, and the consumer of `integer_range_obligations`.
- Plan construction for access-custody layouts, and when `plan_laid_layouts` is filled.
- `source_checking.rs` and `source_compilation.rs`, beyond the search hits that call representation rederivation.
- The call sites of `validate_result_use` other than the definition.
- Whether machine `name` text is unique, whether a dynamic-trait argument may be a reference, and whether assembly text can write a rank parameter.
