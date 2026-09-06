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

Last pruned: 2026-09-05.

## Q1 — Ranked receiver-subplace transfer identity

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
