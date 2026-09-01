# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Last pruned: 2026-08-31.

## Q1 — Compiler-inserted spill-access fault semantics

Omega must compile ordinary register-pressure programs by relocating live
values through compiler-owned spill storage. Chapter 16 currently requires
operation- or platform-triggered faults to remain inside explicit
`crashes Trap` ceilings, while the optimizer semantic contract forbids
introducing an observable trap or exit change. Neither contract says whether
a fault caused only by realizing an otherwise-valid value in a
compiler-selected stack slot is an Omega program observation.

Choose the semantic boundary for compiler-inserted spill loads/stores:

- the target/runtime must establish sufficient spill storage before entering
  the checked invocation, making admitted spill accesses non-faulting in the
  language model and treating establishment failure as an outer activation or
  deployment failure;
- each possibly faulting spill access is a platform-triggered `Trap` site that
  must enter inferred/published crash ceilings with retained guard and
  provenance; or
- a versioned target realization profile explicitly selects between those
  contracts and becomes part of optimization, frame, and publication custody.

This decision blocks only conversion of the validated abstract spill schedule
into real memory operations and frame/probing code. Logical spill choice,
abstract slot coloring, reload-value allocation, and non-authoritative frame
planning may proceed without making a trap claim.
