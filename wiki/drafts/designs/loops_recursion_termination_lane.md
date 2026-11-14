# Loops, recursion and termination lane

Staging plan, 2026-11-11, for one lane of the first parallel group of
[check on the lowered form](check_on_lowered_form.md). The owner authorized
starting it ahead of route one's gate, paused it on 2026-11-11 and lifted the
pause on 2026-11-12. Slice 1 is built (`#2367`); nothing else here is.

The new checker refuses every program in which a machine calls itself, directly
or through other machines (`call recursion`). In Omega that is every loop: a
loop is a machine whose transition arm calls it again. This lane lets the
checker hold such programs. A call cycle becomes what the specification says
it is on the lowered form, blocks with a jump back, and three things are then
checked there: what holds each time the loop is entered, that a loop which
promises to end has a measure that goes down, and what a machine may publish
about ending.

The lane adds no proof method. A loop entry condition and a measure
comparison are goals over values, and the one route-one prover discharges
them through `proof/admission`. It adds no table either: the obligation
kinds (`ArrivalInvariant`, `WellFoundedness`, `RankComparison`), the
justification (`BlockInvariant`) and the summary term (`Termination`) it
needs are already in `terminal-check-tables`, unused.

## What the specification fixes

Quotations are verbatim.

| Rule | Sentence |
| --- | --- |
| A runtime call cycle is a loop | [termination](../../spec/language/termination.md#calls-and-productive-loops): "Runtime recursive call cycles are tail-position only and lower to constant-stack iteration." [Control flow](../../spec/terminal-psi/control_flow.md#graph-and-frontier): "Constant-stack terminal cycles may become block backedges". |
| Nesting is not a boundary | control flow: "Source nesting does not bound the component or justify any edge." termination: "one joint ranking for the entire strongly connected call component, whether declarations are nested or top-level and whether an edge is an ordinary call or terminal transfer". |
| A loop need not end | termination: "A productive terminal-transfer cycle without a termination promise may run forever and owes no rank". control flow: "A productive component may remain unranked and execute indefinitely." |
| Each arrival proves the target's `requires` | [state contracts](../../spec/language/state_contracts.md#bindings-and-arrivals): "Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution." |
| On the lowered form that is a block invariant | control flow: "A scalar block invariant is a semantic row identifying a machine, destination block, scalar predicate, and one obligation for every actual incoming edge." and "Every arrival proves the predicate under simultaneous destination-parameter to actual-argument substitution, before destination hypotheses become available." |
| Nothing else is known at a loop head | control flow: "At a cyclic scheduling cut, only checked invariant predicates enter as induction hypotheses; prefix facts are discarded." and "A fact from an acyclic prefix or one iteration is not a loop invariant." |
| The machine entry is not a loop head | control flow: "The machine entry cannot carry a row: invocation supplies the machine's declared preconditions rather than an explicit control-flow edge." |
| A promise needs a witness on a cycle | termination: "A cyclic terminating body must supply its witness; inference never chooses its subjects or silently publishes a guarantee. Unknown progress dependencies prevent a checked guarantee." |
| The witness is one ranking per component | control flow: "One grouped control judgment covers each reconstructed component, with a shared well-foundedness question and comparison obligations for every internal edge." and "Removing strict edges must leave an acyclic graph: every cycle encounters descent, not just the cycles selected by a traversal." |
| A rank is not a safety fact | control flow: "A decreasing rank does not grant an arithmetic safety bound." |
| A recursive contract is not its own proof | termination: "It does not authorize importing a recursive callee's contract as an induction hypothesis on a rank-preserving edge. That import separately requires strict descent at the exact edge". |

## The lowered form

A call component is a strongly connected set of machines in the call graph
elaboration reads: one machine that calls itself, or several that reach each
other. The lane serves a component when every call between its members
stands in terminal position, where the call's result completes the caller.

Each machine keeps its own Terminal machine, as today. For a machine `M` in
a component, that machine's graph holds the whole component:

- **Entry.** The entry block holds `M`'s formals as machine parameters and
  jumps to `M`'s head block with them.
- **Head blocks.** One block per member `N`, whose block parameters are
  `N`'s scalar formals. `N`'s body is lowered once beneath it, its slots
  bound to those parameters.
- **Back edges.** A terminal call of member `N` evaluates its arguments left
  to right and jumps to `N`'s head with them. No call operation and no
  invocation row is made: the lowered form has no call there.
- **Everything else** is lowered as it is today: a call of a machine outside
  the component is a call operation, a return returns from `M`.

So a component of `k` machines is lowered `k` times, once per member that can
be entered from outside. Nothing is shared between those graphs, and
identities never cross them.

**`requires` at a head.** Member `N`'s `requires` clauses, with each formal
replaced by the head's parameter, are that head's one `ScalarBlockInvariant`
row (a conjunction when there are several). Elaboration mints one
`ArrivalInvariant` obligation per edge into the head: the entry's jump for
`M`'s own head, and every back edge. A head whose member has no `requires`
gets no row; its arrivals are still minted, as on every edge that passes
arguments, and their goal is `Truth`. The predicate is authored, never
inferred; this lane infers no loop invariant.

**What is known in a loop.** The checker adds one knowledge row per head row,
the predicate over the head's parameters, available from the head's entry
and justified as `Justification::BlockInvariant`. Knowledge stays what it is
everywhere else on this route: a row over value identities, available where
its point dominates. A head parameter is a fresh value on each arrival, so
nothing known about an argument is known about the parameter, which is the
specification's "prefix facts are discarded" in a model where facts name
values. A row about a value evaluated before the loop stays true inside it:
that value does not change. `M`'s own `requires` hold at the entry over the
machine parameters and reach the head only by being proved on the entry's
jump.

**Termination.** A member that authors `terminates by subject` supplies the
component's ranking. Elaboration writes the machine's `ranked_scc`: each
head's rank value, and one edge row per jump inside the component, `Strict`
or `Preserving`. It mints one `WellFoundedness` obligation for the component
and one `RankComparison` per such edge, whose goal is `successor_rank <
source_rank` or `<=` under the edge's argument substitution. The structural
half, that removing the strict edges leaves no cycle, is a graph judgment
with no certificate, like initialization in the places lane.

**What a machine publishes.** `Summary::termination` is `Terminates` for a
machine whose graph has no cycle and whose callees all terminate, and for a
member of a ranked component whose obligations are discharged; otherwise
`NoGuarantee`. A machine that authors `terminates` and derives `NoGuarantee`
is a program error.

## Slices

Each slice lands on its own, and each refuses
the constructs of the later ones as `Unsupported` with a detail naming them.

| Slice | Serves | Refuses until later |
| --- | --- | --- |
| 1. Productive cycles (built, `#2367`) | Self and mutual recursion in terminal position between scalar machines; head blocks, back edges, `requires` as the head's invariant, `ArrivalInvariant` proved on every arrival; outcomes (normal completion, crash routes) of a component as one fixed point | a call of a member outside terminal position; a record formal on a cycle; `ensures` on a member; a termination promise on or over a cycle; an authored guarded crash route on a member; an `[erased]` binding or a value of a type parameter in a member |
| 2. Witness by identity | A ranking witness reaches the typed trees twice: as text (`RankingWitness::subjects: Vec<String>`) and as typed expressions (`RankingExpressionCustody`, with the view as a `RankingViewId`). The reader takes subjects, view arguments and range from the custody only, and refuses a witness that has none as a spelling; it never resolves the text. If some witness form has no custody, the repair goes to the stage that dropped it (standing rule: identity is fixed upstream) | |
| 3. Natural descent | `terminates by x` and `-> Nat::Descending` on a scalar formal; `ranked_scc`, `WellFoundedness`, `RankComparison`, the every-cycle-descends judgment; the published `Termination` term and a promise checked against it | other views, rank ranges, measures, lexicographic ranks |
| 4. Guarantees across a cycle | `ensures` on members: each return in the fused graph proves the entered machine's clause, using a member's clause only where the specification lets a recursive contract be cited | |
| 5. Other views | `Nat::IncreasingTo`, `Nat::BoundedDistance` (control flow: "one exact subtraction over parameters and integer constants"), `in lower..=upper`, declared `measure`s, lexicographic products | `Slice::Length` (collections lane) |
| 6. Storage across a back edge | Record formals on a cycle as structural block parameters; the write log across a back edge | references on a cycle (references lane) |

Slice 4's rule needs care and may end as an owner question: the
specification ties citing a recursive contract to "strict descent at the
exact edge", which an unranked productive loop does not have.

Out of the lane altogether: non-tail recursion, which is "valid in
proof/compile-time use and rejects runtime lowering" and belongs to the
proofs lane; proof-only recursion components; progress profiles and
provider premises; cross-package recursion, which the specification leaves
unsupported.

## One route, no second carrier

AGENTS.md stop rule 6 applies to every row above.

- A loop entry condition is `ArrivalInvariant`, on every arrival alike. A
  back edge does not get a `CallRequirement`, because the lowered form has
  no call there, and the first arrival does not get a special kind.
- `requires` of a member is stated once, as the head's row. The checker
  reads the predicate from the module's `scalar_block_invariants`, never
  from the member's contract a second time.
- Every certificate goes through `prove.rs` and `verify_obligation`. If a
  rank comparison needs a rule the kernel lacks, that is reported as a
  finding, not worked around.
- Acyclic joins (`terminal-psi-to-terminal-psi`, `joins`) were on `main` first: lowering
  mints one `ArrivalInvariant` on every edge that passes arguments, and the
  checker infers a join's predicate as a `BlockInvariant` knowledge row.
  Slice 1 uses that one mechanism. A loop head's declared row becomes the
  head's `BlockInvariant` knowledge after `arrivals::admit` checks it, and
  `joins::arrival_goal` states every arrival's goal, at a head and at a
  join alike. The one difference is the program's claim: a join whose
  arrival fails has its inferred predicate weakened to `Truth`, and a head's
  declared predicate is never weakened, so its failed arrival rejects.

## Measures and acceptance

Per slice:

1. **Rejection diagnostics.** On every must-reject program of the slice, the
   route's message names the failing obligation and stands at the source
   construct. A failed head arrival is worded as the call requirement it is.
   Slice 1 reports such a failure once per member whose graph holds the
   cycle.
2. **Kernel-checked evidence** per obligation kind, with its denominator.
3. **Decisions keyed on text: 0.** Slice 2 exists for this: the witness's
   rendered subjects are never read.
4. **Silent drops: 0.** Every edge into a head has an obligation; a test
   asserts it over the edge table.
5. **Routes per obligation kind: 1.**
6. **Lines added**, production and test, per slice.

Must-reject programs are named per slice. A slice adds at most one focused
positive/negative pair, kept beside the test that reads it under
`tests/fixtures/`.
