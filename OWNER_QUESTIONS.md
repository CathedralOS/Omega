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

<a id="stage-05-unselected-rewrites"></a>
### 1. Are Stage 05 call inlining and vector promotion legalization or optimizations?

The optimization contract says no "compiler default, build mode, or broad
`O1`/`O2`/`O3` level implicitly enables an optimization", and that "mandatory
normalization, legalization, allocation, and realization remain necessary
whether or not optional profitability rewrites are selected". It does not say
which side of that line a given rewrite falls on.
[Exact selection](wiki/spec/build/optimizations.md#exact-selection)

Native Stage 05 runs two rewrites on every build, after instruction selection
and with no selection, catalog entry or execution record.
`selection_inlining.rs` (1,919 lines) inlines acyclic internal calls and merges
fall-through blocks. `selection_vector_promotion.rs` (5,381 lines) moves packed
memory operations into 128-bit vector registers, replaces single-block local
slots with registers, merges adjacent 16-byte stores and deletes dead stores.
Nobody has checked whether every program still compiles without them.
[Call site](source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/selection.rs) ·
[Inlining](source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/selection_inlining.rs) ·
[Vector promotion](source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/selection_vector_promotion.rs)

**Recommend making both optimizations:** give each a Stage 06
`SelectedLowering` catalog entry with its own validator and leave it off by
default, keeping in Stage 05 only the part a target needs to lower some program
at all. Alternatively, classify both as mandatory legalization and keep them
on in Stage 05. Which are they?

<a id="windows-handle-release-replay"></a>
### 2. Does the product offer receiver admission, or does the Windows handle release proof wait?

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
### 4. What does a sum input denote in a logical contract?

#### Context

A user wants a law about a sum, such as a build step that never reports
`RunResult::NotRun`. They want to pass that law as a proof argument the way a
record law is passed today. The specification says logical evidence states
"the contract's implication over fixed mathematical subjects". It requires
general data to elaborate to checked inductive declarations "where induction is
required". It says payload fields "require the corresponding case knowledge".
None of these says what subjects a stored, non-recursive sum gives a contract.
[Theorem proof parameters](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use) ·
[Inductive elaboration](wiki/spec/proofs/foundation.md#universes-functions-and-recursion) ·
[Case knowledge](wiki/spec/language/data_and_literals.md)

Current implementation: a record input is one subject per scalar field. A
nested record's fields count in its place, and each data invariant is a
premise (`terminal_semantics::theorems::data_subjects`). A sum input is
refused at the theorem's declaration with "logical data subject is not a plain
record". Self-containing indexed data already has its own route, an erased
family theorem.
[Data subjects](source-rust/subsystems/compiler/psi/semantics/src/theorems/data_subjects.rs) ·
[Indexed families](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/families.rs)

#### Problem statement

The premise holds. The open part is the denotation, not the compiler. "Where
induction is required" leaves non-recursive data free to use a non-inductive
encoding, and records already use one. A sum differs from a record because
its payload exists only in its active case. So a sum cannot simply become one
always-defined scalar subject per payload field, the way a record field does.

#### Proposed solution

**Recommend:** *A logical contract's sum input denotes its active case and that
case's payload. Its subjects are one Boolean per case, of which exactly one
holds, and the scalar payload fields of every case. A clause reads a payload
field only under that case's membership. The implication must hold for every
payload value, so an inactive case's payload constrains nothing. Self-containing
data stays an indexed family's evidence.*

With `data Choice { case One(x: u8); case Two(y: u8); }`:

- `requires c in Choice::One && c.x != 0u8 ensures c.x > 0u8` is accepted.
  The payload read sits under **`c in Choice::One`**. Applying it owes both
  premises at the actual sum.
- `ensures c.x > 0u8` with no case premise is rejected at the declaration.
  **`c.x`** has no case knowledge, which is the same rule ordinary contracts
  follow.

Accepting the source needs Stage 05 to read case membership and guarded
payload clauses. Recording the obligation reuses the existing premise goals.
Proving it needs the checker to bind each case Boolean from the case reaching
the application and each active payload from its contents. An inactive payload
gets any fixed value, which is sound because the law quantifies over it.

#### Alternates

- **One inductive subject.** Elaborate every sum, recursive or not, to a checked
  inductive declaration and give the contract a single binder of that type.
  This also covers whole-value identity, `c == d` as one subject. It costs a
  non-scalar binder in Terminal Psi's theorem contract, the kernel contract
  type and citation checking. The first example has the same outcome under
  either ruling.
- **Keep the refusal.** Sum laws stay inexpressible as proof arguments. Users
  restate them over the payload scalars at each use.
