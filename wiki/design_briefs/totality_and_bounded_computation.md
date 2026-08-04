# Design Brief: Totality, Productivity, And Bounded Computation

Current design as of 2026-07-18. Frozen decision 23 settles the termination
surface, its public/private identity split, and opaque boundary progress
profiles. The compiler and corpus still implement the superseded spelling.
General trace logic, WCET, and non-returning control outcomes remain open.

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
starvation freedom. Decision 23 represents v1 provider progress evidence as
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

This algorithm-visible budget is distinct from terminal-Psi sponsor fuel.
Sponsor fuel meters already-admitted execution, is not observable or catchable
by the program, and may pause, cancel, or terminate the execution externally.
See
[`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md).

## Failure and non-return

Logic failures such as unchecked overflow, invalid indexing, and impossible
case extraction are proof obligations and reject by default. Recoverable
failure remains a return sum with case-specific guarantees.

Traps, cancellation, and deliberate non-return occupy the failure/control axis
of the complete machine contract; they are not reach-row members. Reaching a
process-exit boundary also contributes the `ProcessExit` service identity to
the reach row. These facts are independent: graceful exit and nuclear abort
may reach the same service while having different cleanup and control
contracts.

## WCET and quantitative resources

Termination is not a worst-case execution-time theorem. WCET additionally
needs target timing assumptions and compositional bounds for loops, calls,
memory behavior, and suspension. Quantitative memory and retention bounds
likewise belong to the resource algebra rather than a single `budget` clause
or qualitative reach-row member.

Hard external roots expose the intermediate structural tier explicitly.
Admission may require a fixed-work certificate denominated in terminal-Psi
fuel and compare it with the sponsor provision, while `terminates by` rankings,
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

## Still open

- general trace propositions, deadline/starvation contracts, and entailment
  between opaque progress profiles;
- the exact complete-contract spelling for deliberate non-return
  (`OWNER_QUESTIONS.md` Q2);
- target WCET proof scope and timing-model composition for profiles that require
  physical deadlines; and
- richer productivity theorems for reactive systems.

## Cross-references

See chapters 3, 9, 16, 18, and 19; `mathematical_proofs.md` for proof-stratum
recursion; `effects_authority_and_observation.md` for progress/effect
separation; `proof_caching.md` for transmissible proof artifacts; and
[Termination, Ranking, And Progress](termination_ranking_and_progress.md) for
the frozen source, identity, and progress-profile ruling.
