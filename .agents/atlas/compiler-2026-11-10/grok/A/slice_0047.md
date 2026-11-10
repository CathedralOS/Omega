# slice_0047

Claim: ACCEPT | area_13 (digest preamble read at `.agents/atlas/compiler-2026-11-10/digests/area_13.md:1-27`; path shorthand `calls` = `omega-rust/psi/compilation/validation/src/calls.rs`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0357 | omega-rust/psi/compilation/validation/src/calls.rs:8844-8846 | REACHABLE | Non-exclusive reference parameters are skipped when tracing exclusive boundary results, so shared-borrow aliasing is not recorded. |
| A0358 | omega-rust/psi/compilation/validation/src/calls.rs:12128-12129 | REACHABLE | Cycle substitution edges for write origins rooted at a source-state local name are dropped by `continue`. |
| A0359 | omega-rust/psi/compilation/validation/src/calls.rs:11870 | REACHABLE | `TransitionTargetNode::Value` always contributes an empty write list; the value expression is not summarized here. |
| A0360 | omega-rust/psi/compilation/validation/src/calls.rs:16366,16373 | REACHABLE | Syntactic write fallback ignores non-place argument shapes (`Cast`, `Unary`, `Binary`, `Match`, etc.) via `continue`. |
| A0361 | omega-rust/psi/compilation/validation/src/calls.rs:16158-16193 | REACHABLE | Byte-predicate and value-builtin calls exit write collection early when builtin metadata marks them write-free. |
| A0362 | omega-rust/psi/compilation/validation/src/calls.rs:15923-15928 | REACHABLE | Statement-root write frames use only `expression_has_calls`; atomics are not counted (unlike per-expression frames). |
| A0363 | omega-rust/psi/compilation/validation/src/calls.rs:17528 | REACHABLE | `TypeReferenceNode::DynamicTrait` is treated as caller-isolated, so caller-reachable references inside `dyn Trait` are omitted from write frames. |
| A0364 | omega-rust/psi/compilation/validation/src/calls.rs:17669-17681 | REACHABLE | `rebase_local_alias_path` leaves the path unchanged when no alias entry matches the root. |

## A0357

**Cited site (read):** `calls.rs:8828-8846` inside `exclusive_boundary_result_origins` (`calls.rs:8816-8888`). Cites match this checkout.

For each parameter/actual pair, a reference parameter with `!access.is_exclusive()` hits `continue` at `calls.rs:8844-8846` and is not consulted for `owned_storage_may_hold` / `boundary_argument_origins` (`calls.rs:8847-8877`).

**Enclosing function (read):** full `exclusive_boundary_result_origins` through `calls.rs:8888`. Empty origins ⇒ `ExclusiveResultRoute::Unrelated` (`calls.rs:8883-8886`).

**Callers (read):** `rg '\bexclusive_boundary_result_origins\b' omega-rust` → 4 matches, all in `calls.rs` (definition plus `boundary_result_origins` at `calls.rs:8701`, machine/requirement result paths at `calls.rs:8946`, `calls.rs:8998`).

**Upstream (read):** `boundary_result_origins` (`calls.rs:8654-8722`) appends `invocation_result_origin` when route is `Unrelated` and receiver origins are empty (`calls.rs:8716-8718`). Invoked from `boundary_argument_origins` when `selected_boundary_signature` (`calls.rs:8610-8618`). Also `calls.rs:18466`.

**Input that reaches the branch (read):** A boundary call whose exclusive return referent may alias storage only reachable through a **shared** (`!is_exclusive()`) reference argument; that argument is skipped, so result origins do not alias the borrowed storage.

**Accept path (inferred):** Write-frame / alias-origin computation under-approximates overlap; downstream overlap checks that rely on recorded origins may treat the program as safe when rejection would require seeing the alias. Full compile accept path to Checked Trees not traced end-to-end.

**Verdict:** REACHABLE — smallest program shape: a boundary call returning an exclusive handle that aliases caller storage passed only as a shared-borrow argument; the compiler records an unrelated or synthetic invocation origin instead of the shared-borrow path.

## A0358

**Cited site (read):** `calls.rs:12118-12132` inside `substitutions_are_finite` (`calls.rs:12051-12148`). Cites match.

For each canonical write source, roots resolved from `source.path`: parameter and receiver map to substitution roots; if the root name is in `equation.locals`, `continue` at `calls.rs:12128-12129`; unknown names `return false` (`calls.rs:12130-12131`).

**Callers (read):** `rg '\bsubstitutions_are_finite\b' omega-rust` → 2 matches in `calls.rs` (`calls.rs:11439` in cycle-frame summarization; definition `calls.rs:12051`). Consumer in `flow.rs:2404` reads `storage_origin_substitutions_are_finite` (not opened).

**Upstream (read):** `calls.rs:11433-11447` — when there is a transition cycle and `substitutions_are_finite` returns true, cycle-frame summarization proceeds; otherwise `return None` at `calls.rs:11446`.

**Input (read):** A multi-state cycle where a transition argument’s write origin is spelled as a **local** in the source state (`equation.locals` contains the root name); that contribution never inserts a `substitutions.insert` edge.

**Verdict:** REACHABLE — a cycle whose growth depends on mapping a parameter write through a source local can be judged substitution-finite without that edge, potentially admitting a cyclic write pattern that should be rejected.

## A0359

**Cited site (read):** `calls.rs:11868-11870` in `summarize_transition_target_written_paths` (`calls.rs:11842-11937`). `TransitionTargetNode::Value(_) => Some(Vec::new())` matches digest.

**Enclosing function (read):** Full function through `calls.rs:11937`; `Named` targets summarize callee-state writes (`calls.rs:11871-11934`); `Terminal` also empty (`calls.rs:11869`).

**Callers (read):** `rg '\bsummarize_transition_target_written_paths\b' omega-rust` → 3 matches in `calls.rs` (definition; use in state-write walk at `calls.rs:27234-27245`).

**Upstream (read):** `StatementNode::Transition` branch at `calls.rs:27202-27258` only merges `transition_writes` from this helper; it does not walk a `Value` target’s expression for writes (`calls.rs:27234-27257`). Contrast: `TransitionTargetNode::Value(expression)` carries an expression elsewhere (e.g. call-site numbering `calls.rs:10127-10131`) but not here.

**Input (read):** A state transition whose target is `Value(call_or_side_effect)` where that expression writes caller-visible storage; `transition_writes` is empty.

**Verdict:** REACHABLE — a machine that performs a side-effecting call only as a `Value` transition target can get an under-reported state write frame.

## A0360

**Cited site (read):** `calls.rs:16363-16373` in `syntactic_call_written_paths` (`calls.rs:16341-16381`). Cites match; digest marked [091; checked].

**Enclosing function (read):** Builds receiver `self` path, then for each argument matches expression kind: exclusive `Borrow` records place; shared `Borrow` `continue`; `Name`/`Member`/`Indexed` record; `StructLiteral`/`ArrayLiteral`/`Call` `return None`; **`_ => continue`** skips `Cast`, `Unary`, `Binary`, `Match`, etc.

**Callers (read):** `rg '\bsyntactic_call_written_paths\b' omega-rust` → 7 matches in `calls.rs` (definition; `conservative_call_written_paths` `calls.rs:16394`; cycle-frame path `calls.rs:11727`; demand collector `calls.rs:16305`; state-write walk `calls.rs:27179`; re-exports/imports).

**Upstream (read):** Last rung in `collect_operation_written_paths` call arm at `calls.rs:16301-16313` when `exact_receiver && !has_divergent_actual` after known/boundary/requirement paths fail.

**Input (read):** A call whose only written actual is buried in a `Cast`/`Unary`/`Binary`/`Match` argument (e.g. exclusive borrow inside a cast), and inference falls through to syntactic fallback.

**Verdict:** REACHABLE — the call’s write set omits those arguments, so overlap/conservation checks see fewer writes than the program performs.

## A0361

**Cited site (read):** `calls.rs:16158-16193` in `collect_operation_written_paths` (`calls.rs:16100+`). Cites match.

**Enclosing function (read):** Large `match` on expression; cited block is early return `Some(())` without pushing paths when `byte_predicate_call(...).is_some()` OR `value_builtin_has_empty_write_frame(program, call)` and a chain of guards on boundary signature, machine state, requirement signature, and receiver boundary needs all pass (`calls.rs:16164-16191`).

**Helper (read):** `value_builtin_has_empty_write_frame` at `calls.rs:16325-16338` — true when `builtin_function_for_symbol` has `has_empty_write_frame()` or asm flag intrinsic (`calls.rs:16334-16337`).

**Callers (read):** `rg '\bvalue_builtin_has_empty_write_frame\b' omega-rust` → 3 matches in `calls.rs` (definition and two uses in same function).

**Input (read):** A call classified as a byte predicate or toolchain value builtin with empty-write metadata, satisfying the guard conjunction, even if the call could write through operands the syntactic path would have recorded.

**Verdict:** REACHABLE — a program invoking such a call is treated write-free for frame collection when the builtin flag says so; mistaken or coarse metadata yields acceptance with an incomplete write frame.

## A0362

**Cited site (read):** `calls.rs:15923-15928` in `statement_expression_roots_write_frame` (`calls.rs:15916-15953`).

**Contrast (read):** `expression_write_frame_uncached` at `calls.rs:15809-15818` returns empty only when **both** `!expression_has_calls(...)` **and** no `atomic_once_carrier` in the subtree via `expression_any`. Statement path tests **only** `expression_has_calls` (`calls.rs:15923-15925`).

**`expression_has_calls` (read):** `calls.rs:11319-11328` — true only for nested `ExpressionNode::Call(_)`, not atomics.

**Callers (read):** `rg '\bstatement_expression_roots_write_frame\b' omega-rust` → 3 matches in `calls.rs` (definition; `statement_value_write_frame_uncached` `calls.rs:15908`; indirect via caches).

**Upstream (read):** `collect_operation_written_paths` records atomic-once writes at `calls.rs:16112-16141` when reached; statement fast-path at `15927` returns `NormalizedWriteFrame::complete(Vec::new())` before `with_caller_origins`.

**Input (read):** A statement whose value root is solely an atomic-once operation (no nested `Call` nodes).

**Verdict:** REACHABLE — statement-level inferred write frame is empty while expression-level logic would include the atomic write place.

## A0363

**Cited site (read):** `calls.rs:17528` in `type_is_caller_isolated_local_inner` (`calls.rs:17378-17653`). `TypeReferenceNode::DynamicTrait { .. } => true` matches digest [091; checked].

**Enclosing function (read):** `true` means caller-isolated (no caller-reachable references inside). `Reference` and most other shapes recurse or return `false` (`calls.rs:17529-17531`).

**Callers (read):** `rg '\btype_is_caller_isolated_local\b' omega-rust` → 34 matches in `calls.rs` (public wrapper `calls.rs:17348-17353`; uses e.g. skip recording locals/parameters with isolated types `calls.rs:11823`, `calls.rs:26712`, `calls.rs:10925` — not all opened).

**Input (read):** Storage or parameters typed with `dyn Trait` where interior references could be written or borrowed; isolation treats the type as not carrying caller-reachable references for frame purposes.

**Verdict:** REACHABLE — writes or borrows through `dyn Trait` storage can be omitted from write-frame construction, accepting programs that should fail overlap or conservation checks.

## A0364

**Cited site (read):** `calls.rs:17669-17681` — `rebase_local_alias_path` in `local_aliases` module. Cites match.

**Enclosing function (read):** Splits `relative` into root/suffix; `find_map` on `aliases` for matching `alias == root` projects through `project_write_path`; **`unwrap_or_else(|| relative.to_owned())`** when no match (`calls.rs:17681`).

**Callers (read):** `rg '\brebase_local_alias_path\b' omega-rust` → 8 matches in `calls.rs` (definition; `substitutions_are_finite` `calls.rs:12097`; `stable_assignment_target_path` `calls.rs:7315`; `expand_write_paths` uses `calls.rs:37343`; other write-path closing).

**Input (read):** A path spelled with a local alias name that is **not** listed in `aliases` (stale name, shadowing, or incomplete alias table); rebasing leaves the path rooted at that name.

**Verdict:** REACHABLE — downstream frame logic treats the path as a real place rooted at the alias string instead of failing or rebinding, which can accept incorrect write/alias relationships.

## What I did not read

- Full `CallFrameTables::build` / diagnostic emission when write frames are opaque vs complete.
- Every consumer of under-reported write frames in `borrow.rs`, `access_views_and_borrows.rs`, and flow checking beyond cited call sites.
- `flow.rs:2404` body for `storage_origin_substitutions_are_finite`.
- Language-guide semantics of `Value` transitions and whether side-effecting value targets are ill-formed by specification.
- Toolchain definitions of `has_empty_write_frame` for every value builtin and byte predicate.
- Corpus witnesses for any row (no programs run; no build).

SLICE COMPLETE

