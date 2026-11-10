# slice_0068

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0521 | pcc:2036-2058 | REACHABLE | Domain weakening exits before comparing atoms when the value names an unresolved local initializer or `expression_atoms` is empty. |
| A0522 | pcc:3014, 3100-3103 | REACHABLE | Member typing consults each partial owner environment from contract facts, not necessarily the executable site environment. |
| A0523 | pcc:5565-5572, 5686-5695 | REACHABLE | A symbol missing from locals is treated as immutable via `parameter_mutability` defaulting missing entries to false. |
| A0524 | pcc:5934-5953, 5959 | REACHABLE | User-defined `&`/`<<` binaries skip builtin-meaning checks because those operators are not spelled candidates. |
| A0525 | pcc:6085-6144 | REACHABLE | `validate_checked_call_candidate` returning `None` skips the embed call with no diagnostic. |
| A0526 | pcc:7515-7519 | REACHABLE | `is_boolean_fact_expression` accepts any `Name` or `Call` without checking the expression's type. |
| A0527 | pcc:7156-7160 | REACHABLE | Proposition facts zip arguments to parameters with no arity mismatch diagnostic. |
| A0528 | pcc:8189-8195 | REACHABLE | Pending opaque-property validation sets `allow_pending_copy`, bypassing the copy receipt requirement. |

## A0521

Digest preamble (area_14, lines 1–7): `pcc` is `omega-rust/psi/compilation/validation/src/proof_contracts_core.rs` (read).

Cited site (read): `validate_domain_delivery` (`pcc:2025–2124`) returns early at `pcc:2036–2042` when `expression_reads_unresolved_local_initializer` is true (`arithmetic_domains.rs:694–717`, read) or at `pcc:2056–2058` when `expression_atoms` (`pcc:2300–2348`, read) yields an empty vector, so no implicit-weakening diagnostic runs (`pcc:2117–2124`).

Callers (read): `validate_return_domain_weakening` (`pcc:2001–2021`) → `program_validation.rs:3724–3730` on state return validation; `validate_argument_domain_weakening` (`pcc:1974–1998`) → `calls.rs:40312–40322` on call arguments. Upstream acceptance is `program_validation::validate` (`pv:4067`, cited in area_14 preamble) which records diagnostics but does not re-run domain weakening elsewhere (`grep` `implicit domain weakening` in `omega-rust/psi/compilation/validation`: 2 sites, both in `pcc` and `arithmetic_domains.rs`, read).

Input: a machine state whose return (or call argument) is a bare name of a mutable local whose initializer is still an unresolved bare name, or a value whose shape makes `expression_atoms` empty while the declared target type still carries domain atoms the value would drop.

Verdict **REACHABLE**: such a program can pass return/argument validation without any domain-weakening error.

## A0522

Cited sites (read): `contract_owner_environment` returns `None` for `ContractProofFactOwner::Unknown` (`pcc:3014`). When `owner_index` is absent, `exact_owner_environments` (`pcc:3086–3146`) builds environments only from contract facts that mention the expression (`pcc:3096–3103`), plus other collected owners; it does not automatically include full `machine_environment` for the statement site.

Enclosing usage (read): `checked_member_target_from_exact_owner` (`pcc:2571–2587`) iterates those environments and calls `member_target_in_environment` (`pcc:4018–4040`) per environment, requiring consistency via `retain_consistent` (`pcc:2894–2902`).

Caller search: `checked_member_target_from_exact_owner` — `rg '\bchecked_member_target_from_exact_owner\b' omega-rust` → 5 matches (1 definition, 4 production call sites in `proof_contracts_core.rs`, `pipeline/05_.../shared.rs`, `pipeline/04_.../preparation.rs`, read).

Input: a member access inside a proof fact that appears in multiple contract owners, where one partial `TypeEnvironment` types the receiver so the member resolves, while the executable site environment would not.

Verdict **REACHABLE**: resolution can succeed using a partial owner set rather than the site where the expression runs.

## A0523

Module: `immutable_integer_bounds` (`pcc:4983+`, read).

Cited logic (read): In `normalize_bound` (`pcc:5548–5590`), for a single-segment `Name` with `path.symbol` valid and `head_symbol == symbol`, `local_by_symbol` → `LocalLookup::Missing` (`pcc:5567–5572`) consults `parameter_mutability` (`pcc:5686–5694`), which returns `Some(false)` when `lookup.parameter_entry(symbol)` is `None` (`pcc:5693`), yielding `NormalizedBound::Expression` (immutable). For name-by-string path when local is missing, `normalize_bound` returns `None` (`pcc:5580–5584`) — the digest scenario is specifically the symbol-head path.

Callers (read): `normalize_bound` used by `normalize_immutable_integer_bound_expression`, `projected_integer_bound_*`, `domains_and_predicates.rs:308`, `access_views_and_borrows.rs:2969+`, `literals_and_struct_literals.rs:5207+`, and `program_validation` passes `ImmutableBoundLookup` during statement validation (`program_validation.rs:1543, 3754, 4181`).

Input: an integer bound expression that names a symbol that is neither a state local nor a state parameter (for example a closed capture or foreign symbol handle still marked valid on the path), where mutability should be known but is not in the lookup tables.

Verdict **REACHABLE**: the compiler treats that name as an immutable bound expression.

## A0524

Function (read): `has_builtin_parameter_bound_expression_meaning` (`pcc:5900–5962`) walks binary nodes; only operators with `spelled_binary_operator` (`pcc:5965–5987`) invoke `has_builtin_spelled_expression_meaning` (`pcc:5943–5950`). `BitwiseAnd` and `ShiftLeft` map to `None` (`pcc:5981–5985`), so the check is skipped (`pcc:5959` falls through) and traversal can finish with `true` (`pcc:5962`).

Callers: `rg '\bhas_builtin_parameter_bound_expression_meaning\b' omega-rust` → 13 files / multiple call sites (read listing): `places.rs:2053`, `machine_parameters_and_bindings.rs:2332–2341`, `contract_entailment.rs` (several), `calls.rs:16938`, pipeline scalar checks, re-export `lib.rs:1598`.

Input: a state-parameter bound expression using a user-overloaded bitwise-and or shift-left on non-integer operands, validated only through this helper (for example the guarantee-expression path in `mpb:2332–2341`).

Verdict **REACHABLE**: the bound is classified as having builtin parameter expression meaning even though spelled-operator verification never ran for `&`/`<<`. Other subsystems (`operators.rs`, `expression_types_and_fields.rs`) also call `has_builtin_spelled_expression_meaning` directly, so this is not the sole gate globally, but paths that only call this function are open.

## A0525

Function (read): `validate_integer_embedding_calls` (`pcc:6025–6146`), called from `program_validation.rs:496` (prologue) and `:4882` (facts collection).

Cited branch: when `validate_checked_call_candidate` returns `None` (`pcc:6085–6093`), the loop neither calls `reject` nor pushes to `admitted`; execution continues to the next embed source (`pcc:6144–6145`).

Input: a program containing an `embed` wrapper around a checked call that fails candidacy (unreachable operationally, wrong route, etc.) such that the candidate validator returns `None`.

Verdict **REACHABLE**: the embed is silently omitted from `ValidatedIntegerEmbeddingCall` results with no diagnostic in this function; compilation may proceed unless a later pass requires admitted embed calls (not traced here).

## A0526

Functions (read): `validate_proof_facts` (`pcc:7087–7184`) calls `is_boolean_owned_fact_expression` (`pcc:7440–7471`), which for non-`Match` expressions delegates to `is_boolean_fact_expression` (`pcc:7487–7532`). At `pcc:7515–7519`, `ExpressionNode::Call`, `Name`, `Member`, and `Indexed` all return `true` without inspecting result type. Same predicate is used in `validate_domain_fact_payloads` (`pcc:7062–7066`, read).

Callers of `validate_proof_facts`: `program_validation.rs:1495`, `declarations_core.rs:2242, 2287`, `value_custody_core.rs:3931` (grep count 4 definition + 4 call sites, read).

Input: a machine or domain proof fact whose expression is an integer-valued name or call (not a boolean operator tree).

Verdict **REACHABLE**: validation accepts the fact as boolean-shaped.

## A0527

Cited site (read): In `validate_proof_facts`, `ProofFact::Proposition` handling (`pcc:7127–7181`) builds `parameters` then iterates `expression_handles(application.arguments).iter().zip(parameters)` (`pcc:7156–7160`) with no preceding `arguments.len() != parameters.len()` check (contrast `validate_integer_embedding_calls` at `pcc:6109–6117`, read).

Call path: `validate_proof_facts` from contract/domain validation in `program_validation` and declarations (see A0526).

Input: a stored proposition proof fact with more arguments than parameters (extras ignored) or fewer arguments than parameters (missing positions never checked).

Verdict **REACHABLE**: no arity mismatch diagnostic is emitted from this gate.

## A0528

Function (read): `validate_opaque_data_properties` (`pcc:8182–8209`); copy receipt required only when `properties.multiplicity == Multiplicity::Unrestricted && !allow_pending_copy && !receipts...` (`pcc:8189–8194`).

Caller (read): `validate_data_properties` (`pcc:7842–7868`) passes `allow_pending_opaque_copy` for boundary-opaque definitions. Prologue wiring (`program_validation.rs:547–556`, read): `OpaquePropertyValidation::PendingBuildSelection` sets `allow_pending_opaque_copy` true; `Required(receipts)` sets it false. Pipeline (`checking.rs:408–411`, read) selects pending mode when `allow_pending_opaque_copy` is true at prepare time.

Input: boundary-opaque data marked `[copy]` with no `OpaqueDataPropertyReceipt`, validated under pending build selection.

Verdict **REACHABLE**: the pending prologue accepts `[copy]` without a receipt. When validation later runs with `OpaquePropertyValidation::Required` and `allow_pending_copy` false, the same code path emits the error at `pcc:8196–8199` (read) — full `validate` with required receipts is a separate pass.

## What I did not read

- `machine_calls::fact_call_projections::validate_checked_call_candidate` body (only the `None` branch effect in the caller).
- Whether typed-tree construction always prevents malformed proposition fact arity before `validate_proof_facts`.
- Downstream consumers that require non-empty `integer_embedding_calls` facts.
- `git show 457adf4a11f:...` at each cite (current checkout lines matched cited symbols at the stated ranges).

SLICE COMPLETE

