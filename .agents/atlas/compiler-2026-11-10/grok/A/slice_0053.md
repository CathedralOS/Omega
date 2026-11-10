# slice_0053

Claims: ACCEPT · digest `area_13` (validation, first half — arithmetic domains, borrows, calls, `contract_entailment.rs`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0405 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:56590 | UNDECIDED | Opaque placeholder on missing observation is read; no end-to-end trace from that binding to ordinary program acceptance. |
| A0406 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:58147-58149 | UNDECIDED | Debug-format polynomial atoms are read; whether distinct `StructuralTerm`s can share the same `{:?}` text not determined. |
| A0407 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:60372-60402 | UNDECIDED | Application denotation equality without argument compare is read; soundness through `completed_integer_call` to rejection/acceptance not traced. |
| A0408 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:63233-63239 | UNDECIDED | Caller-isolated shortcut is read; semantics of `type_is_caller_isolated_proof_value` and accept path not read through. |

## A0405

**Cited site (read):** `contract_entailment.rs:56589-56591` inside `invocation_entry::bind_invocation` (`56528-56596`). When `entry_argument_value` returns `None`, the incoming environment maps the parameter name to `StructuralTerm::Opaque("unobserved invocation argument")` instead of failing `bind_invocation`. Digest line numbers match at this checkout (not re-checked with `git show 457adf4a11f:`).

**Enclosing logic (read):** With `requires_value_preservation()`, each formal is paired via `entry_argument_value` (`56552-56567`); `references` use `reference_argument_observation`, otherwise `owned_argument_observation` (`56514-56525`). Failed preservation mode returns raw argument clones (`56542-56550`).

**Callers (read):** `rg '\bbind_invocation\b' omega-rust` → 4 matches, all in `contract_entailment.rs`: definition `56528`, call sites `44062`, `46287`, `58953`. `rg '\bentry_argument_value\b' omega-rust` → 3 matches (definition plus two uses in the same `bind_invocation` zip).

**Downstream judgment (read, partial):** `judge_equation` (`83473-83541`) strips descriptors; non-integer/non-constructor mismatches (including `Opaque` vs concrete) fall through to `StructuralJudgment::Unknown` (`83541`). Two equal `Opaque` strings can be `Proven` via `left == right` (`83489-83490`). `intake_equation_with_purpose` can register rewrites pairing `StructuralTerm::Opaque(_)` with other terms (`52067-52075`). One requires-style check read: `ExitPredicate::proves_observation` treats non-`Proven` `judge_observed_predicate` as failure (`12840-12842`) inside `normal_exit_proves` (`12698+`); that path is not shown to consume the invocation `incoming` vector from `bind_invocation`.

**What would decide REACHABLE vs CLOSED:** A traced path where a program that should be rejected still reaches acceptance because an entry parameter stayed `Opaque` yet a `requires`/proof goal became `StructuralJudgment::Proven` (or a check treated `Unknown` as success). Inverse: proof that every acceptance path requiring the parameter value fails unless observation succeeded.

**Verdict:** UNDECIDED.

## A0406

**Cited site (read):** `completed_integer_comparison` (`58103-58156`), nested `polynomial` (`58128-58150`). Non-add/sub/mul integer leaves become `Polynomial::atom(format!("{term:?}"))` at `58147` and `58149`.

**Enclosing logic (read):** `fold_scalar_binary` (`57966+`) calls `completed_integer_comparison` only after bool/addr fast paths (`58019-58020`); on `Some(verdict)` it returns a folded boolean term; otherwise it continues to literal integer folding or returns `None` (`58022-58024`). Production use in this file includes `intake_observed_integer_bound` (`50915-50927`).

**Callers (read):** `rg '\bcompleted_integer_comparison\b' omega-rust` → 2 matches (definition and single call at `58019`). `rg '\bfold_scalar_binary\b' omega-rust` → 10 matches (definition, tests, and call sites including `50915`, `58348`, module import).

**Risk model (inferred from code, read):** Colliding `{:?}` strings would merge distinct subexpressions into one polynomial atom and could make `completed_integer_comparison` return an incorrect `Some(bool)`, which `fold_scalar_binary` would turn into a constant boolean predicate.

**What would decide REACHABLE vs CLOSED:** Exhibit two unequal `StructuralTerm` values whose `Debug` output is identical under the compiler’s `StructuralTerm` formatter (`80120+` area), or prove impossible.

**Verdict:** UNDECIDED.

## A0407

**Cited site (read):** `same_completed_denotation` (`60372-60402`) in `completed_scalar_calls`. After `same_resolution_payload`, two `Application` terms with equal `target`, equal argument **length**, and equal canonical `result_type` (`60392-60398`) are equal **without** comparing `arguments` elements.

**Contrast (read):** `StructuralTerm::same_resolution_payload` uses `StructuralIdentity::ResolutionInput`, which compares application arguments via `term_lists_match` (`79847-79867`, `79955-79961`).

**Callers (read):** `rg '\bsame_completed_denotation\b' omega-rust` → 6 matches, all in `contract_entailment.rs`: `60282`, `60321`, `60332`, `60372` (def), `60414`, `60438`. Read contexts: `completed_integer_call` selection loop (`60236-60333`); `single_completed_call_denotation` conflict check (`60404-60420`); `completed_call_result_projection` filter (`60424-60446`) — note `60444` still requires matching `same_resolution_payload` across multiple captured lefts.

**Input shape (read):** Two `StructuralTerm::Application` denotations with identical callee target, arity, and canonical result reference, but pairwise different arguments in `arguments`, could be treated as the same completed denotation while `same_resolution_payload` would distinguish them.

**What would decide REACHABLE vs CLOSED:** Trace from mistaken denotation merge through `completed_integer_call` / rewrite intake to a proof or obligation that allows a program that should fail; or show intervening checks (`completed_scalar_observation`, rewrite guards `60294-60317`, `60337-60347`) always block unsound merges before acceptance.

**Verdict:** UNDECIDED (aligns with open item in `records/unit_096.md` on callers `60282`, `60321`, `60414`, `60438`).

## A0408

**Cited site (read):** `owned_arguments::owned_argument_observation` (`63219-63263`). Invalid declared type or reference shell → `None` (`63224-63228`). When `owned_observation_type` is `None` and `frame_resolver().proof_value_is_caller_isolated(declared)` → return `value.clone()` unchanged (`63233-63239`), bypassing carrier matching that follows (`63243-63255`).

**Delegate (read):** `CallFrameResolver::proof_value_is_caller_isolated` (`calls.rs:15088-15091`) memoizes `isolation::type_is_caller_isolated_proof_value(program, reference)`; definition stub at `calls.rs:17363-17367` (body not read).

**Callers (read):** `rg '\bowned_argument_observation\b' omega-rust` → 6 matches in `contract_entailment.rs`: definition `63219`, call sites `8743`, `56524`, `66082`, `76348`, `78597` (plus internal recursion paths).

**Invocation link (read):** `entry_argument_value` routes non-reference formals to `owned_argument_observation` (`56523-56525`); a `None` there feeds the Opaque placeholder in A0405, while the caller-isolated branch returns `Some(value)` without carrier match.

**What would decide REACHABLE vs CLOSED:** Read `type_is_caller_isolated` / `data_definition_is_caller_isolated` with `proof_values: true` and trace whether returning an unobserved actual for such a type can make a proof obligation `Proven` or skip a diagnostic on an ordinary compilable program.

**Verdict:** UNDECIDED.

## What I did not read

- Full `judge_observed_predicate_uncached` and all consumers of invocation `incoming` bindings after `bind_invocation`.
- `isolation::type_is_caller_isolated_proof_value` / `type_is_caller_isolated` implementation beyond `calls.rs:17363-17367`.
- End-to-end checked-tree publication or corpus acceptance gates tied to `completed_integer_call` outcomes.
- Exhaustive `StructuralTerm` `Debug` formatting cases for atom-collision analysis.
- `git show 457adf4a11f:omega-rust/psi/compilation/validation/src/contract_entailment.rs` (current lines matched atlas cites on read).

SLICE COMPLETE

