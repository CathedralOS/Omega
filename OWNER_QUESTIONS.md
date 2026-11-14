# Owner Questions

Only the owner answers an entry. The owner's word in a session is the
ruling: write it into the specification, remove the entry, and build it; that
is what the owner asked for. Otherwise agents add questions and may research
existing ones, but do not settle a question themselves.

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

Bootstrap construction follows the owner's [variant mandate](bootstrap/MINIMIZATION.md).
Architecture exploration and implementation within that mandate are delegated.
Unresolved changes outside it belong in [OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md).
This file continues to own product-language questions.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

### Q1 - May a qualified operator be declared outside the domain's module?

Decision name: `unresolved-qualified-operator-home`.

**Customer.** No authored source and no product requirement needs an operator
declared outside its domain's module. The only user is the #2965 homing test.

**Gap.** [Operator families](wiki/spec/language/expressions.md#operator-families)
(`expressions.md:299-328`), [declaration and membership](wiki/spec/language/domains.md#declaration-and-membership)
(`domains.md:23-29`), [foreign attached paths](wiki/spec/language/modules.md#foreign-attached-declaration-paths)
(`modules.md:77-94`), and [operators on a domain](wiki/language_guide/chapter_8_domains.md#operators-on-a-domain)
(`chapter_8_domains.md:111-123`) were checked. A qualified name establishes a
home, operations are separate machines, a short path selects an exposed domain,
and the guide places the operator beside the domain. Those clauses are silent
on a root file declaring `Carrier::Leaf::op`.

```omega
// Form A, inside the domain's module.
module units;
public domain u64::Distance requires self > 0;
public operator + u64::Distance::add(
    left: u64 in u64::Distance, right: u64 in u64::Distance
) -> u64 in u64::Distance;

// Form B, root file.
use units;
public operator + u64::Distance::add(
    left: u64 in u64::Distance, right: u64 in u64::Distance
) -> u64 in u64::Distance;
```

Before #2965, Form B stays a root item, so `a + b` fails to resolve.
Now Form B is homed on `u64::Distance`, so `a + b` resolves.

**Question.** (a) Allow the root form (current behavior)? (b) Or reject a
qualified operator declared outside the domain's module?

**Until answered.** The compiler homes Form B on `u64::Distance`.

### Q2 - How does a caller in another verification unit know a machine never returns?

Decision name: `published-no-return`.

**Customer.** A library helper whose every path exits or crashes (an `abort`
or `fail` machine in `std`) called from an application. Under
[statements after a call that never returns](wiki/spec/language/state_contracts.md#statements-after-a-call-that-never-returns)
the caller's continuation is unreachable only when checked evidence excludes
normal return; without that evidence the caller must still prove everything
after the call, including a result it can never produce.

**Gap.** [Local call summaries](wiki/spec/language/state_contracts.md#local-call-summaries)
says that across a verification-unit boundary a call uses the published
contract, that extra result and state guarantees are authored rather than
exported from the body, and lists what a published contract retains from
inference: reach, write frames, ownership, borrow-origin, validity and callback
dependencies. Absence of normal return is not in that list, and
`code-after-no-return-call` introduces no `never` type or keyword. The pages
give no way to author it, and do not say it is retained. The canonical
`ProcessExit::exit_process` requirement is covered by
[process exit](wiki/spec/language/process_exit.md); an ordinary machine wrapping
it is not. Pages checked: those two, plus "Verification-unit boundary" and
"Unreachable branches" in `state_contracts.md`.

**Question.** Across a unit boundary, is a machine's lack of any normal return
(a) authored with an existing construct, `ensures false`, which under partial
correctness already means no normal return can occur; (b) retained
automatically in the published contract like inferred reach; or (c) not
visible, so the caller treats non-return as unknown?

**Until answered.** The checker uses every machine's body-derived outcome
regardless of unit (behaviour that predates the rule, kept by #2983). That is
(b) in effect and is wider than the unit rule as written.

### Q3 - What lifetime does a returned view of a quoted literal have when the machine takes no reference?

Decision name: `image-storage-result-lifetime`.

**Customer.** A helper that names a fixed byte string for several callers:
`machine package_name() -> &[u8] { "my_library" }`, called as
`Build::package(builder, package_name())`. The Build scripts of
`samples/cli/facts/recorded_build_role/` and
`samples/cli/facts/required_output_path/` are written this way, and any
library that publishes a name, a path or a message table would be.

**Gap.** [Quoted bytes](wiki/spec/language/data_and_literals.md#quoted-bytes)
says a literal's "runtime shared view refers to immutable image storage, not a
invocation-local owner", so the view outlives the call that returns it.
[Returned views](wiki/spec/language/lifetimes.md#returned-views) relates a
result to an input: "For an elided output, one unambiguous reference input
supplies the relation." A machine with no reference input has no input to
supply it, and the pages name no lifetime for image storage that a result
could be written at (no `'static` or `'image` binder exists). Pages checked:
those two, plus "Binders and source applications" and "Carried-loan
transport" in `lifetimes.md`.

**Question.** For a result view whose machine takes no reference, is the
elided lifetime (a) image storage, so the result may only be a view of a
literal (or of another such result) and is valid for any use; (b) an error,
with a named lifetime for image storage added for the author to write; or
(c) an error with no such lifetime, so a fixed byte string is passed down
from a caller that holds the literal instead of returned?

**Until answered.** The lowered-form checker refuses the machine by name
(`typing result lifetime that no argument is a view of`), with no claim that
the program is wrong.
