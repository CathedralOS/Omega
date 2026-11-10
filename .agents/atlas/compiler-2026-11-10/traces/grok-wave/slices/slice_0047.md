slice_0047

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0357 | calls:8844-8846 | UNDECIDED | Shared-ref parameters are skipped in `exclusive_boundary_result_origins`; no traced check that an exclusive result cannot alias shared-arg storage. |
| A0358 | calls:12128-12129 | CLOSED | `continue` applies only when the path root is a state-local name already listed in `equation.locals`; cross-state wiring still requires parameter/receiver roots. |
| A0359 | calls:11870 | REACHABLE | `Value` transition targets return an empty write list while `state_write_walk` does not also walk the value expression for calls. |
| A0360 | calls:16366,16373 | UNDECIDED | Syntactic fallback skips non-place arguments; exclusive borrow under Cast/Match at that site was not traced in borrow validation. |
| A0361 | calls:16158-16193 | CLOSED | Early write-free return is gated on `byte_predicate_call` or `has_empty_write_frame` plus absence of boundary/machine/requirement paths. |
| A0362 | calls:15923-15928 | REACHABLE | Statement write-frame path omits the `atomic_once_carrier` gate that the expression write-frame path uses. |
| A0363 | calls:17528 | UNDECIDED | `DynamicTrait` is classified caller-isolated; whether ordinary `dyn` values can carry interior references was not traced. |
| A0364 | calls:17669-17681 | CLOSED | Unmatched alias leaves the path unchanged; `substitutions_are_finite` still fails closed on unknown roots (calls:12130-12131). |

### A0357

1. Read `exclusive_boundary_result_origins` in full (calls:8816-8888, read).
2. Search `rg '\bexclusive_boundary_result_origins\b' /agent/repos/omega/omega-rust` → 4 hits (definition plus `boundary_result_origins` calls:8701, `bodyless_callee_result_origins` calls:8946, `requirement_result_origins` calls:8998; read call sites, not every transitive consumer of those three).
3. Read callers `boundary_result_origins` (calls:8654-8721), `bodyless_callee_result_origins` (calls:8910-8960), `requirement_result_origins` (calls:8963-9012); each feeds `canonical_result_origins` (calls:9019-9041) into write-frame / result-origin machinery (inferred).
4. At calls:8835-8846 (read), for each parameter/argument pair, non-`Reference` parameters may return `Opaque` if `type_may_carry_write_in` (calls:8839-8840); reference parameters with `!access.is_exclusive()` hit `continue` and are not checked with `owned_storage_may_hold` (calls:8847+).
5. **Input on the branch:** typed boundary/requirement/bodyless call whose result referent is already exclusive (`exclusive_result_referent` calls:8648-8649, read; bodyless calls:8935-8936, read), with a shared-reference parameter that could overlap the exclusive result storage.
6. **Verdict: UNDECIDED** — Entry matches code at calls:8844-8846 (read). `exclusive_result_referent` guards the result type, not overlap with shared arguments. I did not trace validation that forbids exclusive results aliasing shared-borrow referents at the same site.
7. **Unsure:** Whether borrow/reference custody rejects overlapping exclusive result and shared argument before write frames consume these origins.

### A0358

1. Read `substitutions_are_finite` in full (calls:12051-12147, read).
2. Search `rg '\bsubstitutions_are_finite\b' /agent/repos/omega/omega-rust` → 3 hits (definition, caller at calls:11439, `flow.rs` uses `storage_origin_substitutions_are_finite` at a different helper; read calls:11439 and calls:15237-15256, not full `flow.rs` consumers).
3. Read caller `summarize_state_written_paths_for_cycles` (calls:11396-11447, read): when `substitutions_are_finite` is false, returns `None` (calls:11438-11446); when true, continues cycle write propagation (calls:11449+, partial read).
4. At calls:12118-12129 (read), for each write source spelling, roots matching `equation.locals` skip `substitutions.insert`.
5. **Input:** `equation.locals` names from `LocalData` (calls:11826, read) appear as the first path segment of a transparent argument write origin in a named-transition edge (calls:12081-12094, read).
6. **Verdict: CLOSED** — Skipping edges for local roots limits the substitution graph to cross-state parameter/receiver linkage; local names are not caller-visible parameter slots in `CycleFrameEquation` (calls:37661-37667, read). Non-local unknown roots still hit `return false` (calls:12130-12131).
7. **Unsure:** Whether `canonical_write_origins` can emit only local-rooted paths for arguments that should induce cross-state growth; that path was not read.

### A0359

1. Read `summarize_transition_target_written_paths` in full (calls:11842-11895+, read through `Value` arm).
2. Search `rg '\bsummarize_transition_target_written_paths\b' /agent/repos/omega/omega-rust` → 3 hits (definition, import at calls:26205, call at calls:27234; read call site).
3. Read caller `walk_state_write_prefix_body` transition arm (calls:27202-27258, read): merges `transition_writes` only from this helper.
4. At calls:11868-11870 (read), `TransitionTargetNode::Value(_)` returns `Some(Vec::new())` without traversing the value expression.
5. **Input:** transition target handle classified as `Value` with an expression subtree that performs caller writes via nested calls.
6. **Verdict: REACHABLE** — Smallest harmful shape (plain words): a machine state ends with a transition to a value expression that is only a call mutating caller-visible storage, while state write-prefix collection uses calls:27234; the transition arm records no writes from that value.
7. **Unsure:** Whether all validation paths that matter use `build_cycle_frame_equation` (which walks `statement_value_expression_roots` at calls:11582, read) instead of this walk for the same state.

### A0360

1. Read `syntactic_call_written_paths` in full (calls:16341-16380, read) and caller ladder in `collect_operation_written_paths` (calls:16275-16313, read).
2. Search `rg '\bsyntactic_call_written_paths\b' /agent/repos/omega/omega-rust` → 9 hits, all in `calls.rs` (read sites at calls:11727, 16305, 16394, 27179; not every intermediate wrapper).
3. Read upward through `collect_operation_written_paths` → `collect_expression_call_written_paths` (calls:16053+, partial read) into write-frame normalization (inferred).
4. At calls:16364-16373 (read), shared `Borrow` continues; `Name`/`Member`/`Indexed` recorded; `Cast`/`Unary`/`Binary`/`Match`/`Call`/`StructLiteral`/`ArrayLiteral` hit `continue` or `return None`; entry’s Cast/Unary/Binary/Match case is the `_ => continue` arm (read).
5. **Input:** call reaching only the syntactic rung (`exact_receiver && !has_divergent_actual`, calls:16302-16304, read) with an exclusive-borrow actual syntactically wrapped in Cast/Unary/Binary/Match.
6. **Verdict: UNDECIDED** — Syntactic under-recording is real at calls:16373 (read). I did not trace whether typed borrow rules prevent exclusive places from appearing only under those nodes at this rung.
7. **Unsure:** Prior ladder rungs (`known_call_written_paths_for_parts`, boundary, requirement) for those argument shapes.

### A0361

1. Read `collect_operation_written_paths` `Call` arm in full (calls:16144-16319, read) and `value_builtin_has_empty_write_frame` (calls:16325-16338, read).
2. Search `rg '\bvalue_builtin_has_empty_write_frame\b' /agent/repos/omega/omega-rust` → hits in `calls.rs` only (read).
3. Read `BuiltinFunction::has_empty_write_frame` (omega-rust/psi/foundation/symbols/src/builtin.rs:728-740, read) documenting float-family empty frames.
4. At calls:16158-16192 (read), conjunction requires byte-predicate or empty-frame builtin plus negated boundary/requirement/machine/receiver guards before `return Some(())` without adding paths (skips ladder at calls:16275+).
5. **Input:** receiver-free float builtin or byte-predicate call with no boundary signature, no machine state callee, no requirement signature, no boundary receiver.
6. **Verdict: CLOSED** — Empty frame is tied to builtin metadata (read builtin.rs comment) and explicit negated guards; misclassified callees take the ladder or return `None` at calls:16207-16207 / calls:16231-16232 (read snippets).
7. **Unsure:** Full semantics of `byte_predicate_call` (not opened).

### A0362

1. Read `statement_expression_roots_write_frame` (calls:15916-15949, read) and `expression_write_frame_uncached` (calls:15801-15819, read).
2. Search `rg '\bexpression_has_calls\b' /agent/repos/omega/omega-rust/psi/compilation/validation/src/calls.rs` → definition calls:11319-11328 (read).
3. Read caller `statement_value_write_frame_uncached` (calls:15901-15914, read) → state write frames (inferred).
4. At calls:15923-15927 (read), only `expression_has_calls` gates empty frame; `expression_has_calls` matches `ExpressionNode::Call(_)` only (calls:11324-11327, read), not `Atomic`. `expression_write_frame_uncached` also checks `atomic_once_carrier` via `expression_any` (calls:15809-15816, read). `collect_operation_written_paths` handles `ExpressionNode::Atomic` (calls:16112-16142, read) but is not reached after early return.
5. **Input:** statement value roots that are solely an atomic-once write with no `Call` in the executable subtree.
6. **Verdict: REACHABLE** — Smallest harmful shape: a statement whose only effect is an atomic-once mutation on caller storage, compiled through `statement_expression_roots_write_frame`, yields `NormalizedWriteFrame::complete(Vec::new())` at calls:15927 while the expression path would not (calls:15809-15818 vs calls:15923, read).
7. **Unsure:** Whether downstream checks outside write frames reject such statements anyway.

### A0363

1. Read `type_is_caller_isolated_local_inner` through the `TypeReferenceNode` match (calls:17378-17532, read), including calls:17528.
2. Search `rg '\btype_is_caller_isolated_local\b' /agent/repos/omega/omega-rust/psi/compilation/validation` → many hits; read use at calls:6809-6810, calls:11547-11548, calls:11823-11824 (read).
3. Read upward: locals/parameters that are not caller-isolated enter write tracking (calls:11547-11548, read); isolated types skip alias/write registration (inferred from `!type_is_caller_isolated_local` guards).
4. At calls:17528 (read), `TypeReferenceNode::DynamicTrait { .. } => true`.
5. **Input:** local or parameter typed `dyn Trait` with interior reference storage the isolation pass would otherwise track.
6. **Verdict: UNDECIDED** — Code matches entry (read). Isolation semantics are clear; whether executable `dyn` values can hold caller-reachable references was not traced in representation or custody code.
7. **Unsure:** Runtime layout of `DynamicTrait` in this compiler and whether write paths can originate inside trait objects.

### A0364

1. Read `rebase_local_alias_path` in full (calls:17669-17681, read).
2. Search `rg '\brebase_local_alias_path\b' /agent/repos/omega/omega-rust` → production uses in `calls.rs` at calls:7315, 12097, 34403, 37343 (read call sites 7314-7315, 12097-12131, not 34403/37343 bodies).
3. Read `substitutions_are_finite` consumer of rebase at calls:12097-12131 (read): after rebase, unmatched names must map to parameter/receiver or local skip, else `return false`.
4. At calls:17674-17681 (read), `find_map` on `(alias == root)`; `unwrap_or_else(|| relative.to_owned())` keeps the original path when no alias entry matches.
5. **Input:** relative path whose first segment is not an alias key but is still a legitimate parameter/local/receiver spelling.
6. **Verdict: CLOSED** — Identity fallback does not bypass `substitutions_are_finite` (fail-closed at calls:12130-12131, read). `stable_assignment_target_path` uses rebase only after `coarse_place_path` (calls:7314-7315, read).
7. **Unsure:** Callers at calls:34403 and calls:37343 and whether identity fallback can hide overlap in frame-path deduplication.

### What I did not read

- Full bodies of `canonical_write_origins`, `owned_storage_may_hold_inner`, `byte_predicate_call`, and most `flow.rs` consumers of write frames.
- Callers of `boundary_result_candidate_origins` beyond the three direct callers of `exclusive_boundary_result_origins`.
- `rebase_local_alias_path` uses at calls:34403 and calls:37343 in full.
- Remaining arms of `summarize_transition_target_written_paths` after calls:11895.

SLICE COMPLETE
