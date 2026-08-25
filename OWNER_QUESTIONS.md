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

## Q1 — How does composition select and partition component artifacts?

The semantic unit is settled: a component is one selected provider realization
plus the closed code, state, resource, registration, continuation, and version
graph that realization owns. A package is only a source, naming, visibility,
and dependency-reach unit; `pub` does not export a component ABI, and a package
is not automatically one component. Hot-swap points may nest within a package,
while a part needing a different source dependency/reach envelope is a distinct
package. Package-shaped component closure is therefore only a valid first
implementation fence.

What remains undecided is the composition authority and exact partition
surface. Choose one model that specifies:

- how a consuming build or final composer selects an exact provider
  realization for independent emission rather than static fusion;
- how one package may contribute zero, one, or several component roots;
- which satisfied requirement identities become component exports and which
  requirements remain external imports;
- how the compiler closes concrete implementation calls, private helpers,
  constants, mutable state, registrations, continuations, authority, and
  resources beneath each selected root;
- what happens when two proposed closures overlap, distinguishing shareable
  immutable dependencies from state or custody that cannot belong to two
  replaceable eras;
- what stable symbolic identity names the component slot independently of one
  candidate artifact or era; and
- which replacement, coexistence, resource, and admission policies are chosen
  by composition rather than asserted by the provider package.

Recommended direction: keep ordinary source limited to declaring requirements
and realizations. The consuming build/composer names an exact requirement slot
and selected realization application and chooses `fused` or `independent`;
the compiler derives and validates the closure. An independent component
exports only requirement identities selected by composition. Concrete-identity
edges either remain inside the derived closure or reject; they never become an
implicit component ABI. A provider package may publish candidates and their
contracts, but cannot force every deployment to componentize itself or bless
its own admission and replacement policy.

The first implementation may require each independent closure to coincide
with one package and reject overlap. That restriction must be reported as an
implementation fence, preserve the general identities above, and later relax
to multiple roots within one package without changing already accepted source.

Tempting but wrong alternatives are a `component` source block that bakes one
deployment policy into reusable library code, treating every package or every
`pub` declaration as a component/export, allowing a concrete machine identity
to cross a replaceable boundary, discovering roots from reachability without
an authored composition selection, naming slots with strings or ordinals, or
letting two eras silently claim the same mutable state or linear custody.

## Q2 — How are named conformances published across packages?

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
