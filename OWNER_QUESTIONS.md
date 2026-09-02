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

## Q2 — Mutable structural-parameter native identity

Omega structural parameters currently select a native ABI from their value
shape without incorporating `MutableBorrow` or `WriteOnlyBorrow`. A small
record can therefore arrive directly in registers and be staged into a local
Unit-frame home. A verified field store updates that local carrier and an
immediately following projected call observes it, but a caller or alias cannot
observe the mutation after return. That bounded closed-use route is not a
general realization of borrowed mutable identity.

Choose the native identity contract for mutable structural parameters:

- mutable and write-only structural borrows are always represented by an
  indirect referent placement, while owned values retain shape-selected
  by-value ABI treatment;
- a checked copy-in/copy-out contract is added with explicit alias, partial
  write, crash, and return-path semantics; or
- Omega defines these structural parameters as invocation-local value
  carriers despite their borrow spelling, which would require reconciling the
  language-level meaning of mutation and aliases.

The first option is the proposed direction. Treating a staged copy as though it
were the referent is tempting but wrong; ordinary post-return or concurrent
alias observation would distinguish them. This decision blocks general native
structural-field stores and writeback. It does not block the explicitly bounded
direct named-dynamic fixture whose stored value is consumed by the following
projected call before the Unit returns.

## Q3 — Ranked receiver-subplace transfer identity

The product compiler requires ranked cyclic control to preserve a mutable
receiver subplace across a backedge. The current ranked source and checked plan
synthesize target `self` from source `self` as a whole receiver and retain only
source/target parameter positions. Neither layer has a receiver projection path
or a rule that identifies a projected referent as the next state's receiver.

Choose the authored and semantic identity of that transfer:

- keep `self` whole and carry `&mut self.field` as a separate explicit state
  parameter, with the receiver and subloan both present in the cyclic frontier;
- allow a transition to rebind the target state's `self` directly to a
  projected source subplace, defining the required nominal-type and lifetime
  relationship; or
- keep the target receiver whole but add explicit external root-to-receiver
  provenance, so the ranked carrier records that `self` denotes a subplace of
  an enclosing owner without transferring a second parameter.

These choices produce different checked frontier, alias, cleanup, ABI, and
native replay obligations. This decision blocks only the first projected
receiver-subplace ranked-countdown slice. Whole-receiver ranked countdowns and
ranked work that does not require receiver projection may proceed unchanged.
