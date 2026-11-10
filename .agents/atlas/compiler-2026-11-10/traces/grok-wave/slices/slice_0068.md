slice_0068

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0521 | proof_contracts_core.rs:2036-2058 | REACHABLE | `validate_domain_delivery` returns before any weakening diagnostic when the value reads an unresolved local initializer or `expression_atoms` is empty. |
| A0522 | proof_contracts_core.rs:3014,3100-3103 | REACHABLE | `exact_owner_environments` can build a nonempty environment list while skipping `ContractProofFactOwner::Unknown` and other `None` owners. |
| A0523 | proof_contracts_core.rs:5565-5695 | REACHABLE | A valid single-segment name missing from the local table and not a mutable parameter becomes `NormalizedBound::Expression`. |
| A0524 | proof_contracts_core.rs:5934-5962 | REACHABLE | `ShiftLeft`/`ShiftRight`/`BitwiseAnd` skip `has_builtin_spelled_expression_meaning`; the walk can still end in `true`. |
| A0525 | proof_contracts_core.rs:6085-6144 | CLOSED | `None` from `validate_checked_call_candidate` only skips admission; failures already call `reject` inside `checked_call_target_candidate` (mcc:6225-6334). |
| A0526 | proof_contracts_core.rs:7515-7519 | REACHABLE | `is_boolean_fact_expression` treats any `Name` or `Call` as boolean-shaped without checking the expression type. |
| A0527 | proof_contracts_core.rs:7156-7160 | REACHABLE | Proposition facts zip arguments to parameters with no check for extra unmatched arguments. |
| A0528 | proof_contracts_core.rs:8189-8195 | CLOSED | `[copy]` without a receipt is allowed only when `allow_pending_opaque_copy` is true (`PendingBuildSelection`, pv:549-550). |

### A0521

1. Read `validate_domain_delivery` in full (pcc:2025-2140, read) and `expression_atoms` (pcc:2300-2348, read).
2. Search `rg 'validate_domain_delivery' omega-rust` — 3 hits, all in `proof_contracts_core.rs` (definition plus two internal call sites).
3. Search `rg 'validate_argument_domain_weakening' omega-rust` — 2 files / 3 hits (`proof_contracts_core.rs`, `calls.rs:40312`). Search `rg 'validate_return_domain_weakening' omega-rust` — 2 hits (`program_validation.rs:3724`, definition). Search `rg 'validate_implicit_domain_weakening' omega-rust` — 3 files / 6 hits (`program_validation.rs`×3, `literals_and_struct_literals.rs`×2, definition). Read `program_validation.rs` return path (3724-3731, read) and `calls.rs` argument path (40312-40322, read); did not read every literal call site.
4. Input on the accepting branches: a return, assignment, or call argument whose handle is valid, with a valid target type, but either (a) the value is a one-segment name for a local whose initializer is an unresolved bare name (`expression_reads_unresolved_local_initializer`, arithmetic_domains.rs:694-717, read), or (b) `expression_atoms` returns `[]` (pcc:2055-2057) because the expression shape carries no atoms and `declared_place_type_raw` yields nothing (pcc:2343-2348).
5. **Verdict: REACHABLE** — A program could return or assign a value that still carries stricter domain atoms than the target type, yet skip the “implicit domain weakening” errors at pcc:2117-2126 because the function returns early (pcc:2036-2042, 2056-2057) instead of comparing `source` to `target`.
6. Unsure whether typed-tree lowering always prevents unresolved bare-name locals before validation; did not trace the full name-resolution pipeline.

### A0522

1. Read `contract_owner_environment` in full (pcc:2947-3016, read) and `exact_owner_environments` scan loop (pcc:3086-3146, read).
2. Search `rg 'exact_owner_environments' omega-rust` — 6 hits in `proof_contracts_core.rs` (definition plus five call sites); read `checked_member_target_from_exact_owner` (pcc:2571-2587, read) and `checked_machine_call_target_from_type_owners` (pcc:2662-2684, read); did not read all `infer_expression_type` callers.
3. `ContractProofFactOwner::Unknown => None` at pcc:3014 drops that owner; the loop at pcc:3096-3103 only pushes when `contract_owner_environment` returns `Some`, so the environment vector can be a strict subset of all proof contexts containing the expression.
4. Input: an expression shared by contract proof facts where some matching owners are `Unknown` or otherwise map to `None`, while remaining owners still yield environments; member or call-target resolution then runs only over that partial list (`checked_member_target_from_exact_owner` at pcc:2580-2586 uses `retain_consistent` across the partial set only).
5. **Verdict: REACHABLE** — If a member or call target is consistent across the collected environments but would disagree in a skipped owner environment, resolution can succeed where a full owner set would fail; `checked_machine_call_target_from_type_owners` marks `complete = false` only when a collected environment lacks a target (pcc:2676-2680), not when owners were omitted upstream.
6. Did not enumerate every `ContractProofFactOwner` variant that can yield `None` besides `Unknown`.

### A0523

1. Read `normalize_bound`, `parameter_mutability`, `local_by_symbol`, and `bound_name_receiver` in full (pcc:5548-5702, read).
2. Search `rg 'normalize_bound' omega-rust` — 5 hits, all in `proof_contracts_core.rs`; public callers include `normalize_immutable_integer_bound_expression` (pcc:5169-5177, read). Did not trace every caller in pipeline `05_typed-trees-to-checked-trees`.
3. At pcc:5565-5572, `LocalLookup::Missing` for `path.symbol` with `parameter_mutability` returning `Some(false)` (pcc:5693: missing parameter entry ⇒ `Some(false)`) yields `NormalizedBound::Expression(expression)` — treating the name as an immutable bound without proving it is a state local or parameter.
4. Input: single-segment `ExpressionNode::Name` with valid `path.symbol == path.head_symbol`, symbol absent from `ImmutableBoundLookup` locals/parameters but not classified as mutable.
5. **Verdict: REACHABLE** — A bound position (slice length, range endpoint, etc.) can accept a free name that is neither a resolved local nor a parameter as a normalized immutable bound; programs that should be rejected for an invalid bound name may proceed through bound normalization.
6. Did not read full `ImmutableBoundLookup` construction in `program_validation.rs`.

### A0524

1. Read `has_builtin_parameter_bound_expression_meaning` and `spelled_binary_operator` in full (pcc:5900-5987, read); read `has_builtin_spelled_expression_meaning` (typed-trees `operator_declarations.rs:1847-1917, read).
2. Search `rg 'has_builtin_parameter_bound_expression_meaning' omega-rust` — 9 files / 15+ hits (pcc, `machine_parameters_and_bindings.rs`, `places.rs`, `contract_entailment.rs`, pipeline checks, etc.); this function is not the only operator gate globally but is the dedicated parameter-bound walk.
3. For `ExpressionNode::Binary`, operators with `spelled_binary_operator` = `None` (including `ShiftLeft`, `ShiftRight`, `BitwiseAnd`, pcc:5978-5985) skip the `has_builtin_spelled_expression_meaning` test (pcc:5934-5952); traversal may still finish with `return true` (pcc:5962).
4. Input: parameter-bound expression tree whose subexpressions are only “unspelled” binary operators (e.g. user-defined `<<` or `&` still encoded as those `BinaryOperator` variants) plus leaves that pass the walk.
5. **Verdict: REACHABLE** — A bound using a user-defined shift or bitwise operator on a non-integer carrier can satisfy this gate and return `true` even when builtin meaning does not exist; callers such as `places.rs:2053-2055` and `calls.rs:16938-16944` then treat the expression as having builtin bound meaning or continue evaluation.
6. Did not prove whether an earlier typing stage always rejects such bounds before validation.

### A0525

1. Read `validate_integer_embedding_calls` loop (pcc:6025-6146, read) and `validate_checked_call_candidate` / `checked_call_target_candidate` (mcc:6225-6336, read).
2. Search `rg 'validate_checked_call_candidate' omega-rust` — 3 hits (`proof_contracts_core.rs:6086`, `machine_calls_core.rs:6138,6214,6225`).
3. When `if let Some((machine, state)) = validate_checked_call_candidate(...)` is false (pcc:6085-6144), the embed source is not pushed to `admitted`; the outer block emits no additional error.
4. Input: embed source call that fails any `checked_call_target_candidate` check (wrong target, impure closure, etc.); `reject` is invoked on every `return None` path in mcc:6263-6334 (read).
5. **Verdict: CLOSED** — Absence from `admitted` is not silent acceptance: `reject` at pcc:6058-6062 already records a diagnostic via the closure passed into `validate_checked_call_candidate`.
6. Entry description matches read behavior; the “no error here” note refers to the outer `if let`, not absence of diagnostics.

### A0526

1. Read `validate_proof_facts` (pcc:7087-7184, read), `is_boolean_owned_fact_expression` (pcc:7440-7472, read), and `is_boolean_fact_expression` (pcc:7487-7533, read).
2. Search `rg 'validate_proof_facts\(' omega-rust` — 4 call sites (`declarations_core.rs:2242,2287`, `program_validation.rs:1495`, `value_custody_core.rs:3931`); read declaration machine/state contract paths (declarations_core.rs:2242-2295, read).
3. Non-`Match` facts delegate to `is_boolean_fact_expression`; `Name(_)`, `Call(_)`, `Member(_)`, and `Indexed(_)` arms return `true` without inspecting type (pcc:7515-7519).
4. Input: `ProofFact::Expression` in a machine `requires`/`ensures` or data `where` fact whose root is an integer-typed name or call; `validate_proof_facts` skips the boolean-shaped error at pcc:7110-7114.
5. **Verdict: REACHABLE** — A contract fact that is semantically an integer (or non-boolean) expression can pass the boolean-shape gate and allow validation to continue for declarations validated through `validate_proof_facts`.
6. `is_boolean_domain_fact_expression` (pcc:7409-7437) is stricter for domain facts but is a different path; did not verify all fact owners use it.

### A0527

1. Read `validate_proof_facts` proposition arm (pcc:7127-7181, read).
2. Same caller search as A0526; no separate arity validator found in this function.
3. `expression_handles(application.arguments).iter().zip(parameters)` (pcc:7156-7160) pairs only the shorter prefix; extra arguments are never examined; no diagnostic for surplus arity in this function (read).
4. Input: `ProofFact::Proposition` with more runtime arguments than `program.proposition_parameters(declaration)` length, on paths through `validate_machine_contracts` / `validate_state_signature_contracts` (declarations_core.rs:2242-2295).
5. **Verdict: REACHABLE** — A machine contract proposition fact with too many arguments can validate without an arity error; extra arguments are silently ignored at this gate.
6. Search `rg 'arguments.len() != parameters.len()' omega-rust/psi/compilation/validation` shows arity checks elsewhere (e.g. embed calls pcc:6109-6117) but not for generic contract `validate_proof_facts` propositions.

### A0528

1. Read `validate_opaque_data_properties` (pcc:8182-8209, read) and `validate_data_properties` (pcc:7842-7868, read).
2. Search `rg 'allow_pending_opaque_copy|allow_pending_copy' omega-rust` — only `program_validation.rs:547-556` and `validate_data_properties` parameter (read pv:547-556).
3. The `[copy]` receipt requirement is guarded by `!allow_pending_copy` (pcc:8190); when true, unrestricted-multiplicity opaque data with `[copy]` and no receipt does not error (pcc:8189-8195).
4. Input: `OpaquePropertyValidation::PendingBuildSelection` prologue (`program_validation.rs:550`: `allow_pending_opaque_copy = true`) before build receipts are supplied.
5. **Verdict: CLOSED** — Acceptance is tied to the explicit pending-build prologue mode; `Required(receipts)` sets `allow_pending_opaque_copy` false (pv:549), restoring the receipt check for normal validation.
6. Not a reachability hole on the `Required` path unless the driver always leaves pending mode set (not read here).

### What I did not read

- Full bodies of `infer_expression_type`, `collect_parameter_constraint_environments`, and all `exact_owner_environments` helper collectors beyond the scan at pcc:3122-3145.
- Typed-tree name-resolution guarantees for unresolved locals and proposition arity before validation.
- Entire `checked_call_target_candidate` callers besides embed validation.
- Pipeline-stage consumers of `normalize_immutable_integer_bound_expression` in `05_typed-trees-to-checked-trees`.

SLICE COMPLETE
