slice_0053

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0405 | ce:56589-56590 | CLOSED | `bind_invocation` continues with an `Opaque` placeholder, but obligation checks require `StructuralJudgment::Proven`, and opaque parameters do not get proven value predicates. |
| A0406 | ce:58147,58149 | UNDECIDED | Polynomial atoms use `format!("{term:?}")`; custom `Debug` is detailed but I did not establish whether two distinct terms can share debug text and yield a false `completed_integer_comparison`. |
| A0407 | ce:60372-60402 | CLOSED | The shortcut arm still ends in `same_resolution_payload`, which compares application arguments via `term_lists_match` (ce:79847-79868, ce:79955-79960). |
| A0408 | ce:63233-63239 | CLOSED | Caller-isolated proof-value types are meant to pass the raw term; callers that need a real observation return `None` or fail the obligation instead. |

### A0405

1. Read `bind_invocation` in full at ce:56528-56596 and `entry_argument_value` at ce:56506-56526 (`omega-rust/psi/compilation/validation/src/contract_entailment.rs`).
2. Search `bind_invocation(` under `omega-rust/`: 4 hits (definition plus call sites at ce:44062, ce:46287, ce:58953; read all three callers’ immediate `?` use).
3. Search `entry_argument_value(` under `omega-rust/`: 3 hits (definition, ce:56559, ce:59111; read ce:59111 in `capture_call_bindings`, which uses `filter_map` and drops failed observations instead of inventing `Opaque`).
4. Traced callers: ce:44062 and ce:46287 pass `incoming` into `recognize_structural_state_leaves` (ce:45366); ce:58953 into `observed_body_paths` / `recognize_structural_state_leaves` and returns `None` when `diagnostics` is non-empty (ce:58994-58995).
5. Input on the accepting branch: `requires_value_preservation()` is true, `entry_argument_value` returns `None` for a parameter, and `bind_invocation` maps that slot to `StructuralTerm::Opaque("unobserved invocation argument")` (ce:56589-56590).

**Verdict: CLOSED.** Program acceptance for data predicates requires `matches!(judgment, StructuralJudgment::Proven)` at ce:12609 (read). `judge_observed_predicate` (ce:53629-53705, read through `judge_equation` call) does not treat the fixed opaque string as proof of a value predicate; `term_is_unobserved_descriptor` is false for bare `Opaque` (ce:88609, read), so the placeholder mainly keeps body simulation alive where `bind_invocation` returning `None` would abort at ce:44062 via `?`.

**Unsure:** I did not read every leaf arm in `recognize_structural_state_leaves_from` (ce:45421+); I only traced the three `bind_invocation` callers and the `Proven`-gated predicate site at ce:12609.

### A0406

1. Read `completed_integer_comparison` in full at ce:58103-58156 and nested `polynomial` (ce:58128-58150).
2. Read caller `fold_scalar_binary` at ce:57966-58021 (invocation at ce:58019, read).
3. Search `fold_scalar_binary(` under `omega-rust/`: 8 hits (definition, ce:58019, logical resolution ce:58348, tests ce:58170+, intake ce:50915; read ce:58333-58349 and ce:50903-50927).
4. Read `Polynomial::atom` at ce:5927-5932 (keys monomials by the atom name string).
5. Read `impl Debug for StructuralTerm` at ce:79981-80082 (read through `Application`).

**Input on the accepting branch:** non-add/sub/mul integer `ScalarBinary` leaves, or other shapes, become `Polynomial::atom(format!("{term:?}"))` (ce:58147, ce:58149).

**Verdict: UNDECIDED.** The code equates polynomial variables by debug text without a follow-up structural equality (unlike the digest note at ce:43757 with payload check at ce:43767). Custom `Debug` includes handles and fields (ce:80014-80028, read), so I could not construct or refute a smallest program where two distinct subterms share `{:?}` text and `completed_integer_comparison` returns a wrong boolean that later becomes `Proven`.

**Unsure:** Whether `ObservedValue` debug forwarding (ce:79987) can hide distinguishing metadata for operands that still reach `polynomial`’s fallback arm.

### A0407

1. Read `same_completed_denotation` in full at ce:60372-60402 and `canonical_completed_denotation` at ce:60360-60369.
2. Read enclosing `completed_integer_call` usage at ce:60282-60333 and `single_completed_call_denotation` at ce:60404-60421 (read).
3. Search `same_completed_denotation(` under `omega-rust/`: 6 hits, all in the same module (read call at ce:60438).
4. Read `StructuralTerm::same_resolution_payload` at ce:79546-79548 and `Application` matching at ce:79847-79868 with `term_lists_match` at ce:79955-79960.

**Input on the accepting branch:** two `StructuralTerm::Application` nodes with equal targets and argument counts after the top-level `same_resolution_payload` check failed (ce:60377-60393).

**Verdict: CLOSED.** The entry’s “arguments are not compared there” matches the `if` guard (ce:60392-60393, read), but the branch body calls `canonical_completed_denotation(left).same_resolution_payload(&canonical_completed_denotation(right))` (ce:60395-60398), and `Application` resolution equality requires pairwise argument match (ce:79864-79867, ce:79955-79960). Canonicalization only rewrites `result_type` (ce:60362-60367, read), so differing arguments still yield false.

**Unsure:** Nothing material; I did not read every `captured_result_equations` registration site.

### A0408

1. Read `owned_argument_observation` in full at ce:63219-63263.
2. Search `owned_argument_observation(` under `omega-rust/`: 6 hits — definition ce:63219; production call sites ce:56524, ce:8743, ce:66082, ce:76348, ce:78597 (read each snippet; did not read test-only bodies).
3. Read `proof_value_is_caller_isolated` at calls.rs:15088-15091 (read) and isolation helper at calls.rs:17363-17368 (read header only).
4. Traced ce:8743-8765: when `owned_argument_observation` returns `None` and the predicate expression mentions the parameter path, `proven = false` and return (read). The ce:63233-63239 branch returns `Some(value.clone())` instead of `None`, so that failure path does not run for caller-isolated types.
5. Traced ce:76348-76350: `None` from observation returns without storing (read).

**Input on the accepting branch:** valid non-reference-shell `declared`, `owned_observation_type` is `None`, and `frame_resolver().proof_value_is_caller_isolated(declared)` is true (ce:63233-63238).

**Verdict: CLOSED.** This is typed behavior for proof values the caller cannot observe (`calls.rs:15088-15091`, read); returning the term avoids inventing observation metadata. Obligation paths that need real observation still fail closed (ce:8747-8765, ce:76349-76350).

**Unsure:** I did not read full `type_is_caller_isolated` recursion in `calls.rs` beyond ce:17363.

## What I did not read

- Full body of `recognize_structural_state_leaves_from` after ce:45630 (except skimming statement dispatch).
- Full `type_is_caller_isolated` in `calls.rs` beyond the entry at ce:17378.
- All test modules at ce:58158+ and ce:63148+.
- Pipeline stages outside `contract_entailment.rs` that consume validation outcomes.

SLICE COMPLETE
