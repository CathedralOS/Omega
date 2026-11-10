# Discharge on the lowered form

This crate decides the obligations that
[structure-only elaboration](../structure-elaboration/README.md) mints, and it
accepts one only when the proof kernel accepted a certificate for it. It is
the third piece of
[check on the lowered form](../../../../../wiki/drafts/designs/check_on_lowered_form.md):
every other outcome is a `Refusal` with reason `Unproved`. There is no
interval verdict, no admitted route and no assumption, so a discharged
obligation is one that [stage trust](../../../../../wiki/drafts/designs/stage_trust.md)
rule 2 lets remove a runtime check. It is not wired into the default compile.

Start at [lib.rs](src/lib.rs) (`discharge`). [graph.rs](src/graph.rs) answers
dominance between program points; [prove.rs](src/prove.rs) is the one prover.

## What it decides

| Kind | Goal | Hypotheses available at its point |
| --- | --- | --- |
| `OperationSafety` | Terminal semantics' canonical goal for the operation (`proof_bearing_scalar_leaf_semantics`): representability of an exact add, subtract or multiply, a defined division or remainder, a shift count in range | entry `requires`, result equations of the operations before it, callee `ensures` after earlier calls |
| `CallRequirement` | the callee's `requires` clause at that position, its formals replaced by the call's actuals | the same, before the call |
| `NormalGuarantee` | this machine's `ensures` clause at that position | the same, on the exit edge |

Any other kind is refused as `Unsupported`; elaboration mints none yet.

Hypotheses come only from the lowered form and the tables. `discharge` fills
each machine's `knowledge` table with one row per entry `requires` clause
(`EntryRequirement`, at the entry block), per operation result equation
(`OperationSemantics`, on its normal successor) and per callee `ensures`
clause (`CalleeGuarantee`, after the call). A row is passed to the kernel for
an obligation only when its point dominates the obligation's point.
Requires and guarantees go in as assumptions, result equations as semantic
axioms, which is how the kernel's definition rules cite them.

## The prover

One goal-directed prover serves all three kinds. It carries interval bounds as
proof: each integer value gets a best upper and lower literal bound, each with
a node the kernel checks, from the carrier bound of its type, a hypothesis
bounding it or relating it to another value, or, for an exact add or subtract
result, its operands' bounds through `IntegerExactAddDefinitionBound`. A
comparison closes by chaining the two sides' bounds through a closed literal
relation. An arithmetic safety goal closes through `IntegerAffineBound`, whose
image of the operands' bounds the kernel itself computes.

`verify_obligation` then decides the whole certificate. Its acceptance records
whether the mathematical core judged it as well; every certificate in the
tests was judged (`MathematicalCoreDecision::Judged`).

## Why this route and not the arithmetic engine

The existing certificate builders (`proof/src/checker.rs`, module
`certificate`) take typed-tree expression handles and a `ProofPlan`, so they
cannot be driven from Terminal values; only their kernel interface
(`ProofNode`, `verify_obligation`) can. The arithmetic engine
(`validation/src/contract_entailment.rs`, `arithmetic_domains.rs`) decides by
interval and relational search and emits no derivation; making it emit one
the kernel re-decides would mean threading proof terms through it, and its
hypotheses are bound to source shapes. The kernel already has the rules this
subset needs, so the prover emits them directly.

## Findings

1. **No justification for a branch condition.** `Justification` has no
   variant for the condition of a conditional edge, so a guard such as
   `when a < 10` gives the arm no knowledge. Obligations that need a guard
   are `Unproved`. `values.rs` is not this crate's to change.
2. **Knowledge rows point at a table that does not exist.**
   `KnowledgeRow::proposition` is a `PropositionId` into "the module's
   proposition table", which Terminal Psi does not have. This crate mints the
   proposition in lockstep with the row (`MachineDischarge::propositions`,
   `PropositionId` n for `KnowledgeId` n). The justification already
   identifies the proposition, so the field could go.
3. **Summaries name no proposition.** `Summary` no longer holds
   `requires` and `result_relation`: the callee's clauses live once in its
   `MachineContract`, identified by machine and position, which is what
   `CallRequirement` and `CalleeGuarantee` use. `CheckTables::summaries`
   stays empty; the discharge reads callee contracts from the module.
4. **Shift-left representability is not built.** The kernel has a direct
   shift route (`map_direct_shift_left_bound`); this prover does not build
   its evidence, so an exact `<<` whose count is not a literal zero is
   `Unproved`. No multiply result bound is built either: the kernel has no
   multiply definition rule.
