# slice_0054

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0409 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:73357-73359 | REACHABLE | No flow graph makes `current_value_valid` vacuously true while graph-mode reads still consult stored values. |
| A0410 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:76996 | REACHABLE | Non-value-preserving judges skip the staleness tree walk and always treat observations as preserved. |
| A0411 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:83286-83287 | REACHABLE | With `hypotheses_contradictory`, any logical-not fact is `Proven` without judging the operand. |
| A0412 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:85364-85368 | UNDECIDED | Function omits integer-premise contradiction; whether a contradictory judge reaches `observe` without the outer guard was not proved. |
| A0413 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:88447-88449 | REACHABLE | Early `true` for non-reference declared types runs before the `Identity`/`Opaque`/`LogicalNot` false arms. |
| A0414 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:89539 | REACHABLE | Unsupported expressions become `Opaque(display_name)` and `ResolutionInput` equality is string equality on that text. |
| A0415 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:90209-90216 | REACHABLE | `premises_prove` returns true on contradictory hypotheses without judging the transparent conjuncts. |
| A0416 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:90730-90736 | REACHABLE | Duplicate ensures/requires facts auto-enter `proven` when every nested call target is name-declared only. |

## A0409

**Cited site (read):** `contract_entailment.rs:73351-73359` in `current_value_valid` (`omega-rust/psi/compilation/validation/src/contract_entailment.rs`). Lines match the atlas cite at this checkout.

When `self.storage_graph()` is `None`, the function returns `true` immediately (`contract_entailment.rs:73357-73359`) before anchor or snapshot checks (`contract_entailment.rs:73402-73531`).

**Sibling (read):** `storage_value_transfer_is_current` at `contract_entailment.rs:75288-75296`: with `version: None`, validity is `self.storage_graph().is_none()` (treats missing graph as current). With `Some(version)`, it calls `current_value_valid` (`contract_entailment.rs:75294`).

**Graph mode (read):** `graph_storage_mode` is `self.storage_version.is_some()` (`contract_entailment.rs:73239-73241`), not presence of a graph. `current_storage_value_at` uses `!graph_mode || at.is_some_and(|at| self.current_value_valid(...))` (`contract_entailment.rs:75825-75828`); when graph mode is on but `storage_graph()` is absent, `current_value_valid` still returns `true`.

**Callers (read):** `rg '\bcurrent_value_valid\b' omega-rust` → 10 matches, all in `contract_entailment.rs` (definition plus store-preservation and storage-value paths including `contract_entailment.rs:76751`, `:74576`, `:75827`).

**Accept path (read):** `validate_machine_contract_entailment` (`contract_entailment.rs:90324`) runs structural entailment that retains stored values under judges using the storage helpers above; stale anchored values are not invalidated when no graph is attached.

**Input (plain words):** A machine checked with runtime body values and a storage version set, but whose `CheckingInput` supplies no flow storage graph, still treats every captured store as current when entailment reads it.

**Verdict:** REACHABLE.

## A0410

**Cited site (read):** `contract_entailment.rs:76992-77007` in `entry_storage_observations_preserved`. Cite matches.

If `!self.requires_value_preservation()` the function returns `true` (`contract_entailment.rs:76996-76997`). Otherwise it collects the expression tree and fails when any subtree has `storage_observation_invalidated` (`contract_entailment.rs:76999-77007`).

**`requires_value_preservation` (read):** `contract_entailment.rs:81087-81088` — `self.runtime_body_values && !self.case_facts_only`.

**Callers (read):** `rg '\bentry_storage_observations_preserved\b' omega-rust` → 3 matches in `contract_entailment.rs` (`:76992` def, `:83312`, `:83394` inside `judge_proposition`).

**Upstream (read):** `judge_proposition` for mathematical calls returns `Unknown` when preservation fails (`contract_entailment.rs:83312-83314`); with preservation not required, judging continues to `judge_observed_predicate` (`contract_entailment.rs:83315-83318`). Float closed-equality path also gates on preservation (`contract_entailment.rs:83394-83398`).

**Input (plain words):** A proof-only or case-facts-only structural judge (no runtime value preservation) evaluating a mathematical-call or float-equality proposition that reads storage invalidated in the invalidation set still passes the preservation gate.

**Verdict:** REACHABLE.

## A0411

**Cited site (read):** `contract_entailment.rs:83280-83290` in `judge_proposition`. On `LogicalNot`, if `self.hypotheses_contradictory`, returns `StructuralJudgment::Proven`; else delegates `inner.negated()` (`contract_entailment.rs:83285-83290`).

**Flag sets (read, partial):** `hypotheses_contradictory = true` at `contract_entailment.rs:51417` (refuted case guarantee), `:51993` (constructor identity mismatch), `:52078`, `:52105`, `:52336`, and `structural_arm_judge` ORs vacuous requires at `contract_entailment.rs:47016`. Full intake surface not exhaustively read.

**Callers (read):** `judge_proposition` is invoked throughout entailment including `validate_machine_contract_entailment` filtering (`contract_entailment.rs:90702`) and arm-bound copies (`contract_entailment.rs:90628`).

**Input (plain words):** A machine whose judge already carries `hypotheses_contradictory` can have an ensures fact that is a negation marked proven without examining whether the inner proposition holds.

**Verdict:** REACHABLE (ex falso path is unconditional at the cite; whether the flag itself is always sound was not fully traced).

## A0412

**Cited site (read):** `contract_entailment.rs:85350-85373` in `visit_non_contradictory_arm`. After `structural_arm_judge`, only `judge.hypotheses_contradictory` causes early return (`contract_entailment.rs:85364-85367`); then `visitor.observe` runs (`contract_entailment.rs:85373`).

**Contrast (read):** `arm_premises` at `contract_entailment.rs:85654-85659` also requires `!judge.integer_premises_contradictory()`. `integer_premises_contradictory` is defined at `contract_entailment.rs:53351-53363`.

**Callers (read):** `rg '\bvisit_non_contradictory_arm\b' omega-rust` → 4 matches in `contract_entailment.rs` (definition plus `contract_entailment.rs:44871`, `:45249`, `:45266`). All three call sites pass empty hypotheses and equations (`contract_entailment.rs:44877-44879`, `:45255-45257`, `:45272-45274`).

**Outer guard (read):** Prefix walk checks `hypotheses_contradictory || integer_premises_contradictory()` before calling at `contract_entailment.rs:44843-44850` and `contract_entailment.rs:45260-45263`, but at `contract_entailment.rs:45248-45258` reuses `checked_contradiction_state` and calls `visit_non_contradictory_arm` without re-running the integer contradiction test.

**Verdict:** UNDECIDED — the cite omits the integer check relative to `arm_premises`, but I did not establish a program where `integer_premises_contradictory()` is true while the outer `(integer_premise_revision, guarantees.len)` cache still allows `visitor.observe`.

## A0413

**Cited site (read):** `contract_entailment.rs:88424-88511` in `scalar_operand_is_observed`. Digest text names `Identity`/`Opaque`/`LogicalNot`/`MathematicalBinary` as falsely observed; the implementation orders checks differently.

After shared/owned/descriptor/`ObservedValue` handling, if `crate::reference_shell(program, declared).is_none()` the function returns `true` (`contract_entailment.rs:88447-88448`) before the `match` whose arms mark `Identity`, `LogicalNot`, `Opaque`, `MathematicalBinary` as `false` (`contract_entailment.rs:88504-88510`).

**Callers (read):** `rg '\bscalar_operand_is_observed\b' omega-rust/psi/compilation/validation` → many production uses in `contract_entailment.rs` (scalar proof, storage entry selection at `contract_entailment.rs:75802-75808`, arithmetic judging, etc.).

**Input (plain words):** A scalar proof obligation whose declared type has no reference shell, applied to a structural term that lowered to `Opaque`, `Identity`, `LogicalNot`, or `MathematicalBinary`, is treated as observed at `88447-88448` and never reaches the false arms.

**Verdict:** REACHABLE.

## A0414

**Cited sites (read):** `structural_term` lowers unsupported names to `StructuralTerm::Opaque(display_name(expression))` at `contract_entailment.rs:89539-89541`; similar for unresolved calls `contract_entailment.rs:89638-89640` and members `contract_entailment.rs:89738-89740`.

**Equality (read):** `StructuralIdentity::ResolutionInput` matches `StructuralTerm::Opaque` by string equality on the payload (`contract_entailment.rs:79934-79938`), used by `StructuralTerm::same_resolution_payload` (`contract_entailment.rs:79546-79547`). Separately, `diagnostic_shape_match` always returns `false` for two opaques (`contract_entailment.rs:18966`); resolution input equality is the relevant path for equation intake.

**Input (plain words):** Two distinct unsupported expressions in the same frame that share the same `display_name` text become indistinguishable `Opaque` terms for resolution-style equality.

**Verdict:** REACHABLE.

## A0415

**Cited site (read):** `contract_entailment.rs:90180-90216` in `premises_prove`. Builds a requires judge, runs `intake_transparent_requires`, then returns `judge.hypotheses_contradictory || predicates.into_iter().all(...Proven...)` (`contract_entailment.rs:90209-90216`).

**Callers (read):** `premises_prove` is called from `transparent_call_requirement_entailed` (`contract_entailment.rs:90171-90177`) and an inherited-requirement helper (`contract_entailment.rs:90142`). `transparent_call_requirement_entailed` is used from checked-tree call validation (`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs:3620`, read via grep).

**Input (plain words):** A caller with contradictory hypotheses in the synthesized judge satisfies transparent callee requirements without each conjunct being proven individually.

**Verdict:** REACHABLE.

## A0416

**Cited sites (read):** In `validate_machine_contract_entailment` ensures filtering, when a fact is structurally equal to some requires fact and `fact_call_targets(program, *fact).all_declared`, the fact is pushed to `proven.entry` and dropped from further work (`contract_entailment.rs:90730-90735`).

**`all_declared` (read):** `fact_call_targets` for `ExpressionNode::Call` sets `declared` when `call.target_symbol.is_valid()`, or `resolve_named_expression_call` succeeds, or any machine name equals `call.target.as_str()` (`contract_entailment.rs:91001-91017`); nested subtrees are merged (`contract_entailment.rs:90990-90993`).

**Upstream (read):** `validate_machine_contract_entailment` entry at `contract_entailment.rs:90324`; invoked from `program_validation.rs` (`grep` → `program_validation.rs:191`, `:4756`).

**Input (plain words):** An ensures expression textually the same as a requires fact, containing a call whose target is accepted only by matching the machine name string, is marked proven without structural proof of the ensures body.

**Verdict:** REACHABLE.

## What I did not read

- Exhaustive list of every `hypotheses_contradictory = true` assignment beyond the samples cited for A0411.
- Every consumer of `visit_non_contradictory_arm` visitors to see whether contradictory integer premises change emitted facts (A0412).
- Whether normal checking always attaches a storage graph when `graph_storage_mode` is true (A0409).
- `wiki/spec` prose on storage preservation obligations.
- Terminal-installation or non-`ce` validators for duplicate ensures/requires closure (A0416).

SLICE COMPLETE
