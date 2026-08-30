# Design Brief: Totality, Productivity, And Bounded Computation

Current design as of 2026-08-06. Frozen decision 23 settles the termination
surface, its public/private identity split, and opaque boundary progress
profiles. The compiler and corpus still implement the superseded spelling.
Guarded crash ceilings settle non-returning control independently. General
trace logic and WCET remain open.

## Governing law

> **No unbounded computation is invisible.** Terminating cycles carry a
> checked well-founded ranking. Deliberately productive state-graph loops are
> visibly productive. Computation whose mathematical termination is unknown
> takes an explicit budget and returns exhaustion as data.

Omega does not treat all non-acyclic computation as one effect. It distinguishes
three cases mechanically.

## Ranked cycles and runtime recursion

A terminating cycle is legal only when every cyclic edge strictly decreases a
well-founded rank selected with `terminates by ...`. The same rule covers call
recursion and explicit state/transition loops. A productive loop that makes no
termination promise may run forever and owes no ranking.

Runtime recursion has an additional lowering rule: every recursive call in the
cycle must be in tail position. The compiler then lowers the cycle to the same
constant-stack back-edge machinery used by state transitions. Measured
non-tail recursion is legal only in the proof/compile-time stratum, where no
runtime activation frames are emitted.

The ranking proves termination; tail position determines runtime lowering.
Neither substitutes for the other. After lowering, the runtime call graph is
acyclic and the maximum live activation storage is statically reportable.

## Productive transition loops

A transition back-edge is a jump inside one machine, not recursive calling. It
may run forever when no completion guarantee is authored. This is the source model for
event loops, services, schedulers, and other productive machines.

Productivity does not by itself prove fairness, eventual wakeup, deadlines, or
starvation freedom. Decision 23 represents provider progress evidence as
sealed opaque progress profiles admitted through boundary grants. They remain
separate from termination and from the negative guarantees obtained by
omitting `suspends` or `blocks`.

## Budgeted unknowns

When termination cannot be proved, the honest API accepts fuel or another
explicit resource bound and returns exhaustion as an ordinary case:

```omega
machine collatz(n: u64, budget: u32) -> Converged(steps: u32) | OutOfBudget {
    // bounded work only
}
```

An interpreter similarly returns `OutOfFuel`; a bounded search returns
`Exhausted`. The bound makes that invocation total without pretending the
unbounded mathematical process is known to terminate.

This algorithm-visible budget is distinct from Terminal-Psi logical-work
accounting used by compiler services and static analysis. Neither creates a
native runtime meter.
See
[`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md).

## Failure and non-return

Logic failures such as unchecked overflow, invalid indexing, and impossible
case extraction are proof obligations and reject by default. Recoverable
failure remains a return sum with case-specific guarantees.

`crashes Cause` publishes guarded non-return routes on an independent may-axis
of the complete machine contract. `Trap` and `Abort` are distinct causes; both
terminate without cleanup. Call-site facts may disprove individual routes and
remove the crash edge for that invocation. Cooperative cancellation remains a
returned task outcome rather than a crash.

Reaching a process-exit boundary also contributes the `ProcessExit` service
identity to the reach row. These facts are independent: graceful exit and abort
may reach the same service while having different cleanup and control
contracts. `terminates` remains a positive eventual-progress guarantee and
cannot be used to name a crash cause; `terminates by ...` remains ranking
evidence.

## WCET and quantitative resources

Termination is not a worst-case execution-time theorem. WCET additionally
needs target timing assumptions and compositional bounds for loops, calls,
memory behavior, and suspension. Quantitative memory and retention bounds
likewise belong to the resource algebra rather than a single `budget` clause
or qualitative reach-row member.

Hard external roots expose the intermediate structural tier explicitly.
Admission may require a fixed-work certificate denominated in Terminal-Psi
logical units and compare it with an authored profile ceiling, while `terminates by` rankings,
acyclic control flow, callee summaries, and proof internals remain private
evidence. The same restricted checker can analyze a segment ending at the next
semantic safe point. A compile-time ranking range can bound cyclic edges; it
does not by itself bound transitive work or latency. Cathedral's first timer
uses the trivial profile: acyclic final control flow and fixed-work admitted
leaves. This proves finite logical work under provider contracts, not a
portable deadline or WCET.

## Acceptance register

1. An unranked cycle in a terminating machine rejects.
2. A ranked runtime recursive cycle rejects if any recursive call is not
   tail-position.
3. A ranked tail cycle lowers without stack growth.
4. A proof-only ranked non-tail recursion is legal and emits no runtime
   frame.
5. A transition loop may be deliberately non-terminating and constant-stack.
6. A budgeted computation exposes exhaustion as a return case.
7. `terminates` does not imply fairness, no suspension, or a deadline.
8. Effect-row absence cannot be used to launder an ambient progress or
   resource requirement.
9. Every derived crash site is covered by a published route of the same cause.
10. A crash site's live-claim frontier is an audit lower bound, not proof that
    unlisted state remains valid or that another activation may survive.

## Still open

- general trace propositions, deadline/starvation contracts, and entailment
  between opaque progress profiles;
- target WCET proof scope and timing-model composition for profiles that require
  physical deadlines; and
- richer productivity theorems for reactive systems.

## Cross-references

See chapters 3, 9, 16, 18, and 19; `mathematical_proofs.md` for proof-stratum
recursion; `effects_authority_and_observation.md` for progress/effect
separation; `proof_caching.md` for transmissible proof artifacts; and
[Termination, Ranking, And Progress](termination_ranking_and_progress.md) for
the frozen source, identity, and progress-profile ruling.
