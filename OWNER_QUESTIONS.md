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

Last pruned: 2026-08-25.

## Q1 — How are named conformances published across packages?

Named conformances are top-level, exact declarations selected by public generic
bounds, dynamic-trait types, proof evidence, and specialization. Ordinary
`pub` currently rejects a conformance item, typed conformances retain no
visibility, yet package admission permits a direct dependency's named
conformance to be selected. The result is an accidental all-public lane that
contradicts the settled rule that independently nameable declarations own
visibility rather than inheriting it from a carrier.

Proposed solution: permit `pub` on a named conformance declaration, retain that
bit through checked trees, and make unmarked conformances package-private.
Cross-package selection and public-interface citation require `pub`; private
same-package implementation selection remains legal. Add a blocking
`PublicConformance` review row for the exact package-qualified declaration,
trait application, static telescope, requirement map, and checked evidence
interface. Realization bodies and proof machinery remain source-committed
implementation, not serialized certificates.

An acceptable alternate is to make named conformances explicitly public by
definition and remove the pretense that they participate in ordinary `pub`, but
that should be a deliberate language rule with no private named-conformance
use case. Tempting but wrong alternatives are inheriting visibility from the
carrier or trait, treating direct dependency admission as publication, using
the alias string as identity, or publishing realization bodies/proof text as
review evidence.
