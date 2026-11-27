# Owner Questions

Only the two owners, Zac and Jarod, answer an entry. An owner's word in a
session is the ruling. To close an answered question at the owner's direction,
update the governing documentation or specification, point existing references
there, and remove the question from this queue in the same PR. Track any
remaining implementation work in the owning task.

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
under [the current policy](source-rust/subsystems/compiler/psi/README.md#changes-to-psi).
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

<a id="windows-handle-release-replay"></a>
### 1. Does the product offer receiver admission, or does the Windows handle release proof wait?

The permission contract says ordinary production requires no receiving policy,
and "an explicitly requested deployment compatibility check applies the
supplied policy". It also specifies the bounded proof that lets one Windows
`CloseHandle` or `DuplicateHandle` call count as an ordinary release. The
command-line contract defines no request that supplies a receiving policy.
[Production versus receiver admission](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission) ·
[Bounded release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof) ·
[Command line](wiki/spec/build/command_line.md)

The 15 `windows_handle_*.rs` files in provider admission (12,501 lines)
implement that proof. `checked_calls` returns before doing any work unless a
receiving policy is present. `psi_to_native.rs:131`, the only place that builds
the native compile request, passes `terminal_authority_permission_policy:
None`, so only tests reach the proof. Physical behavior exclusions also
consume this classification, but they do not run the proof.
[Proof entry](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_review/windows_handle_replay.rs) ·
[Native request](source-rust/subsystems/compiler/psi-to-native/psi_to_native.rs) ·
[Physical exclusions](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)

**Recommend archiving the proof:** move `windows_handle_*.rs` to
`_recycling/` and restore it with the request that first supplies a
receiving policy. Alternatively, specify that request on the command line now
and wire its policy through `psi_to_native`, which makes the proof reachable.
Which should the product do?

<a id="logical-sum-subjects"></a>
### 2. What does a sum input denote in a logical contract?

#### Context

A user wants to pass a law about a sum as a proof argument, the way record laws
are passed today. A *subject* is one value a theorem's clauses talk about. The
specification says logical evidence states "the contract's implication over
fixed mathematical subjects". It requires general data to elaborate to checked
inductive declarations "where induction is required". It says payload fields
"require the corresponding case knowledge".
[Theorem proof parameters](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use) ·
[Inductive elaboration](wiki/spec/proofs/foundation.md#universes-functions-and-recursion) ·
[Case knowledge](wiki/spec/language/data_and_literals.md)

Observed today: a record input is one subject per scalar field. A nested
record's fields count in its place, and each data invariant is a premise. A sum
input is refused. An ordinary machine contract over the same sum is accepted,
and a call that passes another case fails the payload clause.
[Data subjects](source-rust/subsystems/compiler/psi/semantics/src/theorems/data_subjects.rs)

#### Problem statement

**Premise 1: The specification does not say what subjects a non-recursive sum
gives a logical contract.** Holds (spec). Inductive elaboration is required only
"where induction is required", and no clause names a sum's subjects.

**Premise 2: The refusal comes from today's compiler, not the language.** Holds
(observed). The same `requires` and `ensures` are accepted on an ordinary
machine. Only the proof-argument path refuses them.

The decision is the denotation. A sum cannot copy the record encoding of one
always-defined subject per field, because a payload exists only in its case.

#### Proposed solution

**Recommend:** *A logical contract's sum input denotes its active case and that
case's payload. A clause that reads a payload field holds only in that field's
case, as in an ordinary contract, so applying the law to a sum in another case
fails its premise.*

**Before**

```omega
data Choice {
    case One(x: u8);
    case Two(y: u8);
}

data Law = machine(c: Choice)
    requires c.x != 0u8
    ensures c.x > 0u8;

machine law(c: Choice)
requires c.x != 0u8
ensures c.x > 0u8
{
}

machine apply<L: Law>(c: Choice)
requires c.x != 0u8
{
    L(c);
}

machine entry() -> u8 {
    apply::<law>(Choice::One { x: 7u8 });
    0u8
}
```

Observed: refused at `apply`'s signature with "logical data subject is not a
plain record".

**After: accepted.** No code changes. The same program compiles under the ruling.

Expected: accepted. `L(c)` owes `c.x != 0u8`, which `apply`'s own `requires`
supplies.

**After: rejected.** Only `entry` changes. It passes the other case:

```omega
machine entry() -> u8 {
    apply::<law>(Choice::Two { y: 7u8 }); // another case
    0u8
}
```

Expected: rejected at the call, because `c.x` has no value outside
`Choice::One`. Observed today for the ordinary call `law(Choice::Two { y: 7u8 })`:
refused with "the clause's observation has no reaching value identity".

Compiler work, not part of the ruling: the contract's subjects are one Boolean
per case and the scalar payload fields of every case. The law must hold for
every payload value, so an inactive case's payload can take any fixed value at
an application. Self-containing data stays an indexed family's evidence.

#### Alternates

- **One inductive subject.** Elaborate every sum to a checked inductive
  declaration and give the contract one binder of that type. Both examples above
  turn out the same. It also covers comparing whole sums, such as
  `requires c == d`. It costs a non-scalar binder in Terminal Psi's theorem
  contract, the kernel contract type and citation checking.
- **Keep the refusal.** Both examples above stay refused at
  `apply`'s signature. Users restate sum laws over payload scalars at each use.
