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
dominance between program points; [places.rs](src/places.rs) says what a
place holds when a field read observes it; [prove.rs](src/prove.rs) is the
one prover.

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
(`OperationSemantics`, on its normal successor), per field read whose place
holds one value on every path to it (`OperationSemantics` of the read; see
below), per callee `ensures` clause (`CalleeGuarantee`, after the call) and
per edge of a conditional (`BranchCondition`: the condition is `true` on the
true edge and `false` on the false edge). Each row's proposition is in `MachineTables::propositions`. A row is passed to the kernel for
an obligation only when its point dominates the obligation's point.
Requires and guarantees go in as assumptions, result equations as semantic
axioms, which is how the kernel's definition rules cite them.

## Reads of places

A field read produces a fresh value. It gets the row `read == v` when `v` is
the one value its place holds on every path to it
([fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md):
"A primitive read may introduce an equality between its copied SSA result
and an exact reaching stored SSA value. Every actual predecessor must supply
that same value"). [places.rs](src/places.rs) finds that from the tables
alone:

- **The place.** The read's root is `roots[source]`, and each field of its
  path is one binary search of a parent's `children`.
- **Overlap.** One function answers whether two places may share storage: a
  place overlaps itself, its ancestors and its descendants; two places that
  part by two different record fields are disjoint; any other pair of sibling
  segments overlaps, and places under different roots do not. Later lanes
  extend this function (indices, referents) and add no second relation.
- **Reaching.** Walking the graph back from the read, the first write to an
  overlapping place on each path is the one that reaches along it. A write of
  `Content::Value(v)` to exactly the read place supplies `v`. Any other
  overlapping write supplies no value. A path that reaches the machine's
  entry with no write is supplied by the place's entry content
  (`ValueOrigin::EntryContent`).

One value from every arrival gives the equation. Different values on
different arrivals, or contents with no value identity, give none: the read
is still well formed and nothing is known about it. A path that reaches the
entry of a place with no entry content is a table that does not describe the
module, and the machine is refused.

Knowledge names values only. No row states a proposition about a place and
no structural `Proposition` variant is used, so the prover gained no
field-aware path: it sees one more equation between two values.

A call's actuals are replaced into the callee's clauses by position in the
scalar lane: the callee's `k`-th scalar formal by the caller's `k`-th scalar
actual. A record actual is a place and no clause term can name it, because
elaboration refuses a clause that names or projects a record formal.

## The prover

One goal-directed prover serves all three kinds. It carries interval bounds as
proof: each integer value gets a best upper and lower literal bound, each with
a node the kernel checks, from the carrier bound of its type, a hypothesis
bounding it or relating it to another value, or, for an exact add or subtract
result, its operands' bounds through `IntegerExactAddDefinitionBound`. A
comparison closes by chaining the two sides' bounds through a closed literal
relation. An arithmetic safety goal closes through `IntegerAffineBound`, whose
image of the operands' bounds the kernel itself computes.

The same prover carries relations between values, with no literal endpoint.
A comparison between two values searches order steps between values:
hypotheses relating two values, and `a < a + d` or `a - d < a` from an exact
result when `0 < d` (`IntegerAddOrder`, `IntegerSubtractOrder`), chained by
order transitivity. An unsigned `a - b` is safe from `b <= a` through the
kernel's correlated unsigned subtract. A goal the direct search misses is
split over a disjunctive fact (a disequality guard transports to `v < k or
k < v`): each alternative proves the goal as a local assumption or
contradicts what is known (`DisjunctionElimination`, contradiction
elimination). Splits do not nest.

`verify_obligation` then decides the whole certificate. Its acceptance records
whether the mathematical core judged it as well. Most certificates are
`Judged`; a discreteness step from a transported strict guard (`a < 10` to
`a <= 9`, `0 < a` to `1 <= a`) is accepted by the rule checker and answered
`Refused` by the core. Both count as discharged, and the comparison harness
reports them apart.

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

1. **Branch facts reach the prover through the kernel.** A `BranchCondition`
   row states `c == true` (or `false`); the prover transports it through the
   result equations of Boolean values and constants into the comparison it
   stands for (`ValueEqualityTransport`, whose conclusion the kernel
   computes with `check_value_equality_denotation`). The mathematical core
   does not cover every certificate this makes: a discreteness step from a
   transported `a < 10` to `a <= 9` is accepted by the rule checker and
   answered `Refused` by the core. `verify_obligation` accepts such a
   certificate; the comparison harness counts it apart from `Judged`.
2. **Knowledge propositions have a table.** `MachineTables::propositions`
   (machine-local, minted by `push`); `KnowledgeRow::proposition` indexes it.
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
5. **The read equation needs no kernel rule.** `read == v` is passed to the
   kernel as a semantic axiom, like every operation equation, and the
   existing equality steps carry it: a bound on `v` bounds the read, and two
   reads of one leaf are related through the value they share. An unsigned
   `p.x - p.x` over a formal discharges with a `Judged` certificate.
6. **Edge writes are refused, not placed.** The backward walk moves between
   blocks. A write logged on one edge reaches only that edge's destination,
   so a write at `ProgramPoint::Edge` that overlaps a read refuses the
   machine until the slice that logs one teaches the walk about edges.

## Evidence seals

`Discharge::operation_seals` turns each discharged `OperationSafety`
obligation into a `terminal_codec::ObligationEvidenceSeal`: the obligation's
machine and identity, its goal, the hypotheses and the certificate, and a
SHA-256 fingerprint over their canonical encoding. Only that kind becomes an
accepted fact in Omega 00. The caller hands the seals to Omega 00 with
`lower_module_with_evidence`, the only way to attach them.

Omega 00 checks each seal before it fills the fact's
`proof_bundle_fingerprint`: the fingerprint matches the seal's parts, the
operation is in the module and owns the obligation, and the goal Terminal
semantics states for that operation equals the sealed goal. A seal that fails
a check, or names no accepted fact, refuses the unit. Omega 00 does not re-run
the kernel: that the kernel accepted the certificate under those hypotheses,
and that the hypotheses held where the obligation sits, it takes on faith.

Seals are in memory only. Terminal Psi's layout has no place for them yet, so
they are lost on a byte round trip and do not survive a stored artifact: a
module lowered from bytes, including one a cache rebuilt, arrives with
`ObligationEvidence::ReadFromBytes` and zero fingerprints.

## Measuring against the current checker

`omega-rust/omega/compilation/compiler/examples/route_one_comparison.rs` runs
both checkers on corpus programs and prints one line per program and one per
obligation. `discharge_with(_, Knowledge::WithoutBranchConditions)` exists only
so that comparison can count what branch facts buy; the checker uses
`discharge`.
