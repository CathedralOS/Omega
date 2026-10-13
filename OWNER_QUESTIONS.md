# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

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

Proposed solutions below are recommendations, not owner rulings. The owner
removes an entry once it is answered; unfinished implementation stays on its
owning task.

### Q1 - How does a Build select its core-only discovery entry?

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

**Decision needed:** how is the core-only discovery entry and its boundary with
complete Build authored and selected?

1. **Recommended: explicit separate ordinary entry.** Define how source selects
   a checked core-only discovery machine; it emits role and scoped dependency
   requests. The existing Build entry runs once after graph admission. Define
   the same pair for compiler-supplied default source. Both use the ordinary
   Psi/Terminal path; no call-name projection or implicit skipped statements.
2. **One Build entry with an explicit phase contract.** Define the source-visible
   boundary, checked visibility and effect schedule that derive discovery and
   complete Build from that entry, including dependency-helper calls and whether
   any continuation resumes. No implementation-inferred prefix or no-op call
   semantics. Preserve complete Build's once-only effects and reject dependency
   code/product-body execution before graph admission.

Until answered, only the discovery/default-source phase subfrontier of
**TERMINAL-BUILD-SEED** is owner-blocked. Terminal production, mutable-slot
contracts, accounting, immutable resolution and retained identity/evidence joins
remain implementation work.

Note for the owner (2026-10-13, not a ruling): `178ecc0b10` ("docs: define
resumable build dependency execution") later replaced the discovery-program
wording this question cites with one interpreted Build activation that
suspends on `Build::binding<R>`. Whether that answers this question is the
owner's call.
