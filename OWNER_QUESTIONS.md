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
### 1. Does the first release offer receiver admission, and how does a receiver supply its policy?

Jarod routed this question to Zac on 2026-11-27.

#### Context

Receiver admission lets whoever installs an Omega binary check, before running
it, that the binary exercises only the dangerous authority their own policy
allows (filesystem writes, process control, hardware and so on). The spec
defines it as optional and separate from building: "Ordinary compilation and
retained-Terminal lowering produce artifacts, not permission to install or
execute them". The receiver "selects the policy's versioned package, exact
dependency closure and concrete configuration independently of the offered
program". It also defines the bounded proof that lets one Windows
`CloseHandle` or `DuplicateHandle` call count as an ordinary release.
[Production versus receiver admission](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission) ·
[Bounded release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof) ·
[Installation boundary](wiki/spec/build/executable_installation.md) ·
[Separate operation](wiki/spec/build/compiler_request.md)

An earlier owner question, "Receiving-policy selection surface", asked how a
project names that policy package. At the time every native build stopped at
`Console::exit_process`, because the compiler defaulted to an empty
deny-by-absence policy. `19879a59627b` (2026-09-17) closed it by making
ordinary production need no policy. It chose no selection surface, so the
question below is the part left open. Of its three options, the proof-carrying
code policy was removed from the spec on 2026-10-15 (`5b15e5f1`), and a
`--receiving-policy <file>` flag would bypass the package review and pinning
the spec now requires.

Implementation on `fe289158`:
- `review_terminal_authority_closure` walks every reachable provider leaf on
  every build and records its exact mechanism and classification. Package
  review and `Build::exclude_physical_authority` consume that record.
- `TerminalAuthorityPermissionPolicy` (version 1, deny-by-absence rows of
  service schema, requirement and permitted disposition) and the check of
  each leaf against it are written. The native artifact identity has a slot
  for the policy's identity.
- The 15 `windows_handle_*.rs` files (12,501 lines) implement the bounded
  proof. `checked_calls` runs it only on a PE target, with a policy supplied,
  for a `CloseHandle` or `DuplicateHandle` the ordinary classifier cannot
  place.
- `psi_to_native.rs:131`, the only place that builds the native request,
  passes `terminal_authority_permission_policy: None`. Only
  `build/tests/provider_publication.rs` reaches the policy check and the
  proof. No release gate in
  [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
  names receiver admission.

[Review entry](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_review.rs) ·
[Proof entry](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_review/windows_handle_replay.rs) ·
[Native request](source-rust/subsystems/compiler/psi-to-native/psi_to_native.rs)

#### Problem statement

The decision is product scope, not whether to keep one proof: the spec
defines a complete optional feature that no user can request. Shipping it
needs four things the spec leaves open:
1. How a receiver names its policy package. Under the spec's review and
   pinning requirement this is a declaration such as
   `builder.receiving_policy(Source::Path { location: "../policy" })`
   recorded in `omega.lock`.
2. How a policy package writes its rows as Omega data.
3. Who runs the check and when. The spec calls it a separate operation by the
   receiver against the native artifact, but the code reviews inside the
   producer's native compile. Either the receiver rebuilds, or an
   `omega admit <artifact>` operation works from the artifact's retained
   evidence.
4. How the CLI reports the verdict or the refused leaf.

#### Proposed solution

**Recommended ruling:** "Receiver admission is not in the first release.
Archive the 15 `windows_handle_*.rs` files to `_recycling/` and restore them
with the first request that supplies a receiving policy. Keep the review
walk, the policy type, the optional request field and the artifact identity
slot."

Take a program that opens a file with access zero and `OPEN_EXISTING`,
queries it and calls `CloseHandle`:
- `omega build`: under either choice it produces the artifact with no
  receiver-admission claim. The ruling changes no observable behavior.
- A receiver's admission request (a hypothetical surface): only shipping adds
  it. The proof would let that **`CloseHandle`** count as an ordinary release,
  so the policy needs no deletion row for it.

Before archiving, confirm that physical behavior exclusions keep their
classification without the proof files.

#### Alternates

- **Ship it before release.** Answer items 1–4, wire the policy from Build
  into `psi_to_native`, and test all five hosted targets. This makes about
  13,800 lines of review code live. It needs a named receiver, such as a
  package registry, distribution or organization admitting Omega binaries,
  under the `OWNER-BLOCKED` rule; none is recorded.
- **Keep the proof in place, unreachable.** No work now, but 12,501 lines
  reachable only from a test must keep compiling through every refactor.
- **Archive the whole review policy path as well.** A cleaner cut, but
  restoring it later re-plumbs Stage 02 and the artifact identity for about
  1,300 lines saved.

<a id="logical-sum-subjects"></a>
### 2. What does a sum input denote in a logical contract?

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
