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

<a id="native-cache-producer-authorization"></a>

### Q1 - Does enabling a native component cache authorize its producer?

Decision name: `native-cache-producer-authorization`.

**Customer.** Separate package builds reuse native components from the optional
disk cache. `ComponentProductCache::load` checks the compiler identity, input
key, record size and content digest before recovering a component. Those checks
identify bytes; they do not authorize the process that produced them.

**Gap.** [Publication and separate consumption](wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
requires receiving-host authorization of the exact artifact and producer, and
permits trusted internal outputs without replay. That page and
[claim admission](wiki/drafts/designs/psi_claim_admission.md) do not say whether
configuring `CompilerProductRetention` is itself that host authorization.
The implementation has a directory and identities, but no separate decision.

**Question.** (a) Require explicit host authorization for recovered components,
treating unauthorized optional cache entries as misses and rebuilding source;
or (b) define enabling a cache as an explicit host decision to trust its writers
and recovered contents? Directory protection remains the host's responsibility,
not a new Omega guarantee. Recommend (a), keeping byte identity separate from
authorization without adding an OS credential or ACL system.

**Until answered.** Cache recovery retains its existing behavior; hashes must
not be described as producer authorization. The public native ingress repair
remains under `PSI-CLAIM-ADMISSION`.

### Q2 - Is a borrow of a fixed array accepted where a view of its elements is expected?

Decision name: `array-borrow-as-view`.

**Customer.** Reading a file into a buffer the machine owns. The toolchain's
own Build prelude declares
`BuildSource::read(source: &BuildSource, descriptor: i32, buffer: &mut [u8], count: u64)`,
and every Build that calls it writes
`let mut bytes: [u8; 38]; BuildSource::read(&builder.source, reader, &mut bytes, 38);`.
`Console::read_line(&mut buffer)` in the standard library is called the same
way. About 165 sample and fixture files pass `&bytes` or `&mut bytes` of a
`[u8; N]` for a `&[u8]` or `&mut [u8]` formal.

**Gap.** No page says a borrow of `[T; N]` is a value of `&[T]`.
[Expressions](wiki/spec/language/expressions.md#collection-arguments-and-results)
gives the adaptation for `[]` and `[..]` only and adds that it introduces no
"automatic borrowing for other operator operands";
[counts and addresses](wiki/spec/language/counts_and_addresses.md#bounds-and-lowering)
fixes an array's length at `N` and says nothing of views of it. The language
guide converts explicitly (chapter 5: `ArrayAccess::as_slice(&fixed)`), and
core declares that boundary (`source/library/core/array.omg`). The checker
follows the pages and refuses the call (`typing argument types`). Checked on
`origin/main` at `9f23002f48`: no ratification covers it.

**Question.** At an argument, a `let` or a field store whose expected type is
`&[T]` or `&mut [T]`, is `&array` or `&mut array` of a `[T; N]` (a) accepted
as a view of all `N` elements, with the view's length known to be `N`; (b)
accepted only when written as a range, `&array[..]` and `&mut array[..]`; or
(c) not accepted, so `ArrayAccess::as_slice` and `as_mut_slice` are the only
way, and the prelude's callers change?

**Until answered.** The checker refuses the call by name. Nine programs stop
in their Build for it; the rest of the 165 files meet it later.
