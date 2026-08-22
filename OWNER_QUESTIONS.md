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

Last pruned: 2026-08-21.

## Q1 — Native logical-fuel meter ABI and continuation

Terminal Psi settles sponsor-owned logical fuel: trusted native lowering charges
before each semantic operation or taken edge, exhaustion is not program-visible,
and replenishment resumes at the unpaid site without replaying completed work.
Native artifacts and installation records now retain the exact schedule, site,
units, and byte interval, but no approved target/runtime contract says where the
mutable budget lives or how exhaustion transfers control and later resumes.

Choose the native meter ABI and continuation contract. The decision must settle
whether the budget is passed explicitly, held in a reserved register, or reached
through sponsor-owned execution context; which layer owns and preserves that
state across ordinary and provider calls; how the slow path reports the exact
unpaid `OperationId` or `EdgeId`; and what non-forgeable continuation authorizes
resume without exposing fuel to the program. It must apply coherently across
x86-64 and AArch64 calling policies, preserve stack/alignment and machine-state
contracts, and keep fixed-fuel meter elision a separately admitted installation
decision.

## Q2 — Progress-profile classification and premise attachment

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
