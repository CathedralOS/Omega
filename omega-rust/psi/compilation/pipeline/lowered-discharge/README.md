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

Beside the obligations it judges initialization: every operation and
terminator that reads storage gets one verdict (`MachineDischarge::uses`),
and a use that some path reaches with nothing stored is refused with reason
`Unestablished`. That is a compile error of the program, like `Unproved`,
not a construct outside the subset; see [Initialization](#initialization).

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

## What a place holds

A read, a whole copy and the initialization judgment all ask one question:
what does this scalar leaf hold on every path to this point?
[places.rs](src/places.rs) answers it with one walk (`Storage::reaching`)
over the tables and the lowered form:

- **The place.** A place is its root and the fields from the root down to
  it. A read names its fields by `StructuralFieldId`; a Terminal Psi storage
  path is resolved by the lowered form itself, one segment against the type
  the previous segment reached (`runtime_structural_field`).
- **Overlap.** One function answers whether two places may share storage: a
  place overlaps itself, its ancestors and its descendants; two places that
  part by two different record fields are disjoint; any other pair of sibling
  segments overlaps, and places under different roots do not. Later lanes
  extend this function (indices, referents) and add no second relation.
- **Reaching.** Walking the graph back from the use, the last write to an
  overlapping place on each path is the one that reaches along it.
  - `Content::Value(v)` on exactly the leaf supplies `v`.
  - `Content::Overwritten` on the leaf or above it supplies contents with no
    value identity, unless the operation at that row is one that moves
    storage whole: a whole copy, a record operand of a literal, a record
    stored into a field. Then the leaf holds what the matching leaf of the
    operation's source held just before it, and the walk continues there.
    The write log cannot say this (its `Content` has no "copied from"
    variant), so the source is read from the operation.
  - A path that reaches the machine's entry with no write is supplied by the
    place's entry content when its root is a formal, and by nothing
    otherwise.

A store inside a block reaches only what that block leads to: a store in a
guarded arm that completes the machine reaches neither a read nor a copy in
the continuation, and a store before the branch reaches both sides.

### The read equation

A read produces a fresh value. It gets the row `read == v` when `v` is the
one value its place holds on every path to it
([fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md):
"A primitive read may introduce an equality between its copied SSA result
and an exact reaching stored SSA value. Every actual predecessor must supply
that same value"). Different values on different arrivals, or contents with
no value identity, give none: the read is still well formed and nothing is
known about it. A store never removes a row, so what was known about the
value a leaf held before the store stays known.

### Initialization

[Structural access](../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
"An unestablished local cannot be read: a whole read of the home, a loan of
it to an ordinary or boundary call, and its forwarding on a state edge each
require a whole store into it on every path reaching that use."

Every operation and terminator that reads storage is one use: a field read,
a read of a scalar home, a whole copy, a record operand, a record stored
into a field, a call's record arguments, a record return. Each scalar leaf
at or beneath the place it reads must be supplied on every path: by a store
into the leaf, by a whole write to a place above it, or by a formal's entry
content. If some path supplies nothing, the use's verdict is a refusal with
reason `Unestablished`, concerning that operation or edge. A record is
therefore established by a whole write or by a store into every leaf, and
not by fewer.

It is a structural judgment over the write log and the graph. It is not a
proposition, so it has no obligation row, no certificate and no part in the
prover.

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
   so a write at `ProgramPoint::Edge` that overlaps a use refuses the
   machine until the slice that logs one teaches the walk about edges.
7. **A copy's source is not in the write log.** `Content` cannot name a
   source place, so the walk reads it from the operation at the row
   (`StructuralLeafCopy`, a structural operand of `EstablishRecord`, the
   value of `StoreStructuralField`). An `Overwritten` row at any other
   operation keeps its own meaning: contents with no value identity.
8. **A local with no initializer is unestablished here and zero in the
   current checker.** `let mut x: u32; f(x)` is accepted today with `x == 0`
   known; this route rejects the read. The question is in
   `OWNER_QUESTIONS.md` (`local-without-initializer`). If the answer is
   zero, elaboration logs the zero at the reserving operation and nothing
   here changes.

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
both checkers on corpus programs and prints one line per program, one per
obligation and one per unestablished use of storage; a program with such a
use is `route=rejected`. `discharge_with(_, Knowledge::WithoutBranchConditions)` exists only
so that comparison can count what branch facts buy; the checker uses
`discharge`.
