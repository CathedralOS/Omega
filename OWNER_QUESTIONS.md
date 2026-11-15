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

1. **Does core declare bitwise operators and policy-qualified shift counts for
   `Wrapping`, `Saturating` and `Trapping`?** The compiler refuses
   `bits | mask` when both are `u32 in Wrapping`, and refuses `mask << count`
   when `count` is `u32 in Wrapping`, as "not supported yet". The
   specification does not say whether either operation exists, so the refusal
   cannot become an acceptance or a missing-operator error without a ruling.

   [Policy domains](wiki/spec/language/numeric_values.md#policy-domains) says
   core declares "each unsuffixed arithmetic operator" and the comparisons for
   policy-qualified operands. [Policy operators](wiki/spec/language/numeric_values.md#policy-operators)
   lists the arithmetic operators (`+ - * / %`) separately from the two shifts,
   and [shift counts](wiki/spec/language/numeric_values.md#shift-counts) says
   bitwise AND, OR, XOR and complement "add no overflow-policy requirement",
   which describes their meaning but not whether a policy domain declares them.
   [Domain composition and operators](wiki/spec/language/domains.md#domain-composition-and-operators)
   and [operands](wiki/spec/language/domains.md#operands) make an operator on a
   qualified operand exist only when a declaration names that exact type, so
   if core declares none, `bits | mask` is a missing operator.

   Two decisions are open:
   - **Bitwise.** Does each policy domain declare `&`, `|`, `^` and `~` for
     the same operand shapes as `+` (two qualified operands, or one qualified
     and one plain operand in either order), returning the qualified carrier?
     Or must the source cast, as in `((bits as -Wrapping) | (mask as -Wrapping)) as +Wrapping`?
   - **Shift counts.** The compiler already treats `value << count` with a
     `Wrapping` value and plain count as `<<%`, reading "a shift's count keeps
     its own" type. Which count types do the declared shift signatures accept:
     only a plain count of any integer carrier, or also a count carrying the
     value's policy domain? If the count may carry a policy, does that policy
     select anything, for example does a plain value shifted by a
     `Wrapping` count reduce the count, or is that shape undeclared?

   Authored source already depends on the answer. `omega::core` uses bitwise
   operators on `Wrapping` values in `source/library/core/sha256.omg` (the
   message schedule and the `s0`/`s1` rotations) and
   `source/library/core/wire.omg` (`UnsignedRead::accept`). The samples
   `samples/cli/collections/bitset` and `samples/cli/collections/bitset_sieve`
   use both operations and stop at these refusals in the Terminal Psi reader
   (`read/expressions.rs` in `05_typed-trees-to-terminal-psi`).
