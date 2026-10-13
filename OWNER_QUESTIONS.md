# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

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

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q1 - What does a non-discovery Build call do while discovery runs?

Named decision: `build-discovery-entry-phase`.

**Customer:** an ordinary application using a third-party build dependency for
code generation or topology composition. Its dependency requests must be known
before the dependency's helpers can be checked or executed. Anonymous roots
also need this actual resolved graph before their declaration owner is issued.
This is an execution-phase rule, not a request to preserve the static parser.

**Existing rules:** [Build declarations](wiki/spec/build/declarations.md#project-role)
lines 15–29 say the compiler "constructs a core-only discovery program from
`build.omg`", checks/interprets it, then checks/interprets the complete Build
with dependencies, and require exactly one free
`machine build(builder: &mut Build)` entry. Discovery may use ordinary local
helpers and control flow, but no dependency code. Lines 43–56 require ordinary
default source through those same two routes and forbid discovering dependency
edges from imports.
[Scoped execution](wiki/spec/build/scoped_execution.md#dependency-declarations-and-discovery)
lines 88–114 put dependency declarations and `configure_products(builder)` in
that one entry, allow a top-of-file build-dependency import, and prohibit calling
imported helpers during discovery.
[Execution admission](wiki/spec/build/execution.md#admission-before-execution)
separates discovery from complete Build checking/admission but supplies no
source-level phase boundary or entry-selection operation.
[Configuration](wiki/spec/build/configuration.md#exact-target-requests)
excludes target-dependent discovery, which does not select the discovery entry.

The actual seed
[build.omg](tests/omega/pass/float/named_provider_directed_divide_exit/build.omg)
lines 3–9 interleaves `Build::application`, `Build::depend(Source::Path)` and
four `Build::bind(..., Main::main)` calls in the same free entry. The latter
select product declarations whose std dependency is not available until after
discovery. Executing the whole entry core-only does not establish how later
product/build-dependency work is excluded. Recognizing a prefix by call names,
ignoring later calls or silently pruning source would invent that rule.

The scoped-execution direct-projection wording at lines 108–114 and its
helper/conditional refusal row at line 620 also conflict with the interpreted
local-helper rule. Those migration remnants need reconciliation, but the
remaining ambiguity is which ordinary program runs in each phase, even after
the parser is deleted. No existing owner question resolves it.

**Settled by the contracts above:** there is one Build entry, and the compiler
derives the discovery program from it. [Build declarations](wiki/spec/build/declarations.md#project-role)
require "exactly one free `machine build(builder: &mut Build)` entry", and its
2026-10-07 text has the compiler construct "a core-only discovery program from
`build.omg`" that it checks and interprets; `package`, `application`,
`member`, `depend`, `depend_as`, `build_depend` and `build_depend_as` are the
discovery environment's boundary operations. A separately authored discovery
entry is therefore not an option. The 2026-09-10 direct-projection wording in
[scoped execution](wiki/spec/build/scoped_execution.md#dependency-declarations-and-discovery)
("no extra discovery-time evaluator") predates that and is the migration
remnant to reconcile.

**Decision needed:** what a Build operation outside that discovery set does
when the discovery program reaches it — `Build::bind(builder, ...,
Main::main)` naming a product machine whose dependencies are not resolved yet,
`Build::select_provider`, or a local helper calling build-dependency code.

1. **Recommended: they are effect-free in discovery, and the product names
   they mention stay unresolved until the complete Build.** The discovery
   environment declares every other Build operation as a boundary operation
   with no discovery effect, so the whole entry runs in both phases and the
   complete Build performs each operation once. A product path such as
   `Main::main` is carried as a name during discovery and resolved and
   checked when the complete Build is checked with its dependencies.
2. **Discovery is a defined prefix of the entry.** The discovery program is
   the entry's statements up to its last discovery operation, checked
   core-only; later statements run only in the complete Build. A build then
   orders its role and dependency calls first, as the seed already does, but
   the phase boundary depends on statement position.

Until answered, only the discovery/default-source phase subfrontier of
**TERMINAL-BUILD-SEED** is owner-blocked. Terminal production, mutable-slot
contracts, accounting, immutable resolution and retained identity/evidence joins
remain implementation work.
