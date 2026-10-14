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

### Q1 - Does a provider body call its raw services without a binding argument?

Named decision: `provider-body-boundary-authority`.

**Customer:** every provider and native-callback body that reaches a raw
service. `samples/cli/audio/wav_output` (and the audio samples built on it),
the portable filesystem providers that
[provider-closure transport](wiki/spec/build/platform_services.md#provider-closure-transport)
now requires, and 8 corpus fixtures, among them
`pass/calls/callback_future_registration_cycle`,
`pass/modules/same_name_requirement_field_calls_exit` and
`fail/effects/boundary_empty_provider_hides_reach`. They call a raw requirement
such as `WindowsConsoleIo::write(1, &mut cell, 1)` from a body whose signature
is fixed by the requirement it satisfies, so it has no `&Binding<R>` to pass.
Since `31312173a4e` they are refused with "a boundary call requires an explicit
shared `&Binding<R>` authority argument".

**Existing rules, which point both ways:**

- Explicit binding always.
  [Expressions](wiki/spec/language/expressions.md): "A call through an
  established service binding uses the qualified static form
  `R::operation(&binding, arguments)` ... A missing, differently typed, or
  additional binding argument rejects; no ambient binding search or implicit
  insertion repairs it."
- Provider bodies reach their closure without one.
  [Platform services](wiki/spec/build/platform_services.md#provider-closure-transport):
  "The closure dependency is not a second `Binding<RawFamily>` field in
  portable data ... or an implicitly manufactured binding value. It is static
  provider-plan structure beneath the one portable binding. Entered portable
  calls may invoke only the raw rows retained in that checked closure".
  [Effects](wiki/spec/language/effects.md): a checked body must declare
  `reaches` "including for a receiver-free static boundary call", and "A trait
  identity may name an internally selected binding when no parameter path
  exists".
- Silent: the expressions rule speaks of a call "through an established service
  binding"; no contract says whether a provider's call into its retained
  closure is such a call, nor what source spelling it uses if it is not.

**Decision needed:** inside a body that `satisfies` a requirement (or installs
as a native callback), is a call to a raw requirement in that provider's
checked closure

1. **a closure call with no authority argument**, the static form
   `R::operation(arguments)` admitted only there, with reach and closure
   membership checked; or
2. **an ordinary binding call**, which then needs a stated way for a provider
   with a fixed signature to obtain `&Binding<R>`.

Recommendation: option 1, since the closure-transport ruling already forbids
manufacturing a `Binding<RawFamily>` for these dependencies.
