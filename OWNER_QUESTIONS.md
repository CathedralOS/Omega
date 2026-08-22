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

Last pruned: 2026-08-22.

## Q1 — Progress-profile classification and premise attachment

Termination guarantees can retain sealed `ProgressProfileId` premises, but the
ordinary domain and routed-requirement surface does not distinguish a progress
profile from another predicate-free qualification. Choose the explicit
classification or exact closed inference rule, and choose whether the premise
attaches to the machine guarantee or to a selected operation/provider edge.

The decision must bind establishment to the profile owner or explicit acceptance
authority and to an admitted grant/receipt, while keeping private ranking
witnesses outside public contract identity. Generic routed/domain requirements
must not become progress premises merely because they are predicate-free,
provider-backed, or mentioned by a terminating machine.
