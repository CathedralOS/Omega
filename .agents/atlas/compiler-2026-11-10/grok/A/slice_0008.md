# slice_0008

Digest preamble (read): `digests/area_08.md` — Area 8 is stage-05 checking over the borrow ledger, contracts (`checks_contracts_*`, prover), scalar values (`checks_current_scalar_values.rs`), and related checks; contracts proving is ~20 OR'd routes per call/exit.

| id | file:line | verdict | one-line reason |
| A0057 | checks_contracts_prover.rs:2836-2843 | UNDECIDED | `decomposed_builtin_meaning` returns true for any top-level `And`/`Or` on Requirement/Proposition, but guarantee admission uses `builtin_meaning`, not this helper. |
| A0058 | checks_contracts_prover.rs:4667-4675 | UNDECIDED | Empty `writes` makes `capture_preserved` return true without overlap test; whether an empty signature ceiling can omit real callee writes was not read. |
| A0059 | checks_contracts_prover.rs:5557 | UNDECIDED | `ProgramPoint` variants `Global`/`Definition`/`Machine` skip the non-`Bool` type guard; whether any `boolean_value_at_place` caller passes those points was not fully traced. |
| A0060 | checks_contracts_prover.rs:7703 | CLOSED | Leaf receipts use `contract: Default::default()`; downstream `valid_effect_tree` only reads `receipt.contract` when `children` is non-empty. |
| A0061 | checks_contracts_prover.rs:7615-7622 | REACHABLE | `overlaps` returns false for leaf-vs-tree pairs, so one instruction event can fund both a direct leaf receipt and a helper-routed child receipt. |
| A0062 | checks_contracts_prover.rs:7384-7386 | REACHABLE | Post-return guarantee matching treats fixed incoming parameter references as preserved without `operand_binding_before_statement_agrees`. |
| A0063 | checks_contracts_prover.rs:7864-7875 | CLOSED | Vacuous `zip` applies only when `input_carriers()` is empty and arity is zero; such ops exist and later validation ties leaves to the call intrinsic. |
| A0064 | checks_current_scalar_values.rs:5073 | UNDECIDED | `Assembly` statements do not call `retire_unpreserved`; whether asm can mutate scalar-tracked state during `replay_prefix` was not traced. |

## A0057

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_prover.rs:2836-2843` — inside `Callable::decomposed_builtin_meaning`, `Requirement`/`Proposition` arms match any `BinaryOperator::And`/`Or` and `return true` before operand checks.

**Enclosing function (read):** `decomposed_builtin_meaning` at `2824-2847`; sibling `builtin_meaning` at `2793-2822` delegates to `has_builtin_parameter_bound_expression_meaning` (validation), which walks binary children (`proof_contracts_core.rs:5927-5953`, read).

**Callers (search):** `rg '\bdecomposed_builtin_meaning\b' omega-rust` → **2** matches (definition + `AvailableGuarantee::has_builtin_meaning` at `2889-2892`). `rg '\.has_builtin_meaning\(' omega-rust` → **3** production uses in `checks_borrows_overlap.rs` (`1583`, `2052`, `2106`); not used in `admit`.

**Upward trace (read):** `admit` (`3285-3353`) gates guarantees with `builtin_predicate` (`3303-3305`), which calls `invocation.callable.builtin_meaning` (`5189-5190`), not `decomposed_builtin_meaning`. `decompose_premise_expression` (`2034+`) after `has_builtin_meaning` still recurses through `And`/`Or` children (`2154-2173`, read).

**Input / accept branch:** A Requirement/Proposition callee `ensures` whose top-level expression is `And`/`Or` — digest claim is unconditional acceptance in `decomposed_builtin_meaning`; whether that alone lets a program compile is **undecided** without tracing every consumer of `has_builtin_meaning`.

## A0058

**Cited site (read):** `checks_contracts_prover.rs:4667-4675` — in the occurrence loop of `capture_preserved`, if no write ceiling place overlaps the rebound occurrence, the function returns `true` immediately.

**Enclosing function (read):** `capture_preserved` `4557-4689`; `signature_ceiling_places` imported at `4587-4595` (`flow_mutation.rs:66-82`, read header only).

**Callers (search):** `rg '\bcapture_preserved\b' omega-rust` → **4** matches — definition, `admit` (`3317-3325`), `guarantee_scalar` post-state branch (`4754-4763`), import in availability module.

**Upward trace (read):** Failed `signature_ceiling_places` (`None`) rejects before `4667` (`4594-4595`). `Some(empty_vec)` makes `writes.iter().any(...)` always false, so every occurrence hits `return true` at `4675`.

**Input:** Programs whose callee guarantee is admitted via `capture_preserved` with an empty write-ceiling vector. Whether a program that should be rejected can have an empty ceiling while the callee still mutates captured state is **undecided** (`selected_signature_ceiling_regions` not read).

## A0059

**Cited site (read):** `checks_contracts_prover.rs:5557` — `_ => None` in `BooleanValueQueries::at_place` `scope` mapping.

**Enclosing function (read):** `at_place` `5520-5589`; non-`Bool` primitive filter at `5559-5570` runs only when `scope` is `Some`.

**Callers (search):** `rg '\bboolean_value_at_place\b' omega-rust` → **13** matches across `checks_contracts_prover.rs`, `checks_contracts.rs`, `published_facts.rs`, `flow_state_values.rs`, `checks_contracts_exits.rs`, `published_facts_crash_calls.rs` (production).

**Upward trace (read):** Example caller `parameter_boolean_proves` passes `ProgramPoint::Statement` (`5807-5816`). `ProgramPoint` enum includes `Global`, `Definition`, `Machine` (`contexts_and_places.rs:1086-1134`, read).

**Input:** Queries at `Global`/`Definition`/`Machine` where the place’s type is not `Bool` but facts might still yield a boolean via `ordinary_at_place` / predicates. Whether any listed production caller supplies those points is **undecided** (not all call sites read).

## A0060

**Cited site (read):** `checks_contracts_prover.rs:7701-7703` — atomic-release leaf `InstructionEffectEvidence` sets `contract: Default::default()`.

**Enclosing function (read):** `evidence_inner` `7646-7890` (also `7877-7880` for register leaf with `Default` contract).

**Callers (search):** `rg '\binstruction_effect_evidence\b' omega-rust` → **2** matches — re-export `8069`, call in `checks_multiplicity.rs:4839-4847`. `rg '\bevidence_inner\b' omega-rust` → **3** matches (definition + recursive calls `7828`, `7646`).

**Upward trace (read):** `InstructionDispositions::settle` publishes receipt then records disposition (`4870-4885`). `valid_effect_tree` (`5131+`, read `5216-5256`): empty `children` validates payload/operation only (`5216-5230`); non-empty `children` requires `facts.proof.contract_facts.is_valid(receipt.contract)` and matches `call.ensures` (`5232-5245`).

**Closing check:** Default contract handle on **leaf** receipts is not consulted on the leaf validation path (`checks_multiplicity.rs:5216-5230`, read).

## A0061

**Cited site (read):** `checks_contracts_prover.rs:7615-7622` — `InstructionEffectEvidence::overlaps`: `(true, true)` → same event point overlaps; mixed empty/child → `false`.

**Enclosing function (read):** `overlaps` `7605-7624`; spent gating in `evidence_inner` `7707-7708`, `7852-7853`, `7883-7884`.

**Callers (search):** `overlaps` only referenced inside `checks_contracts_prover.rs` (methods on evidence + `evidence_inner`).

**Upward trace (read):** Helper-routed proof builds receipts with non-empty `children` (`7846-7850`); direct register match builds leaf receipts (`7877-7881`). `spent` records prior receipts (`checks_multiplicity.rs:4906`, read); only overlapping shapes block reuse.

**REACHABLE (plain words):** Two assembly-fact dispositions at one statement could both settle if one receipt is a leaf on the instruction-effect fact and another is a tree rooted at the same event through a callee `ensures`, because `overlaps(leaf, tree)` is false (`7615-7622`, read).

## A0062

**Cited site (read):** `checks_contracts_prover.rs:7384-7386` — `post_binding_preserved` returns `true` when `frames.expression_is_fixed_incoming_reference(...)`.

**Enclosing function (read):** `post_binding_preserved` `7360-7438`; caller closure in `guarantee_value_matches` `7287-7296` (post-return / `post_state` path).

**Callers (search):** `rg '\bpost_binding_preserved\b' omega-rust` → **2** matches (definition + call site above).

**Upward trace (read):** `expression_is_fixed_incoming_reference` (`calls.rs:22314-22321`, read) forwards to `assignment_targets_parameter_referent`. Otherwise the function requires `operand_binding_before_statement_agrees` (`7436-7438`).

**REACHABLE (plain words):** A program that proves an instruction-effect guarantee using a callee return value tied to a fixed incoming reference parameter can pass preservation without the binding-agreement check that other operands take.

## A0063

**Cited site (read):** `checks_contracts_prover.rs:7864-7875` — when `operands.len() == required.len()` and `carriers` is empty, `zip(carriers).all(...)` is vacuously true.

**Enclosing function (read):** `evidence_inner` `7668-7681` sets `carriers` from `Register(operation).input_carriers()` or `[]` for `AtomicReleaseStore`; arity must match `required.len()`.

**Callers:** Same as A0060 (`instruction_effect_evidence`).

**Upward trace (read):** Zero-carrier register ops exist (`inline_assembly.rs:187-192`, read — e.g. `SegmentRead`, `SystemRead`). Arity mismatch returns `None` (`7679-7680`, read). Leaf validation still matches operation to call intrinsic (`checks_multiplicity.rs:5217-5227`, read).

**CLOSED:** Vacuous match is confined to zero-arity carrier lists; non-empty carriers still require `operand_matches` (`7870-7873`, read).

## A0064

**Cited site (read):** `checks_current_scalar_values.rs:5073` — `StatementNode::Assembly(_) | AssemblyFact(_)` arm is empty (no `retire_unpreserved`). **Related (read):** `4820-4822` skips `is_arm_pattern_marker` statements in `replay_prefix`.

**Enclosing function (read):** `replay_prefix` `4772-5097`; entry `CurrentScalarValues::at_observing_projections` / `at_completing` → `replay_prefix` (`4767`, read). `is_arm_pattern_marker` (`contract_entailment.rs:90316-90321`, read) matches `LocalBindingRole::ArmPattern`.

**Callers (search):** `replay_prefix` only defined/called in `checks_current_scalar_values.rs`; `CurrentScalarValues::at*` used from `checks_contracts*.rs`, `checks.rs`, `flow_place.rs`, `checks_termination.rs` (read grep listing only).

**Upward trace (read):** `Call`/`Expression` statements call `retire_unpreserved` (`5064-5071`); assembly does not. Arm-pattern locals are non-executing markers (read `90316-90321`).

**Input:** Prefix replay toward a transition/statement index for scalar proofs. **Undecided** whether inline assembly that mutates storage tracked in `CurrentScalarValues` can leave stale bindings because asm statements do not retire; arm-pattern skip appears consistent with marker statements (read).

## What I did not read

- `selected_signature_ceiling_regions` / full write-ceiling construction (`flow_mutation.rs` beyond `66-82`).
- Every `boolean_value_at_place` call site’s `ProgramPoint` value.
- Full `valid_effect_tree` after `5276`.
- Inline-assembly lowering vs scalar snapshot retirement beyond the empty match arm.
- Corpus samples exercising double instruction-effect spend or assembly/scalar interaction.

SLICE COMPLETE
