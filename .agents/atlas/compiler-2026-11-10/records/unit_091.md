# Unit 91 notes

Files covered: `omega-rust/psi/compilation/validation/src/calls.rs` lines 14740-29478 (14,739 lines). All paths relative to `<checkout>/`.

The range starts mid-function (inside a `CallFrames`-style impl whose `new` is at 14337; the type name is declared before the range) and ends mid-function (inside `captured_inputs::destination_addresses::assignment_addresses`, 29267-29479). It is a slice of one very deeply nested module tree (`machine_calls::calls::write_frames::*`). Every module below is `pub(crate) mod` unless noted. All of it computes "which caller places can a call/statement write" (write frames) plus the reference-origin machinery that feeds it.

Common vocabulary (read, defined in this range): a `FramePlaceOrigin` (place_paths 20464) = a `String` path ("self.a.b", "x[*]", "$P0.f", "invocation result @n:g"), a `FramePathPrecision` (Exact / CollectionCoarse, 20458) and a structured `FrameSourcePlace` (19867: kind, root symbol, segments, representation, recasts, builtin_coordinates). Many judgments use the string path, many use the structured source; see "How it decides" per module.

---

## 1. Tail of the call-frames impl (14740-15500 and 15500-16024)

**Purpose.** Public query methods of the frame facade: given a machine and a statement/call/expression, return a `NormalizedWriteFrame` (complete list of written place strings, or "opaque"), or return reference-origin / binding-stability answers. Most methods are thin: they memoize and forward to `reference_subjects`, `caller_aliases`, `state_write_walk`, `stored_calls`, `demand`.

**Shape.**
- `statement_reference_transfer` (tail 14740-14788): returns `(state, index, prefix, slot transfers)` or a `StatementReferenceTransferRefusal` (`StatementSite`/`EntryPrefix`/`StatementEffects`); traces with `CHECKSITE` when `trace_flags::check_sites()`.
- Memoized-by-pointer queries: `assignment_replaces_local_reference_binding` (14790), `assignment_replaces_reference_binding` (14808), `local_reference_origin_before_statement` (14838), `assignment_write_target` (15112), `assignment_write_frame` (15278), `local_write_origins_before_statement` (15317). The memo key is `(machine.symbol, std::ptr::from_ref(statement).addr())` (e.g. 14797, 15119): statement identity = the address of the `StatementNode` in the table (read).
- `expression_is_fixed_incoming_reference` (14826) and `expression_borrows_fixed_parameter_address` (14861): two methods with byte-identical bodies calling `reference_subjects::expression_is_fixed_incoming_reference` (22314).
- Binding-stability: `expression_reference_bindings_are_stable` (15036), `call_reference_bindings_are_stable` (15062) call `note_dependency_body_read(..BindingStability)` then `reference_subjects::bindings::are_stable_at_site(..).is_some()`.
- `assignment_destination_remains_current` (15143), `captured_assignment_destination_remains_current` (15163), private `assignment_binding_preserved` (15184): ask `assignment_addresses::destination_remains_current` with a closure that checks `stored_calls::binding_preserved_after_transfer`.
- `storage_origin_substitutions_are_finite` (15237): builds `cycle_substitutions::Substitutions` from `(target, source)` pairs and asks `is_finite()`.
- `new` (15337): `Option<Self>`; takes `program.call_frame_tables()` or builds `CallFrameTables::build`; caches come from `frozen_program_memos(program).call_frames` or default.
- `may_write_frame` (15377-15489): per-call frame. Cache key `(machine.symbol.arena_index(), generation(), ptr addr of call)` (15382), uses `statement_calls` mutex cache. Tries, in order: `known_call_written_paths`, `known_boundary_call_written_paths_for_parts`, `known_requirement_call_written_paths_for_parts`, then `conservative_call_written_paths` (unless a divergent actual). Returns opaque if no `MachineSymbols`.
- `expression_may_write_paths`/`expression_write_frame` (15491/15546, memoized by `(machine.symbol, expression)`), `expression_write_frame_uncached` (15801), `operation_write_frame` (15716), `signature_invocation_write_frame` (15558), `statement_value_write_frame*` (15844-15914), `statement_expression_roots_write_frame` (15916), `inferred_state_write_frame` (15955), `inferred_machine_state_write_frames` (15973), `infer_state_write_frame` (15989).
- `declaration_expression_has_empty_write_frame` (15500-15544): `all()` over the expression tree; `Atomic` -> false; `Binary` -> asks the supplied `builtin_operation` closure; `Call` -> true if integer embedding, value builtin with empty write frame, mathematical call, or abstract value call, else true only if the callee state's inferred frame is a complete empty list; `_ => true`.
- `frame_paths_overlap(left, right)` (16026, `pub fn`): string overlap test.
- `statement_value_expression_roots` (16045): forwards to `value_custody::access_views::statement_expression_roots(.., true)`.
- `collect_expression_call_written_paths` (16053): post-order walk over `executable_children`, short-circuits on the first `None`, then `collect_operation_written_paths` for the node.
- `collect_operation_written_paths` (16100-16323): per-node: Atomic-once operands -> adds the written place path expanded through aliases/stored origins (16112-16142); Call -> large decision (see below); other nodes `_ => {}` (16320).
- `value_builtin_has_empty_write_frame` (16325), `syntactic_call_written_paths` (16341), `conservative_call_written_paths` (16383).

**Inputs/outputs.** Read `CheckingInput` (program tables), `TopLevelSymbols`, `MachineSymbols`, `FrameInference`, `CallOriginContext`. Output is `NormalizedWriteFrame` (`complete(paths)` or `opaque()`), `Option<Vec<String>>`, bools. The mutable parts are the memo mutexes and `FrameInference` (`&mut`).

**How it decides.**
- Whole-call write set is chosen from a fixed fallback ladder (15459-15478 and 16275-16313): exact known-call paths, boundary-call paths, requirement-call paths, syntactic over-approximation. Syntactic fallback is skipped when an actual mentions a divergent alias (`has_divergent_actual`, 15432/16257) or the receiver is not exact (16302).
- 16158-16193: a call is treated as write-free if it is a byte-predicate call or a value builtin with `has_empty_write_frame()` AND it is not a boundary signature, not a boundary-frame receiver, not a machine state, has no requirement signature, and the receiver does not require a boundary frame. Keyed on builtin function identity (`builtin_function_for_symbol(call.target_symbol)`, 16332) and `asm_intrinsic_retains_flag_place(builtin.name())` (a name-keyed test, 16335).
- 16145: `expression_call_only_observes_operands` -> no writes.
- Receiver: `receiver_member_chain` -> `exact_receiver` (16194). Inexact receivers are rejected (`return None`, 16207) unless the target is a machine state, has a requirement signature, or the receiver expression is a Call/Match.
- `receiver_frame_origins` -> `(members, origins, data_name, authority_receiver)`.
- `signature_invocation_write_frame` (15602-15701): needs a `ScalarAccessorDeclaration::authenticate`, captured origins for receiver and arguments; refuses `TransitionNamed` sites (15598); if the footprint is empty returns it even when other origins are incomplete (15676-15681, stated comment: "Unrelated incomplete origins must not turn that proved empty set opaque."); otherwise requires `references_complete`.
- `inferred_state_write_frame` (15960): opaque if the state is not one of `machine_states(machine)`.
- `infer_state_write_frame` (15989): `summarize_state_written_paths` or the cycle variant; then each relative path through `normalize_state_relative_path`: `Some(Some)` keep, `Some(None)` drop (a local), `None` -> opaque.
- `frame_paths_overlap` (16026-16043): pure string compare: equal; or one ends in `[*]` and the other starts with that collection prefix followed by `[`; or one is a prefix of the other followed by `.` or `[`. Keyed on source spelling.

**Cases by shape.**
- `syntactic_call_written_paths` match (16364-16374): Borrow(exclusive)->target; Borrow(shared)->skip; Name/Member/Indexed -> the argument itself (even for by-value arguments); StructLiteral/ArrayLiteral/Call -> `return None`; `_ => continue` (Cast, Unary, Binary, Match, literals are not counted as written).
- `collect_operation_written_paths` Call branch handles receiver forms by a fixed list: exact member chain, Call or Match receiver (16202-16205); anything else -> `None`.
- `declaration_expression_has_empty_write_frame`: `Atomic -> false` (15513), `_ => true` (15541).
- `aggregate_arguments` limits are in section 7.

**Same job elsewhere.**
- 14826 vs 14861: duplicated method (same body).
- The "walk a statement/expression and collect callee write paths" job is also done in `state_write_walk::walk_state_write_prefix_body` (26934-26996 calls `collect_expression_call_written_paths`, and 27068-27200 has its own copy of the nested-call ladder known/boundary/requirement/syntactic that is the same ladder as 15459-15478 and 16275-16313). Three copies of the ladder (read).
- Dedupe-by-`contains` push of paths appears inline at 16315, 18746, 19478, 26991, 27196, 27253 instead of one helper (read; `push_visible_frame_path` at 26114 is one helper for part of it).

**Unused / one-caller (grep).**
- `frame_paths_overlap`: `Grep frame_paths_overlap` count mode across `omega-rust` = 28 occurrences in 12 files (calls.rs 5). Widely used.
- `expression_borrows_fixed_parameter_address`: 2 occurrences (this definition + one in `05_typed-trees-to-checked-trees/src/flow.rs`), so one outside caller.
- `expression_is_fixed_incoming_reference`: 6 occurrences (5 in calls.rs incl. both definitions, 1 in `checks_contracts_prover.rs`).
- `syntactic_call_written_paths|conservative_call_written_paths`: 9 occurrences, all in calls.rs (call sites seen: 16305, 16394, 27179; conservative at 15471).

**Stated limits.** None quoted in this part beyond 15676 comment.

**Fail-open risks.**
- `syntactic_call_written_paths` `_ => continue` (16373) and shared-borrow `continue` (16366): an argument that is a `Cast` (e.g. a recast of an exclusive borrow), `Unary`, `Binary` or `Match` is not recorded as a written place. A call whose argument is such an expression gets a write set without it. Status: open (no comment; I did not trace whether the type checker prevents exclusive borrows under these nodes).
- 16158-16193 write-free early return for value builtins and byte predicates: accepts if the builtin flag says empty frame and none of the boundary guards fire. Status: open (flag semantics are in another file).
- `declaration_expression_has_empty_write_frame` `_ => true` (15541): any non-Atomic/Binary/Call expression node is treated as empty-write; plausible because those nodes have no calls themselves, but children are traversed by `collect_expression_tree` first. Status: traced for that function only (inferred children are included).
- `signature_invocation_write_frame` returns `Some(empty)` ahead of the completeness check (15679): stated.
- `statement_expression_roots_write_frame` (15923-15928): complete empty frame if no expression "has calls" (`expression_has_calls`); atomic-once writes are only caught in the expression variant (15809-15819 checks `atomic_once_carrier`), not in the statement variant. A statement root that is an atomic-once operation with no calls gets an empty frame at 15923. Status: open (I did not trace whether atomic operations count as "calls" in `expression_has_calls`).
- none else seen.

---

## 2. `inference` module (16404-17109)

**Purpose.** `FrameInference`: per-walk mutable state carrying which states are active (cycle guard), recorded local origin shapes, and known boolean/integer values of state parameters while following a specific invocation. `ParameterValues` and `BoolTruth` evaluate guards and constant indices.

**Shape.** `FrameInference` (16425: `active_states`, `local_shapes: Arc<Vec<StoredLocalOrigins>>`, `parameter_values: Option<ParameterValues>`, shared `slot_effects` mutex cache). Methods: `with_invocation` (16581), `with_dynamic_selectors` (16609), `with_state_values` (16638), `without_values` (16657), `with_local_scope` (16474), `specialize_segments/source` (16523/16538), `finish_statement` (16664). `ParameterValues` (16677: `state`, `entry_values`, `values`, `entry_indices`, `indices`, `selectors`). `BoolTruth` (16911) with `negate/equal/and/or`. `bool_truth` (16917), `constant_index` (17000). `CompletedSlotEffects = std::collections::HashMap<SlotEffectSummaryKey, Vec<SlotEffect>>` (16421; key `(SymbolHandle, Option<ParameterValues>)`).

**Inputs/outputs.** Reads parameter types, `binding_is_exclusively_exposed`, `binding_has_write_effects`, `binding_is_written`, expression table. Mutates itself. Tests at 17014-17108 (3 unit tests about snapshot sharing).

**How it decides.**
- A parameter value is tracked only if it is a `Bool` that is not exclusively exposed and has no write effects (16698-16714), or a fixed-integer non-reference not exclusively exposed and not written (16731-16745). Keyed on exact primitive type and binding observations.
- `with_invocation` keeps indices only if the state's named transition subgraph is acyclic (16589-16596) and installs parameter values only if `has_known_values()`.
- `bool_truth` (16926-16963): Boolean literal, `!`, Binary only if `has_builtin_parameter_bound_expression_meaning` holds, Name via the values table, everything else Unknown.
- `finish_statement` (16837-16907): tracks `LocalData` of role Value (or boolean DispatchSubject) that are relevant, non-reference, not exclusively exposed; assignments update only names already tracked with `Some`.

**Cases by shape.** `bool_truth` binary operators only Equal/NotEqual/And/Or (16948-16953). `constant_index` only `Integer` literals (17007).

**Same job elsewhere.** `with_invocation` (16581-16607) and `with_dynamic_selectors` (16609-16636) are near copies (acyclicity filter + replace parameter_values). `FrameInference::index_value` (16512) and `ParameterValues::index_value` (16778) overlap. `selected_sources::ResultSelection::new` (24070) constructs `ParameterValues` separately.

**Unused / one-caller.** `with_dynamic_selectors|known_dynamic_implementation` combined grep count = 15 occurrences, all in calls.rs; `with_dynamic_selectors` has one call at 14393 (read via grep).

**Stated limits.** None.

**Fail-open risks.** `BoolTruth::Unknown` is the default for anything not understood; in the consumer `reachable_value_parameters_body` (24307-24310) `truth != False` keeps the edge, so unknown keeps more reachable sources (conservative direction for write sets). `argument_values` returns `Unknown` when the state cannot be resolved (16819-16821). Traced: Unknown never removes a path. None seen as fail-open for acceptance.

---

## 3. `isolation` module (17110-17654)

**Purpose.** Decides whether a type is "caller isolated": it holds no reference or borrow, so a callee writing it cannot reach caller storage. Also compares aggregate storage types and matches struct literals to expected types.

**Shape.**
- `with_isolation_cache` (17127): memo in `program.with_isolation_verdicts(structural_fingerprint, ..)`.
- `definitions_for_symbol` (17134), `definitions_for_name` (17145: by type NAME string via `data_definition_positions_for_name`).
- `type_identities_match` (17156), `aggregate_storage_types_match` (17192, `pub fn`, cached both directions 17205-17208), `aggregate_storage_types_match_in` (17212).
- `type_is_caller_isolated` (17321, cached key `(handle, proof_value, bindings)`), wrappers `_local` (17348), `_local_in` (17355), `_proof_value` (17363), `local_storage_is_caller_isolated` (17370), `type_is_caller_isolated_local_inner` (17378), `data_definition_is_caller_isolated` (17576), `struct_literal_type_is_caller_isolated` (17535), `struct_literal_field_type` (17286), `struct_literal_matches_expected_type` (17303), `concrete_nominal_type` (17177).

**How it decides.**
- Isolated: primitives (17402); builtin atoms `UInt`/`Int` (17437); data definitions whose every field and variant payload is isolated (17622-17648); `DynamicTrait` -> true (17528); `Reference`, `ConstExpression`, `Unit` -> false (17529-17531).
- Named types: if the symbol is valid look up by symbol, else by name string (17443-17447). Requires exactly one definition (17448-17453) and no type parameters.
- Generics: in `proof_values` mode all arguments must be isolated and parameter symbols are tracked in `isolated_parameters`; in ordinary mode binds arguments through `push_generic_application_bindings` (17516).
- Recursion: a type already in `visiting` (or storage-matching one in `visiting`) returns `proof_values` (17617): recursive types are not isolated in ordinary mode, isolated in proof-value mode.
- `proof_values` requires `supply_mode == DataSupplyMode::CheckedShape` (17604).
- `aggregate_storage_types_match_in`: equal under simultaneous substitution -> true (17219-17231); normalized identity equal -> true; same generic base with pairwise matching args; else same non-generic nominal symbol with at least one definition with no type parameters (17279-17283).
- `struct_literal_type_is_caller_isolated` (17535): by `literal.type_name.as_str()`; requires a single definition; "unique_shape" (no variant members, or exactly one variant of that case name, matched by `variant.name == *case_name`: name compare).

**Cases by shape.** `type_is_caller_isolated_local_inner` ends in an exhaustive `TypeReferenceNode` match (17411-17531). Generic proof-values path requires every parameter to be a `TypeParameterKind::Type` child of the definition (17476-17485).

**Same job elsewhere.** `type_is_caller_isolated_local` is used from `place_paths`, `reference_origins` (21171, 21184, 21217), `referent_has_only_owned_storage_in` (21273-21276), `reference_subjects` (22777), `instantiate_written_path_for_origins` (19442). `aggregate_storage_types_match` has 13 occurrences in calls.rs + 1 definition-ish in `typed-trees/src/type_identity.rs` (grep count 14; the typed-trees hit is a different identity helper, suspected not read).

**Unused / one-caller.** `with_isolation_cache` is used at 17131-17342 (several). `struct_literal_field_type` (17286) not grepped for external use; callers not counted (open).

**Stated limits.** None.

**Fail-open risks.**
- `DynamicTrait => true` (17528): a type containing `dyn Trait` is treated as holding no caller-reachable reference, so writes through it do not appear in write frames. No comment. Status: open (I did not trace how dyn values are represented).
- `UInt | Int` builtin atoms -> true (17437-17441): fine, scalars.
- Cache key includes `bindings`; the cache lives in the program-level `IsolationVerdicts` keyed by structural fingerprint: not traced.

---

## 4. `local_aliases` (17655-17985) and `parameter_aliases` (17986-18095)

**Purpose.** Resolve alias names to origins and detect expressions that re-borrow (exclusively) a reference-typed binding, which makes write tracking unsound unless handled.

**Shape.** `rebase_local_alias_path` (17669), `stable_alias_place_origins` (17684), `expression_reborrows_unresolved_reference_binding` (17738), `expression_reborrows_reference_binding` (17763), `expression_has_exclusive_borrow` (17776), `statement_mentions_place_roots` (17810), `validate_divergent_projections` (17829), `expression_mentions_place_roots` (17910), `expression_may_rebind_mutable_alias` (17945). `parameter_aliases::ParameterRelativeFrameOrigin` (17996: `place`, `parameter_symbol`), `expression_reborrows_transparent_alias_binding` (18001).

**How it decides.**
- Aliases are looked up by NAME STRING equality: `(alias == root)` (17677, 17709), `parameter.name.as_str() == root` (17700), `local == root` (17703), `roots.iter().any(|name| name == root.as_str())` (17820), `*root == split_place_root(&place.path).0` (17923). Root string comes from the path string via `split_place_root` (find first '.' or '[', 24524... actually 20524).
- `expression_may_rebind_mutable_alias` (17945-17984): Borrow -> true; Call/Name/SavedSubject/Member/Indexed/Cast -> `expression_result_is_reference(..).unwrap_or(true)` (fail toward "may rebind"); literal and operator nodes -> false.
- `validate_divergent_projections` (17829-17907): invalid if a Member/Indexed over a divergent root whose result is not provably non-reference (`projection() != Some(false)`, 17863), or a Name whose source has segments and a declared local reference carrier whose projected type is unresolvable (17905).
- `parameter_aliases::expression_reborrows_transparent_alias_binding`: exhaustive node match; only Borrow nodes are positive: exclusive access of a `Name` with empty suffix that is a reference-typed parameter (by symbol+self receiver or by name, 18030-18036) or an alias whose local declaration is reference-typed or missing (`is_none_or`, 18056).

**Cases by shape.** `expression_has_exclusive_borrow` (17776) traverses everything with an explicit stack; `Match` handled by `push_children_reversed` and other nodes by `children`.

**Same job elsewhere.** Two near-duplicate traversals for "exclusive reborrow of a reference binding": `local_aliases::expression_has_exclusive_borrow` (17776, string-keyed alias lookup via closure) and `parameter_aliases::expression_reborrows_transparent_alias_binding` (18001, recursive, name+symbol keyed). Both return bool for a similar question with different alias representations (read).

**Unused / one-caller.** Not grepped individually.

**Stated limits.** None.

**Fail-open risks.** `parameter_aliases` 18056 `is_none_or`: an alias whose symbol is invalid or whose state/local cannot be found counts as a reborrow (fail-closed direction). `rebase_local_alias_path` (17669-17681): when no alias matches, returns the original relative path unchanged (`unwrap_or_else(|| relative.to_owned())`): the path is then treated as if rooted at itself. Status: open (callers in other files).

---

## 5. `parameter_relative_origins` (18096-18675)

**Purpose.** Express an expression's place origin relative to the caller's parameters (or aliases), including through call results, so a callee's returned reference can be rebased onto caller arguments.

**Shape.** `ParameterRelativeQuery` (18120), `parameter_relative_place_origin(s)` (18146/18156), `parameter_relative_place_or_carrier_origins` (18164), `parameter_relative_place_origins_in` (18172), `parameter_relative_declared_origins` (18308), `parameter_relative_aggregate_leaf_origins` (18363), `parameter_relative_call_result_origins` (18398), `push_unique_parameter_relative` (18641), `single_parameter_relative_origin` (18653).

**How it decides.**
- Expression shape match (18195-18303): Borrow, Indexed (marks CollectionCoarse), Member (appends `.member` to path only if Exact; falls back to aggregate leaf origins), String (static image storage), Name/SavedSubject, Call, Match (union over arms), recast `Cast` (only `cast.form.is_recast()` and non-effectful operand); `_ => return None`.
- `parameter_relative_declared_origins`: root symbol must equal a parameter symbol (and be reference-typed unless `admit_carrier_roots`), else alias by exact symbol (18334-18338).
- Call results: requirement applications use `closed_requirement_provider_associations`; otherwise `machine_state_by_symbol`; refuses when `call.receiver.is_valid() != attached_data_definition(callee).is_some()` (18498-18502); bodyless callee resolved through `requirement_satisfiers`: exactly one satisfier is used, none -> boundary `bodyless_callee_result_origins`, more -> `None` (18508-18538). Result origins are rebased through `transparent_callee_result_origins` with `inference.with_invocation`.
- `single_parameter_relative_origin` (18653): all candidates must share parameter_symbol and path; precision becomes CollectionCoarse if any is coarse or sources differ; otherwise `None`.

**Cases by shape.** The fixed list at 18195-18303; calls only with finite alternatives; satisfier count "exactly one" (18508-18538, `[satisfier]` / `[]` / `_`).

**Same job elsewhere.** `single_parameter_relative_origin` (18653) vs `place_paths::single_place_origin` (20504): the same merge with different equality tests. `push_unique_parameter_relative` (18641) vs `place_paths::push_unique_origin` (20492). `same_place_origin` (20485) is literally `first == second`. Grep `same_place_origin|single_parameter_relative_origin|single_place_origin` count = 15 occurrences, all calls.rs. `reference_origins::exclusive_reference_origins` (20817) overlaps in purpose with `parameter_relative_place_origins_in` (Name/Member/Match/Call shapes) with a different alias/parameter model.

**Stated limits.** None.

**Fail-open risks.** None seen: failures return `None`. (`(!origins.is_empty()).then_some(origins)` at 18305 refuses empty.)

---

## 6. `path_instantiation` (18676-19489), including `aggregate_arguments`

**Purpose.** Turn a callee's relative write path ("self.x", "$param.f") into caller paths by substituting the actual arguments; and for aggregate arguments (struct/array literals, call results by value) compute which reference leaves inside them can be written.

**Shape.**
- `aggregate_arguments` (18686): `written_paths` (18705), `ReferenceSource` (18754: `Symbolic(FramePlaceOrigin)` | `Captured(CapturedReferenceSource)`), `CapturedReferenceSource` (18760), `partition_sources` (18802), `ReferenceLeaf` (18826, `#[derive(Clone, PartialEq, Eq)]`), `AggregateOrigins` (18836: references, cases, moves), `AggregateMove` (18843), trait `ArgumentOrigins` (18864: `reference`, `stored`, `call_result`, default `project`), `SymbolicArguments` (18906), `aggregate_value` (18952), `reference_leaves` (18980-19381).
- `instantiate_written_path` (19385) / `instantiate_written_path_for_origins` (19416-19488).

**How it decides.**
- `instantiate_written_path_for_origins`: root "self" -> receiver base; root equals a parameter NAME (19438); by-value isolated parameter -> `Some(vec![])` (19441-19445); literal arguments or by-value Call arguments of non-isolated type -> `aggregate_arguments::written_paths`; else origins from the passed `argument_origins` or `transparent_place_expression_origins`; root equal to a local NAME (19484) -> no writes; otherwise `None`.
- `reference_leaves` is a work-list over `(expression, type, suffix, local_suffix, local_segments, local_coarse, scope)`: Match arms, scalar case constructors, struct literals (fields matched by name string `field.name == name` and checked against `any_field_owner` path identity, 19304-19321), fixed arrays (only `FixedArrayLength::Literal` with an array literal of exactly that length, 19209-19232), reference-typed leaves via `inputs.reference`, constrained base, primitive/Unit/DynamicTrait leaves (require empty suffix).
- Calls/names of aggregate type are resolved via `aggregate_value` and then filtered by `demand_is_declared` and `place_suffix` (19100-19146).

**Cases by shape.** 19377 `_ => return None` for any type node not listed (e.g. Slice, non-literal-length FixedArray, Generic-with-primitive...). Quote 19084-19085: "Carry a call's complete result into its receiving field. A formed view can retain several possible backing places." Struct literal with `case_name` requires `selected_construction_definition/case` and exactly one variant matching `path_identity` (19267-19282). Non-variant literal over a definition that has variants -> `None` (19296-19302).

**Same job elsewhere.** `ArgumentOrigins` has two implementors: `SymbolicArguments` (18911) and `Inputs` (29871, in `stored_calls::captured_inputs`, grep `impl.*ArgumentOrigins for`). `ReferenceSource::Captured(` appears 16 times in calls.rs.

**Unused / one-caller.** `instantiate_written_path` (19385, convenience wrapper that builds a receiver origin and passes `None` argument origins): not grepped (open).

**Stated limits.** None quoted except the comment above.

**Fail-open risks.**
- 19441-19445: by-value parameter of caller-isolated type -> no writes. Accepts that nothing is written through that argument; a wrong program would need a callee writing through a value type judged isolated incorrectly (see `DynamicTrait => true`, section 3). Status: traced to `isolation`; the weak point is the isolation verdict.
- 19484: a root that is a local of the callee returns no writes (stated by nature: locals are not visible).
- 19367-19376: primitive/Unit/DynamicTrait leaf with empty suffix is accepted without leaf origins. Status: open.

---

## 7. `place_paths` (19490-20631)

Contains nested `scalar_coordinates` (19494), `source_places` (19805), `write_coordinates` (20316) then module-level helpers.

**Purpose.** Place identity: canonical coordinates for scalar storage through retained recast views; `FrameSourcePlace` source model; string path building (`project_write_path`, `coordinate_storage_path`); `FramePlaceOrigin` helpers.

**Shape.**
- `ScalarStorageCoordinate` (19501, public), `StorageCoordinate` (19510), `LocalWriteSource::{write_storage_coordinate (19542), scalar_storage_coordinate (19601), scalar_storage_coordinate_result (19609), storage_coordinate (19645)}` (an impl of `LocalWriteSource` hosted here), `retained_view_predecessor` (19760), `source_root_type` (19785), `scalar_leaf` (19795).
- `FrameSourceKind` (19817: Declaration, ImageStorage, InvocationOutput(identity), InvocationResult{call, referent}), `ReferenceViewFormation` (19829: Expression | ExtentView), `RecastSource` (19857), `FrameSourcePlace` (19867, with `invocation_output`, `invocation_result`, `parameter_root`, `image_storage`, `from_expression` 19954, `recast` 19980, `append_segments` 20034, `append_source` 20050, `projected` 20054, `append_relative` 20098, `substitute_source` 20110). Free fns `source_coordinates` (20176), `source_has_builtin_coordinates` (20256), `source_owner` (20266), `segments_have_index_geometry` (20164).
- `write_coordinates`: `captured_write_scope` (20322), `project_write_path` (20343).
- Top level: `FramePathPrecision` (20458), `FramePlaceOrigin` (20464, with `rebase_invocation_home` 20471 that writes `format!("invocation result @{}:{}{suffix}", arena_index, generation)`), `same_place_origin` (20485), `push_unique_origin` (20492), `single_place_origin` (20504), `split_place_root` (20524), `append_place_suffix` (20529), `coarse_place_path` (20533), `frame_place_path` (20540), `raw_frame_place_path` (20565), `frame_coordinate_path` (20590, pub), `frame_length_coordinate_path` (20599, pub), `coordinate_storage_path` (20613).

**How it decides.**
- `coordinate_storage_path` (20613-20630): fields append `.name` (name from `program.symbols.name`), `Case` segments add nothing, ANY other segment (index/range) -> returns path so far with `CollectionCoarse`. So case-variant payloads and exact indices are not distinguishable in the string path.
- `raw_frame_place_path` (20565): Indexed -> CollectionCoarse on the collection path; Member appends `.member` only if receiver Exact; otherwise `arithmetic_domains::place_path(program, expression)` as Exact.
- `project_write_path` (20343-20448): if no recasts, Exact -> `base+suffix`; CollectionCoarse with builtin coordinates and index-geometry segments -> `path[*]` else the bare path; with recasts: external-content (ExtentView or `ResidentAccess` formation) -> `"{path}[*]"` or coarse path; else try the exact scalar label (`scalar_storage_coordinate(..).label`), fallback to the coordinate storage path of the first recast's captured write scope. Judged on formation interpretation and `builtin_coordinates`.
- `storage_coordinate` (19645-19757): follows recast chain backwards; for a recast with a `FixedRange`, uses `recast_scalar_source_leaf_in_array_region`; otherwise if `reference_view_storage(..) == Declared` appends the path, else `recast_scalar_source_leaf`; tracks `preserves_scalar_value`, `preserves_primitive_bits`, `replaces_place`. Many `Err(&'static str)` refusal reasons, e.g. 19653 "opaque invocation result carries a retained view".
- `retained_view_predecessor` (19760): refuses access views (19769) and requires carrier == recast.target_type == formation.view_type (19772).
- `FrameSourcePlace::recast` (19980-20032): skips adding a recast when the formation is a `RepresentationView` and source and target type identities are equal.
- `substitute_source` (20110): returns `Self::default()` (an invalid source) if either side lacks a valid root/static/output/result home.
- `source_owner` (20266-20314): by symbol kind Parameter/Local (retained in state), Machine/Field (exact attached field), else `None`.

**Cases by shape.** `storage_coordinate` handles `Field`, `FixedIndex` leaf segments from the recast translation (19729-19737). `captured_write_scope` keeps only the leading Field/Case segments (20332-20337). `source_coordinates` special-cases `SavedSubject` anchors and range selectors (20181, 20222-20253). `FrameSourcePlace::from_expression` sets `builtin_coordinates = !segments_have_index_geometry || source_has_builtin_coordinates` (19968).

**Same job elsewhere.** `source_root_type` exists twice: `scalar_coordinates::source_root_type` (19785: `state_root_type` then `declaration_type_reference`) and `reference_subjects::source_root_type` (23172: state parameters, or a Local with declaration ordinal < index). Different rules for the same name. `frame_place_path` strings for locals vs `coordinate_storage_path` for coordinates are two path spellings of the same place (20547-20557 chooses between them when the raw root differs from the source root). `same_place_origin` is `==`.

**Unused / one-caller.** `frame_length_coordinate_path` and `frame_coordinate_path` are `pub fn`; not grepped (open). `same_place_origin` + two single_* helpers: see section 5.

**Stated limits.** Refusal strings, e.g. "physical reference view has no exact scalar storage translation" (19724).

**Fail-open risks.**
- `coordinate_storage_path` collapses unknown segment kinds to CollectionCoarse (20626): coarse = over-approximate, fails closed for writes.
- `project_write_path` with CollectionCoarse and no builtin coordinates returns the bare path (20369), i.e. does NOT add `[*]`. Consumers comparing with `frame_paths_overlap` treat a bare path as overlapping any `path.` or `path[` descendant (16037-16042), so it still covers children. Status: traced within the range.
- `FrameSourcePlace::default()` returned on any failure (e.g. 20038, 20065, 20115) yields an invalid-root source; consumers check `root.is_valid()` in many places but not all (not traced).

---

## 8. `reference_origins` (20632-21279)

**Purpose.** Find where an exclusive reference expression points (receiver or argument): through declared places, carried references in owned storage, match arms, and call results.

**Shape.** `retained_receiver_projection` (20649), `receiver_frame_origins` (20684), `receiver_referee_name` (20745), `exclusive_reference_origin(s)` (20801/20817), `projected_carrier_reference_origins` (20940), `carried_reference_origin` (21025), `declared_origin_root` (21075), `exclusive_reference_has_owned_storage` (21120), `exclusive_reference_referee` (21128), `owned_receiver_origin` (21143), `exclusive_reference_binding_path` (21243), `referent_has_only_owned_storage(_in)` (21255/21262).

**How it decides.**
- `receiver_frame_origins` (20684-20743): receiver judged by: `call_has_no_runtime_receiver_for_target` or authority receiver -> member chain with no origins; place path or owned receiver origin; Call/Match receivers -> `exclusive_reference_origins` and `receiver_referee_name`. The members vector is `origin.path.split('.')` (20719): path spelling.
- `receiver_referee_name` (20745-20799): returns the referent's `Named` type NAME string; for Match, all arms must agree on that string (20789-20792). Name-compare.
- `exclusive_reference_origins`: reference formation source must be exclusive; effectful sources refused; Name via binding path or carried origin; Member/Indexed via carried or projected-carrier origins; Match union; Call via `transparent_call_result_origins` with a closure validating the parameter referee and binding type parameters via `bind_formal_type`; `_ => return None` (20935).
- `projected_carrier_reference_origins`: Member chain over a Call result (case-variant members refused, 20952), callee return type must not be a reference (20965), field walked by member NAME in the data definition (20981).
- `declared_origin_root`: Name whose `head_symbol == machine.symbol` with members first `self` (20084-20092 region at 21084-21092), else `caller_name_root_type(...).is_some()`; SavedSubject; Member/Indexed recurse to receiver/collection.
- `referent_has_only_owned_storage_in` (21262): Constrained, Slice -> element isolated, Unit -> valid, else isolated.

**Cases by shape.** Exclusive-reference origin forms: Name, Member, Indexed, Match, Call only (20841-20935). `owned_receiver_origin` (21150-21202): Borrow(exclusive) of Name/Member/Indexed, Name, Member, Indexed, Call; `_ => return None`.

**Same job elsewhere.** `parameter_relative_origins` (section 5) resolves similar expression shapes with alias lists; `reference_subjects::value_origins` (22708) is a third resolver of "origins of a reference-valued expression". `exclusive_reference_origins` is called from path_instantiation (18725), reference_origins (21003), and parameter_relative (via `projected_carrier_reference_origins`).

**Stated limits.** None.

**Fail-open risks.** None seen: all failures return `None`. `declared_origin_root` returns `Some(())` for any Name whose root type resolves (not a risk by itself).

---

## 9. `reference_subjects` (21280-23248), with nested `bindings`, `projections`, `read_backing`, `read_projection`, `results`

**Purpose.** Prove statements about reference bindings before a statement: stable bindings, origins of local/ expression references, whether an assignment rebinds a reference or writes storage, and capture of the descriptor loads that back a read through references.

**Shape (module-level).** `validate_operand_selectors` (22108, `pub(super)`), `owned_borrow_origin` (22120), `carrier_storage_type` (22155), `carrier_storage_declaration` (22171), `unknown_readonly_origin` (22187), `assignment_rebinds_parameter` (22201, `pub fn`), `replaces_binding` (22214), `replaces_reference_binding` (22248), `assignment_stored_value_type` (22282, `pub fn`), `expression_is_fixed_incoming_reference` (22314), `parameter_at_place` (22324), `reference_value_replaces_binding` (22335), `expression_origins_before_statement` (22377), `expression_sources_before_statement` (22396), `coordinate_sources_before_statement` (22435), `coordinate_reads_in_prefix` (22456), `expression_read_paths_before_statement` (22506), `expression_lender_declarations_before_statement` (22542), `local_origin(s)` (22604/22620), `value_origins` (22708), `backing_origin(s)` (22745/22803), `validate_backing_projection` (22854), `captured_value_origins` (22880), `declared_initializer_origins` (22900), `frozen_reference_origin` (22984), `validate_source_projection` (23046), `validate_declaration_source_projection` (23161), `source_root_type` (23172), `validate_referent_projection` (23202), `validate_owned_projection` (23220).
- `bindings` (21299): `calls_expose_bindings` (21309), `are_stable_at_site` (21351).
- `projections` (21414): `validate_selectors` (21420), `validate_owned_source` (21458).
- `read_backing` (21498): `capture` (21502), `from_reads` (21530).
- `read_projection` (21569): `ProjectionUse` (Address | Contents), `capture`, `validate_address`, `capture_projection` (21620), `complete_contents` (21732), `contents_invalidation_paths` (21781), `complete_spans` (21852), `admitted` (21916), `resolved_read_coordinate` (21957).
- `results` (22011): `call_expression` (22019), `call_origins` (22028).

**How it decides.**
- `reference_value_replaces_binding` (22335-22375): false when the target is an atomic operation (22343), false when the target is a non-Shared-reference parameter place (22354-22367); true when the declared reference is `Shared`, or when the value `expression_may_rebind_mutable_alias`. Keyed on `ReferenceAccess::Shared` and the declared type (22369-22374).
- `replaces_binding` (22214-22246): only Assignment; target must be a local or parameter with no segments; else `Some(false)`.
- `are_stable_at_site` (21351-21412): `CallerWriteSite::Statement` and `Transition` -> `None` (21409); exposure through `call_exposes_frozen_binding`/`expression_exposes_frozen_binding` or `expression_reborrows_unresolved_reference_binding` on arguments.
- `read_projection::capture_projection` (21620-21730): a cursor walks the declared type; only Field/Case/FixedIndex segments allowed for contents (21684-21691), Index allowed only for Address use (21680-21683); refusing if a reference in the path is not readable; `cuts` mark descriptor loads; reads must supply loaded slots for each cut (21715).
- `admitted` (21916-21955): refuses lifetime debt, opaque outputs, unresolved supplier coordinates, unreadable slots, slots with no declaration namespace.
- `resolved_read_coordinate` (21957-22009): Exact -> true; coarse requires valid root, builtin coordinates, no recasts/representation, and the declared-type cursor to advance through `advance_fixed`.
- `local_origins` (22620-22706): local must be declared before the statement in admitted execution order, reference-typed, same state; the alias name looked up by string `name == local.name.as_str()` (22657, 22667); exactly one alias entry or divergent list; origins must be Exact, valid projections, and root not a machine nor a reference-typed local declared before the statement.
- `value_origins` (22708-22743): for call expressions returns `declared_initializer_origins`; otherwise caller-isolated initializer origins, each Exact and `validate_source_projection`.
- `backing_origins` (22803): origins may be coarse; static allowed; result homes need no recasts/representation (22818-22823).
- `declared_initializer_origins` (22900-22982): peels formation/borrow; call path via `results::call_origins` (requires reference return type, `call_result_origins` with `include_shared=true`, only `Reference` leaves with empty segments/suffix and Exact precision, 22084-22101); non-call: `declared_origin_root`, `validate_selectors`, requires a reference-typed or explicit/implicit borrow, owned-source check else `frozen_reference_origin`.
- `unknown_readonly_origin` (22187-22199): for a reference type that `!type_may_carry_write`, returns an origin with the NAME as path, `CollectionCoarse`, `FrameSourcePlace::default()`.
- `validate_source_projection` (23046-23159): recast chains need same root, matching presentation identity, ExtentView type checks, formation interpretation among RepresentationView/Access/ResidentAccess with matching view type, access-view plans exactly one by `data_symbol == view.symbol`.

**Cases by shape.** `validate_selectors` (21420-21455): Borrow, Member (self fields must be exact self fields; other members' symbol must equal `effective_member_symbol`), SavedSubject, Name; `_ => return None`. `validate_owned_source` (21458-21496): machine-attached field must come from a single, non-generic attached data definition and be non-reference. `validate_owned_projection` (23220-23247): only Field/Case/FixedIndex. `contents_invalidation_paths` (21781): for a coarse source with spans, lifetime access, output identity, non-builtin coordinates or no valid root and not static -> `None` (21807-21815).

**Same job elsewhere.** `source_root_type` duplicate (see section 7). `value_origins`/`backing_origins`/`declared_initializer_origins` (22708/22803/22900) vs `reference_origins::exclusive_reference_origins` (20817) and `parameter_relative_origins` (18160): three origin resolvers. `reference_value_replaces_binding` (22335) vs `local_aliases::expression_may_rebind_mutable_alias` (17945) are related halves.

**Unused / one-caller (grep).** `reference_subjects::local_origin` (22604): grep `local_origin(` shows one call at 14848. `backing_origin` (22745): one call at 6867. `captured_value_origins` (22880): one call at 7175 (grep in this range pattern of fn names). `unknown_readonly_origin`: callers at 25631 and 27296 (read). `assignment_rebinds_parameter`, `assignment_stored_value_type`: `pub fn`, not grepped (open).

**Stated limits.** Doc comments at 22106-22107: "This supplies no backing or access permission." and 22116-22119: "Reference-rooted paths still require chronological origin evidence; selectors, representation and loan admission are not inferred from a write footprint." Refusal strings throughout `expression_lender_declarations_before_statement` (22549-22600).

**Fail-open risks.**
- `unknown_readonly_origin` (22187): a read-only reference type that cannot carry writes gets a coarse default-source origin and is accepted as a known supplier in `pending_result_reference_sources` (25629-25633, only when `include_shared`) and in `state_write_walk` (27294-27305). Wrong program: a shared reference whose type is judged unable to carry writes but whose actual backing matters for loan tracking would be accepted with a name-only origin. Status: traced to the two callers; the type guard `!type_may_carry_write` is the only protection (stated by name only, no comment).
- `reference_value_replaces_binding` 22354-22367 returns false (no replacement) for a parameter place of non-shared type: an assignment to an exclusive-reference parameter is treated as writing through, not rebinding. Status: open.
- `are_stable_at_site` Statement/Transition `None` -> callers `.is_some()` = false = unstable: fail-closed.

---

## 10. `result_origins` (23249-26104), with nested `declared_lifetimes`, `input_moves`, `input_sources`, `primitive_results`, `selected_sources`, `state_equations`

**Purpose.** Compute, for a call, which caller places the returned references (and enum cases, and moved aggregates) may point to, by solving per-state equations over the callee's states and instantiating them at the call site.

**Shape.**
- `declared_lifetimes::call_result` (23283): for calls to signatures (requirement/dynamic) with lifetime-dependency return leaves, builds `SlotSource::from_place` dependencies from `ViewReturnSource::Fields`.
- `input_moves::validate_frozen_inputs` (23387), `instantiate_moves` (23440), `local_suffix` (23537).
- `input_sources::instantiate_source` (23572), `application_source_reaches_leaf` (23744), `compose_source` (23780).
- `primitive_results::result_origins` (23810: tries `installed_context_result_origins` 23977, `activation_borrow_result_origins` 23931, `access_view_result_origins` 23862), `extent_view_types` (23833).
- `selected_sources`: `ResultSelection` (24064), `retain_reachable_sources` (24135), `ResultReachability` (24193), `ResultQuery` (24198), `reachable_value_parameters` (24206), `_body` (24259), `contributed_parameters` (24369), `contributed_named_target` (24442), `parameters_reached_through` (24474).
- `state_equations`: `Edge` (24518), `Equation` (24525), `solve` (24531), `propagate` (24684), `same_relation` (24721), `append_new` (24751), `instantiate` (24781), `returned_parameters` (24811), `finite_substitutions` (24853).
- Module level: `call_result_origins` (25063), `call_body_result_origins` (25134), `state_result_origins` (25217), `state_equation` (25288), `unchanged_static_arguments` (25502), `merge_result_arm` (25531), `pending_result_reference_sources` (25601), `reference_result_relation` (25640), `merge_relative_origins` (25701), `canonical_result_reference_origins` (25783), `merge_named_edge` (25836), `instantiate_edge_origins` (25895), `instantiate_result_origins` (25954).

**Inputs/outputs.** Reads callee states, statements, transitions, parameter types, stored carrier records and aliases from `CollectedStatementPrefix`. Outputs `AggregateOrigins` (references/cases/moves). Mutates `FrameInference` (active states, local scope).

**How it decides.**
- `primitive_results` (23810-24046) are special cases keyed on declarations: `AccessExecutionDeclaration::authenticate` with `operation == View`; `exact_toolchain_activation_storage_contract(..).operation == BorrowBytes`; `static_interrupt_context(..)` returning a context whose `entry == state.symbol`. Each requires exactly one frontier leaf (or exactly one state and one parameter, 23949-23957), matching return type, and fixed access kinds (view result must be a Reference with the storage shell's access, 23894; installed context must be `Shared`, 24010). These build hard-wired origins, e.g. path string `"installed interrupt context"` (24034) and `"{param name}"` (23909, 23968).
- `call_result_origins` (25063-25131): callee is a state with a known machine -> if primitive results exist use that state, else `checked_body_alternatives`; union over alternatives. No machine -> `declared_lifetimes::call_result`.
- `call_body_result_origins` refuses when: state is currently active (recursion, 25167), body not `CheckedBody`/present with no primitive result (25168-25170), receiver presence mismatches attached data (25171-25172), argument count mismatch, result type mismatch (25174).
- `state_result_origins` (25217-25285): primitive first; refuse if active; run `state_equations::solve` with values cleared; then `retain_reachable_sources`, then specialize sources whose root is an entry parameter, found by binary search over `(arena_index, generation)` of parameter symbols (25260-25279).
- `state_equation` (25288-25500): refuses when frozen inputs are exposed, when any statement has an exclusive borrow whose target is not a declared origin root (25307-25323), or a reference source with a place path whose root is not declared (25344-25350). Result expressions: a `completing_call_transfer` into a CheckedBody machine with unchanged static arguments becomes an `Edge` (25395-25423); all other results go through `merge_result_arm`. Only `Ordinary` transitions with `Value`/`Named` targets are processed (25454-25486).
- `unchanged_static_arguments` (25502-25528): all of the callee's type parameters must be in scope in the source machine and passed through unchanged (by symbol/type-reference identity).
- `retain_reachable_sources` (24135-24190): drops reference leaves whose root is a parameter not reachable under known boolean arguments; skipped if reachability is unknown (`let Some(..) else return`, 24159) or if dropping would remove all indirect leaves (24178-24188). Fixed-point loop re-queries until nothing changes (24143-24158).
- `state_equations::solve` (24531-24682): BFS over states reachable by edges, `finite_substitutions` (cycle check via `Substitutions::is_finite`), then two `propagate` passes (moves first, then everything) each a worklist over edges.
- `instantiate_result_origins` (25954-26103): for each leaf: lifetime dependencies instantiated source by source; static/output/result-home origins kept (rebased to the call site expression, 26033-26044); coarse origins with no valid root kept when `include_shared` (26047-26056); else the origin root must be a parameter of the callee (`?`).

**Cases by shape.**
- `input_sources::instantiate_source` (23572-23741): three branches: (a) `include_shared` and reference-typed parameter, (b) exclusive-reference parameter, (c) by-value parameter holding references; (c) requires `relative.source.root == parameter.symbol` (23668-23671).
- `contributed_parameters` (24379-24439): Borrow, Name (parameter if stable; or captured local), Call without receiver; `_ => None`.
- `merge_named_edge` (25836): only callees `CheckedBody`, body present, no type parameters, type-matching return (25855-25863) for terminal-call targets.
- `reference_result_relation` (25640-25698): all locals before the boundary become aliases with invalid parameter symbol; the result relation is exactly one `Reference(access)` leaf per origin with empty segments.
- Quote 24192 `#[derive(Default)] ResultReachability` etc. no comments. No "unsupported" comments.

**Same job elsewhere.**
- `ReferenceLeaf` derives `PartialEq, Eq` (18825) but field-by-field equality is hand-written in `state_equations::same_relation` (24731-24739) and `append_new` (24753-24760), `merge_relative_origins` (25768-25775); whereas `call_result_origins` (25115) and `reference_leaves`/`partition_sources` use `contains` (==). `AggregateMove` hand-compare at 24670-24676, 24770-24774, 25719-25723 vs `contains` at 25125 (read). So one equality, four spellings.
- `merge_result_arm` / `instantiate_edge_origins` build nearly the same `SymbolicArguments` closure pair (25571-25583 and 25925-25937).
- `state_equations::solve` is a worklist fixed-point; `stored_calls::normal_exits::solve_state_effects` (28040) is another fixed-point over the same kind of state graph with its own substitutions/edges/inventory (read both).
- `returned_parameters` + `finite_substitutions` (24811-25060) vs `stored_calls::normal_exits::retain_demanded_substitutions` (28284-28323): both build `cycle_substitutions::Substitutions` demands and ask `is_finite()`; plus `storage_origin_substitutions_are_finite` (15237).

**Unused / one-caller.** `extent_view_types` (re-exported at 24049) is also referenced via `ReferenceViewFormation::extent_view_types` at 19845 and in `validate_source_projection` 23085. `state_result_origins`: `pub(crate)`, callers not counted (open).

**Stated limits.** None quoted. Behavior limits: recursion (`active_states`) yields `None`; non-CheckedBody callees yield `None`.

**Fail-open risks.**
- `retain_reachable_sources` returns early (keeps everything) when `reachable` is `None` (24159) or when filtering would remove all indirect leaves (24186-24188). Keeping extra sources only widens the result: fail-closed direction. Status: traced (consumer is origin set).
- `merge_relative_origins` 25728-25731: `Captured` origin -> `return None` (refuse).
- `canonical_result_reference_origins` 25819-25821: a divergent alias with no candidates returns `vec![None]` (an unknown origin, not an accepted one).
- `instantiate_result_origins` 26047-26056: with `include_shared`, an origin with an invalid root and coarse precision is passed through unchanged. Wrong program: a coarse unknown-root origin passes through the call boundary without binding to an argument. Status: open.
- `primitive_results::activation_borrow_result_origins` (23931-23975): gives a hard-wired `Mutable` origin on the single parameter for any machine whose toolchain activation contract is `BorrowBytes`. Status: stated by name of contract only.
- None else seen.

---

## 11. `state_paths` (26105-26189)

**Purpose.** String-level visibility and normalization of state-relative written paths.

**Shape.** `push_visible_frame_path` (26114), `expression_forwards_exact_symbol` (26128), `relative_state_path_is_visible` (26142), `normalize_state_relative_path` (26161).

**How it decides (string/name keyed).**
- `relative_state_path_is_visible`: root string is `self` -> visible; equals any parameter NAME -> visible; equals a local NAME -> not visible (`Some(false)`); else `None` (unknown root).
- `normalize_state_relative_path`: `self` kept; a parameter NAME becomes `$P{index}{suffix}` (26175-26178), index = position in `state_parameters`; a local NAME -> `Some(None)` (dropped); else `None`. Local detection scans `StatementNode::LocalData` names (26180-26186).

**Same job elsewhere.** `instantiate_written_path_for_origins` (19435-19486) is the inverse substitution, using parameter name compare too. `stable_alias_place_origins` (17699-17703) compares roots against parameter names again.

**Unused / one-caller.** `expression_forwards_exact_symbol`: not grepped. Combined grep `fn state_paths|relative_state_path_is_visible|expression_forwards_exact_symbol` = 12 occurrences in calls.rs.

**Stated limits.** None.

**Fail-open risks.** `normalize_state_relative_path` returns `None` -> `infer_state_write_frame` makes the frame opaque (16019). Fail-closed. Name shadowing: a local and a parameter with the same name would be classified by the parameter branch first (`parameter_index` found first); a local that shadows... Status: open (shadowing rules not traced).

---

## 12. `state_write_walk` (26190-27399)

**Purpose.** Walk a state's statements in admitted execution order, tracking aliases, divergent aliases and stored-origin carriers, and produce either the state's written paths, or per-statement `CollectedStatementPrefix` snapshots, or per-assignment write targets.

**Shape.**
- Queries: `StateWriteQuery` (26376: Complete | OriginTransfer{include_shared} | StatementPrefixes{include_shared}).
- Types: `StateWritePrefix` (26308: private `written`, `stored`), `AssignmentPrefix` (26314), `CollectedPrefixes`, `CollectedPrefixMap = symbols::SymbolKeyMap<(machine, state, include_shared) -> ..>` (26382-26383), `CollectedStatementPrefix` (26386: isolated_locals, aliases, divergent, stored, template_dependent, incomplete_calls; `is_complete()` 26396), `StatementPrefixCollection` (26401).
- Entry points: `summarize_state_written_paths` (26244), `summarize_complete_state_written_paths` (26261), `walk_state_assignments` (26351), `walk_state_write_prefix_collected` (26406), `stored_reference_transfer` (26434), `collect_state_write_prefixes` (26452), `collected_prefix_at` (26486, requires `is_complete`), `collected_prefix_with_readiness_at` (26507, lazily builds and caches), `walk_state_write_prefix` (26542), `walk_state_write_prefix_inner` (26557), `walk_state_write_prefix_body` (26581-27361), `install_statement_call_effects` (27363), `push_queried_assignment_and_visible_paths` (27379).

**Inputs/outputs.** Reads the state's statement table, parameters, `FrameInference`; uses `stored_origins`, `stored_calls::statement_effects`, `alias_origins::*`, `caller_aliases`. Mutates the `Mutex<CollectedPrefixMap>` (cache) and the inference.

**How it decides.**
- `walk_state_write_prefix_body` (26581): returns `None` for core vector surface states (26591-26595). For each statement: snapshot prefix if requested (26660-26670); computes `declared_local_alias_origin` for locals that may carry writes and aren't isolated (or are references) (26709-26731); `replaces_local_binding` via `reference_subjects::replaces_binding` (26732, `?` refuses unknown); `stored_call_transfer` via `statement_effects(...)`; `origin_transfer_only` = relation complete or pending prefix AND query is OriginTransfer/StatementPrefixes (26802-26809); refuses (`return None`) if the statement exposes a frozen binding unless `pending_prefix` (26827-26841).
- Divergent alias branch (26842-26926): special-cases assignment through a divergent alias root; refuses when the value mentions the divergent roots (26860-26866) or when a non-bare target with non-rebinding value is reference-typed (26872-26890).
- Per statement kind (26999-27338): Assembly/AssemblyFact nothing (27000); Assignment via `assignment_origins`; Call (nested ladder 27068-27200); Transition (OriginTransfer/StatementPrefixes queries skip writes and only install call effects, 27203-27219; others require acyclic named subgraph when aliases/stored exist, 27221-27232); Expression nothing; LocalData installs aliases/stored.
- Final gate (27352-27360): result present only if origins complete (for OriginTransfer) and query is None/Complete/OriginTransfer; `StatementPrefixes` returns `None` by design at the last statement (26671-26675).
- `summarize_state_written_paths` (26244): complete summary, and if it fails and the transition subgraph is NOT acyclic, falls back to the prefix walk with no query (26253-26258).
- `summarize_complete_state_written_paths`: if the inference selects the state (invocation values) and the subgraph is cyclic, retries without values (26269-26274); consults `caller_sites.completed_state_writes` cache (26286) and records completed writes (26302); cyclic -> `summarize_state_written_paths_for_cycles` (26289).

**Cases by shape.** Cache key `(machine, state, include_shared)` (26383, 26516). In the assignment-roster fast path (26678-26708): only when `local_alias_origins`, `stored`, `divergent` are all empty and the target has a `coarse_place_path` does it record `Storage{paths: vec![path]}` directly without alias expansion (26682-26707).

**Same job elsewhere.** The nested-call ladder (27126-27187) copies `may_write_frame`'s ladder (15401-15478) and `collect_operation_written_paths` (16275-16313): known / boundary / requirement / syntactic (three copies, read). `walk_state_assignments` roster vs `assignment_write_target` memo at 15112 (consumer).

**Unused / one-caller.** `collect_state_write_prefixes` is called from `collected_prefix_with_readiness_at` (26521); `walk_state_write_prefix_collected` from 25357 and `stored_reference_transfer` (26441). Not grepped outside file.

**Stated limits.** None quoted.

**Fail-open risks.**
- 26827-26841: when `pending_prefix` the frozen-binding exposure check is skipped. `pending_prefix` is true only for `StatementPrefixes` queries with template-dependent but otherwise complete transfers (26793-26797). Status: traced: that query returns `None` at the last statement (26671) and `collected_prefix_at` demands `is_complete()` (26504), so the pending prefix is not usable as complete. `collected_prefix_with_readiness_at` (no completeness filter) is used by `statement_reference_transfer` (14763) and `expression_sources_before_statement` (22409, which then filters on `incomplete_calls.is_empty()`, 22418, but NOT on `template_dependent`). Wrong program: a template-dependent prefix could reach `expression_sources_before_statement` consumers. Status: open for the template_dependent case (not traced).
- 26682-26707 roster fast path: when `exposes_frozen_binding` is true, it does not record the assignment but sets `queried_assignment = false`; the later general path then does not push it either: the assignment is absent from the roster (not recorded as a storage write). Consumers `assignment_write_target` then return `None` (unknown) for that statement presumably: fail-closed. Status: inferred.

---

## 13. `stored_calls` (27400-29478 partial; module continues past the range)

Nested: `slot_relations` (27401), `slot_inventory` (27604), `normal_exits` (27997), `instantiation` (28514), `transfers` (28903), `captured_inputs` (29257, entered at `destination_addresses` 29258; ends at 29478 mid-function).

**Purpose.** Model how a callee's stored-reference writes (writing a reference into a slot reachable through an exclusive parameter) change the caller's carrier records: compute the callee's slot effects at normal exits by a fixed-point over states, instantiate them at a call site using captured argument sources, and apply them to the caller's `stored`/`aliases`/`divergent`.

**Shape.**
- Types (27401-27597): `StoredReferenceDestinationKind` (pub), `StoredReferenceEffect` (pub: suppliers_complete, has_static_values, destination: LocalWriteOrigin, sources, retains_incoming, incoming_identity, may_select, destination_kind), `StoredReferenceCallContext` (pub: Entry | StatementCompletion | Invocation | Join), `StoredReferenceSource`, `SlotEffect` (root, segments, `sources: Option<Vec<SlotSource>>` where `None` = unknown, incoming, may_select, kind), `SlotSource` (origin, carrier, carrier_backing, lifetime_access), `SlotInventory`, `StoredReferenceRelationRefusal` (`Unavailable(&'static str)` | `TemplateDependent`), `StoredReferenceCallFailure`, `SlotRelationTransfer` (effects, case_effects, template_dependent, incomplete_calls; `complete`, `is_complete`, `into_complete_effects`, `unknown`), `DestinationInventory`.
- `slot_inventory`: `destination_inventory` (27608), `provider_output_relation_required` (27617), `parameter_destination_inventory` (27707), `provider_completion_covers_writable_slots` (27867), `seed_incoming_slots` (27894), `mentioned_type_parameters` (27950).
- `normal_exits`: `state_effects` (28001), `SlotExit` (28024), `SlotEquation` (28033), `solve_state_effects` (28040-28282), `retain_demanded_substitutions` (28284), `reference_frontier` (28325), `collect_exit` (28356).
- `instantiation`: `InstantiatedSlots` (28517), `instantiate` (28522), `trace_instantiation_refusal` (28639), `destination_source(s)` (28687/28704), `aggregate_argument_sources` (28888).
- `transfers`: `public_effects` (28906), `apply_transfer` (28968), `binding_preserved_after_transfer` (28984), `materialize_changed_aliases` (29014), `apply` (29115).
- `captured_inputs::destination_addresses`: `CaptureKey = (u32,u32)`, `assignment_addresses` (29267, truncated at 29478).

**Inputs/outputs.** Reads state parameters/types, the stored-origin records, statement prefixes, call captures (`SlotArgumentCapture`, defined outside my range). Output: `Vec<SlotEffect>` per state; `InstantiatedSlots`; mutates caller `stored`, `aliases`, `divergent` via `apply_transfer`.

**How it decides.**
- Inventory (`parameter_destination_inventory` 27707-27865): skip `Erased` parameters; skip non-exclusive reference shells; walk the declared type with a work-list and `visited` (a revisit returns `None`, i.e. recursion unsupported, 27744-27746); skip proof-value-isolated types (27739), skip fully borrow-free types whose retained lifetime owner paths are empty (27748-27759); a leaf of non-reference storage with a type parameter marks `template_dependent`; a boundary-opaque data definition is skipped, any other non-reference leaf with lifetime dependencies -> `None`.
- `provider_output_relation_required` (27617-27705): looks at dynamic slots, selected requirement signatures, or boundary/admission/external-realization machine states; for requirement signatures matches by target symbol and call target; returns whether the inventory is non-empty (`is_none_or(|inv| !inv.slots.is_empty())`, 27704, meaning unknown inventory -> relation required).
- `solve_state_effects` (28040-28282): per-state equations from `destination_inventory`, `stored_reference_transfer` and `expression_effects::prepare_normal_exits`; fixed-point loop (28130-28280) merging candidate effects per state; refuses if effect kinds differ for the same slot (28257-28259, 28500-28502).
- `collect_exit` (28356-28511): refuses dynamic indexes with valid expressions (28365-28371), requires each ancestor to have exactly one matching descriptor origin with `builtin_coordinates` (28375-28385), selects sources, refuses unknown contents unless the slot is `may_select` with no matching leaf (28393-28399); sources coarsened by `summary_windows::coarsen`; refuses non-builtin coordinates, local (non-parameter) sources, and sources inside a by-value parameter that does not reach a reference leaf (28448-28467); an "incoming" effect is a source equal to the slot itself.
- `instantiate` (28522-28637): parameters/args length must match; each effect's destinations come from `destination_sources` (requires exclusive reference shells or by-value aggregates; refuses shared; refuses coarse alternatives when more than one candidate, 28746-28748, 28872-28874); output effects marked `incoming` if conditional (more than one destination, 28621-28627).
- `apply` (29115-29251): `sources == None` (unknown) or may_select invalidates covered descendants by adding origin-less copies; removes overwritten leaves unless the effect is incoming; pushes new `StoredWriteOrigin`s; OpaqueRegion effects also clear `cases` and `moves` under that path (29242-29249).
- `public_effects` (28906-28966): `OpaqueRegion` kind -> `None`; lifetime-destination effects with any non-lifetime source -> `None`; static sources counted in `has_static_values` and then filtered out of `sources` (28948).
- `materialize_changed_aliases` (29014-29112): for effects on whole-root reference bindings that have an alias, converts the alias into a stored record (typed via `declared_origins_for_query`, or an empty record) and drops the alias.
- `assignment_addresses` (29267-): needs unique captures per expression key, `value_has_builtin_coordinates`, canonical place, optional fixed `array_window_shape`, spine walk where each projection receiver is captured, field case selectors must match `payload_variant_for_field`; unsupported segment -> `Err("destination has an unsupported canonical projection")` (29365).

**Cases by shape.** `destination_sources` segment filter (28714-28721): Field, Case, FixedIndex, or Index with an invalid (empty) expression; anything else -> `None`. Boundaries: "boundaries.next().is_some() -> return None" i.e. at most one reference boundary per path step (28813-28815). `reference_frontier` (28325): only Field/Case/FixedIndex segments in the leaf path. `StoredReferenceRelationRefusal::Unavailable(...)` strings at 27538-27546.

**Same job elsewhere.**
- `reference_frontier` (28325-28354) duplicates the "min-length boundary leaf of a declared origin set whose path is a prefix of the source segments" computation done inline in `state_equations::finite_substitutions` (24980-25001).
- `normal_exits::solve_state_effects` vs `result_origins::state_equations::solve` (fixed-point, see section 10).
- `slot_inventory::seed_incoming_slots` (27894) vs `statement_effects`.
- `SlotSource::from_place` (27475) constructs `FramePlaceOrigin { path: path(program, &place), .. }` with a `path` helper defined elsewhere.

**Unused / one-caller.** Not grepped individually (open). `SlotRelationTransfer::into_complete_effects` is a public conversion; callers outside range not counted.

**Stated limits.** `StoredReferenceRelationRefusal::TemplateDependent` and refusal strings. 27538: "stored call has no complete statement-successor effects: an invocation relation is unavailable".

**Fail-open risks.**
- `SlotRelationTransfer::unknown` (27550-27590) builds effects with `sources: None` for every known leaf plus an incomplete-call record unless `template_dependent`; `is_complete()` returns false in both cases. Fail-closed: stays "unknown".
- `apply` with `sources.is_none()` marks leaves unresolved (origin None) rather than preserving. Fail-closed.
- `collect_exit` 28393-28399: `None` sources with `slot.may_select` and no matching may_select leaf in the record -> `continue` (skip the slot). Wrong program: a may-select slot the callee never wrote at exit is treated as unchanged. Status: traced (a may-select slot absent from the record means no write recorded for that selector; plausible but no comment). Open on whether the record could be missing the leaf due to an unrelated refusal.
- `instantiate` 28566: effect with `incoming` and empty sources is skipped (identity effect). Stated by the shape (`incoming_identity`).
- None else seen.

---

## Cross-cutting findings for this unit

**Key and identity styles used (read).**
- Statement identity by address: `std::ptr::from_ref(statement).addr()` in memo keys (14797, 14813, 14846, 14868 area, 15119, 15285, 15324, 15890; `may_write_frame` 15385).
- Symbol identity by `SymbolHandle` and `(arena_index, generation)` pairs (15383-15384, 25260-25279).
- Names: alias, parameter and local lookup by string compare (17677, 17700, 17820, 19438, 19484, 22657, 22890, 26142-26186); `definitions_for_name` by type-name string (17292, 17540, 17446 fallback); `receiver_referee_name` returns a type name string and compares across Match arms (20789); `format!("invocation result @{}:{}{suffix}")` as a path spelling for call-result homes (20477) and path `"installed interrupt context"` (24034).
- Containers: `symbols::SymbolKeyMap` (26238, 26383), `BuildSymbolHasher` HashMap (17123), plain `std::collections::HashMap` for `CompletedSlotEffects` (16421-16422) keyed by `(SymbolHandle, Option<ParameterValues>)` (a struct of vectors as a hash key, 16676 derives `Hash`). Caches are `Mutex<HashMap<..>>`.

**Greps run (Grep tool, count mode unless noted).** `frame_paths_overlap` = 28 in 12 files; `expression_borrows_fixed_parameter_address` = 2 (calls.rs 1, flow.rs 1); `expression_is_fixed_incoming_reference` = 6; `same_place_origin|single_parameter_relative_origin|single_place_origin` = 15 (calls.rs); `with_dynamic_selectors|known_dynamic_implementation` = 15 (calls.rs); `syntactic_call_written_paths|conservative_call_written_paths` = 9 (calls.rs); `fn state_paths|relative_state_path_is_visible|expression_forwards_exact_symbol` = 12 (calls.rs); `fn type_identities_match|fn aggregate_storage_types_match|aggregate_storage_types_match\(` = 14 (calls.rs 13, typed-trees type_identity.rs 1); `ReferenceSource::Captured\(` = 16 (calls.rs); `impl.*ArgumentOrigins for` = 2 sites (18911, 29871). Content grep for `with_dynamic_selectors|local_origin(|captured_value_origins(|backing_origin(`: callers at 14393, 14848, 6867, 7175.

**"Single/shared/canonical" comments.** None in this range claims a single shared mechanism in those words. (No finding.)

## Coverage

- Read in full: `omega-rust/psi/compilation/validation/src/calls.rs` 14740-29478 = 14,739 lines (read in 10 contiguous Read calls of up to 1,500 lines each; the final read returned up to line 29479, one line past the unit).
- Skimmed: 0 lines.
- Not read: 0 lines.

## Five things a maintainer most needs to know about this unit

1. This is one slice of a single 39k+ line file of nested modules that all answer "what can this call or statement write in the caller's frame?", answered as complete string-path lists or "opaque". Failure is expressed as `None`/opaque nearly everywhere, so most refusals widen to opaque (calls.rs 15481-15483, 16019, 26591-26595).
2. Place identity exists in two parallel forms: a string path (`"self.a[*]"`, `"$P0.f"`) and a structured `FrameSourcePlace` (root symbol, segments, recasts, representation). Overlap tests (`frame_paths_overlap`, 16026) and alias/parameter lookups (17677, 17700, 19438, 26142) work on string spelling and names; origin validation (`validate_source_projection` 23046, `storage_coordinate` 19645) works on the structured form. Coarse paths lose case and exact index information (20613-20630).
3. The "known / boundary / requirement / syntactic-conservative" call write ladder exists three times (15459-15478, 16275-16313, 27161-27187), and the conservative fallback `syntactic_call_written_paths` (16341) skips Cast/Unary/Binary/Match arguments and shared borrows (16366, 16373).
4. Origin resolution of reference expressions is done by at least four resolvers with different alias models: `parameter_relative_origins` (18172), `reference_origins::exclusive_reference_origins` (20817), `reference_subjects::value_origins/backing_origins` (22708/22803), and `result_origins` equations (25217); `ReferenceLeaf`/`AggregateMove` derive `PartialEq` yet are compared field-by-field in four places (24731, 24753, 25768, 24670).
5. Hard-wired special cases keyed on declarations sit inside the general solver: `primitive_results` (23810-24046) for access-view, activation-borrow and installed-interrupt-context results, with fixed access kinds and literal path strings; `type_is_caller_isolated` treats `DynamicTrait` as isolated (17528) and `unknown_readonly_origin` accepts name-only coarse origins for read-only references (22187).

## What you are unsure about

- The type name and fields of the facade impl (`CallFrames`-like) starting before my range; I only saw its method tail, so cache field types are from the earlier grep output (14196-14227), not from reading.
- Whether `expression_has_calls` (used at 15809 and 15923) counts atomic-once operations; this decides whether the statement-root frame at 15923 can miss them.
- Whether `DynamicTrait => true` in the isolation verdict (17528) can hide writes through dyn values; I did not trace dyn representation.
- Whether `collected_prefix_with_readiness_at` callers (14763 in this range, 22409) correctly handle `template_dependent` prefixes.
- Call-site counts for `pub fn` items I did not grep (`assignment_rebinds_parameter`, `assignment_stored_value_type`, `frame_coordinate_path`, `frame_length_coordinate_path`, `instantiate_written_path`, `state_result_origins`, `struct_literal_field_type`): reported as open, not "unused".
- The tail of `assignment_addresses` (29480+) and the rest of `stored_calls::captured_inputs` (including `Inputs`, `SlotArgumentCapture`) are outside my range and not described beyond 29478.
