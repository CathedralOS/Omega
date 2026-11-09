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

### 1. May a private machine with no authored `requires` have its requirements inferred?

A body reasons under its entry contract, and each caller establishes that
contract. It is unsettled whether a private machine that authors no `requires`
has an empty entry contract, or one the compiler may infer from its body and
prove at every arrival. The answer decides whether this program is valid:

```omega
machine bit(count: u32) -> u64 {
    transition count < 64u32 {
        true -> calculate(count)
        false -> (0)
    }
    machine calculate(count: u32) -> u64 { 1u64 << count }
}
```

If the contract is empty, `calculate` must repeat `requires count < 64u32`. If
inference is allowed, its one caller's guard discharges the shift.

Contracts checked:

- [`domains.md`](wiki/spec/language/domains.md) under uninhabited domains: "The
  body reasons under its hypothetical entry contract; each caller must establish
  that contract for its actual arguments." It addresses authored contracts and
  is silent on a machine that authors none.
- [`state_contracts.md`](wiki/spec/language/state_contracts.md#bindings-and-arrivals):
  "body inference must justify any inferred private requirements on every
  arrival, not grant them from lexical ancestry." It presupposes inference but
  does not say when a private requirement may be inferred.
- [`assembly.md`](wiki/spec/language/assembly.md): "admissible private contract
  inference can discharge those goals", with every incoming edge justifying the
  inference. It leaves "admissible" undefined outside assembly.

Evidence: six `pass/` fixtures, recorded as having once checked, have this shape:
`arithmetic/runtime_boolean_alias_shift_count_exit`,
`arithmetic/runtime_boolean_guarded_bit_value_exit`,
`arithmetic/runtime_boolean_state_shift_count_exit`,
`arithmetic/runtime_transitive_bit_value_exit`,
`expressions/match_owned_mixed_projection_reads` and
`expressions/match_owned_parameter_field_subject`. A trial that seeded nested
entries from all callers, fail-closed on unknown callers and mutable places,
checked all six. Machines that do author a `requires` are outside this question:
their bodies assume only that contract.
