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

### Q1 - In `ensures` and in a crash route guard, does the name of a `mut` by-value parameter mean the argument or the value the body last assigned?

Decision name: `mutable-parameter-contract-name`.

**Customer.** A machine that counts with its own parameter and states what
it returns:

```omega
machine advance(mut position: u32, step: u32) -> u32
requires position <= 10, step <= 10
ensures result == position + step
{
    position = position + step;
    position
}
```

A caller imports `ensures` by substituting its argument for `position`, so
under one reading this contract is true and useful, and under the other it
says `result == result + step` and the body is rejected.

**Gap.** [Mathematical values](wiki/spec/terminal-psi/mathematical_values.md)
says a caller importing an `ensures` "requires the matching result case and
argument/result substitution", and
[fact validity](wiki/spec/terminal-psi/fact_validity.md#entry-facts-and-crash-coverage)
says "Entry hypotheses bind exact invocation formals" and "Current body
values are not entry snapshots". Both point at the argument, but neither
says what the *source name* denotes once the body has assigned it.
[Ownership](wiki/spec/language/ownership.md#owned-local-mutability) covers
`mut` locals and says only that "the recipient may bind its own value
mutably". [Dependent values](wiki/spec/language/dependent_values.md) uses
`old(entry.owner)` for the entry state of storage behind `&mut`, which
suggests a bare name in `ensures` means the state at return; for a by-value
parameter that state is invisible to the caller.

**Options.** (a) The name always means the argument; the body's later value
is unnameable in the contract. (b) The name means the value at return, and
`old(position)` names the argument, as for `&mut` storage. (c) A contract
clause other than `requires` may not name a `mut` parameter at all.

**Until answered.** The lowered-form checker refuses such a clause as not
supported (`contract clause that names a mutable parameter outside
requires`); it accepts no program under either reading.

### Q2 - Does an unsuffixed operator apply to one policy-qualified operand and one plain binding, or only to a plain literal?

Decision name: `policy-operator-plain-operand`.

**Customer.** A hash or checksum loop that keeps its state in a Wrapping
binding and mixes in plain values:

```omega
machine mix(hash: u32 in Wrapping, byte: u32) -> u32 in Wrapping {
    hash * 31 + byte
}
```

`hash * 31` is the case every reading accepts. `... + byte` adds a plain
`u32` binding to a `u32 in Wrapping` value.

**Gap.** [Policy domains](wiki/spec/language/numeric_values.md#policy-domains)
says core declares each unsuffixed operator "for two operands carrying the
domain and for one such operand with a plain operand of the same carrier, so
`counter + 1` needs no cast". Read as written, `byte` is a plain operand of
the same carrier and the program is valid.
[Overflow policies](wiki/spec/language/numeric_values.md#overflow-policies)
says "incompatible policies do not mix implicitly" and "A machine naming no
arithmetic policy publishes Exact", and the checker being removed rejects
this exact program as "mixed arithmetic domains ... one operand is `Wrapping`
and the other is `Exact`", while accepting the literal. Neither page says
whether the plain-operand declarations were meant for literals only, and
[literals and expected types](wiki/spec/language/domains.md#literals-and-expected-types)
makes a literal an ordinary plain value, which would leave no way to tell the
two apart by type.

**Options.** (a) Any plain operand of the same carrier, as the sentence
reads. (b) A literal only; a plain binding needs `as` first. The same answer
applies to the comparisons core declares for these shapes.

**Until answered.** The lowered-form checker serves the literal and refuses
the plain binding as not supported (`operator between a policy-qualified
operand and a plain operand that is not a literal`).

### Q3 - Are `uN` and `iN` integer types for every width up to the largest built-in one, and are those names reserved now?

Decision name: `finite-width-integer-carriers`.

**Customer.** The lowered-form checker is the only checker since 2026-11-13,
and its integer lanes are being written now: arithmetic, casts, domains, byte
views, layouts and calling plans (`LOWERED-FORM-CHECKER` on
[the task board](TASKS.md)). Each is written either against "a sign and a
width" or against the eight built-in names. Protocol and device code is the
language-side customer: a 31-bit sequence number or a 2-bit mode has an exact
range today only through a wider carrier plus a domain, and its wrapping
arithmetic is written by hand as a mask.

**Gap.** [Arbitrary-width integer carriers](wiki/proposals/0008_arbitrary_width_integers.md)
is "proposed, not accepted" and says no customer was shown when it opened.
[Unbounded computational integers](wiki/spec/language/numeric_values.md#unbounded-computational-integers)
were ratified on 2026-11-11 and say that decision "does not approve arbitrary
finite widths". Nothing reserves the spellings: `i2`, `u13` and `i31` are
ordinary identifiers today, so a program may declare a field or a machine with
such a name, and ratifying the types later breaks it.

The compiler is half shaped for either answer. The checker's vocabulary
already states an integer type as a carrier, a sign and a bit count
(`IntegerType` in `omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs`).
The typed trees state it as a closed list of eight names
(`PrimitiveType` in `omega-rust/psi/compilation/representations/typed-trees/src/type_system.rs`),
with macros that enumerate them; code written against that list is what a
later ratification would have to revisit.

**Question.**

- **(a)** Are `uN` and `iN` types for every width from 1 to the largest
  built-in width, each a distinct type with the exact range of its width, no
  implicit conversion between widths, and `Exact`, `Wrapping`, `Saturating`
  and `Trapping` meaning at width `N` what they mean today (wrapping is modulo
  `2^N`)?
- **(b)** If (a) is not decided yet, are the spellings `u<digits>` and
  `i<digits>` reserved now, so no program can take one as an identifier?
- **(c)** Under (a), is `u1` a type distinct from `bool`, and does a value
  occupy the smallest standard container that holds its width, with
  bit-packing left to layouts as it is for the built-in widths?

Outside this question, and left in the proposal: widths beyond the largest
built-in one, which need multi-limb arithmetic and calling rules, and generic
constraints over integer families ("any signed carrier").

**Until answered.** New checker code states an integer as a sign and a width
wherever the vocabulary already allows it and does not add to the closed list
of names; no `uN` or `iN` spelling beyond the built-in eight is accepted, and
none is reserved.
