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

<a id="native-abstract-operations-phase"></a>
### 1. Does native lowering keep an abstract-operations phase?

The optimization contract lists the phases a build selection projects into:
"Psi, abstract operations, target operations/instruction selection,
pre-allocation, allocation recovery, post-allocation, and resolved layout".
No other specification page names an abstract-operations phase. The
specification names no Rust type for either phase, and the Terminal Psi
product contract places every Psi pass before publication.
[Exact selection](wiki/spec/build/optimizations.md#exact-selection) ·
[Optimization ordering](wiki/spec/terminal-psi/product.md#optimization-ordering)

No optimization belongs to the abstract-operations phase. Native Stage 03
refuses any selection for it. The nine passes Stage 03 can run are Psi passes,
and the native driver's post-Terminal selections cannot contain one, so in
every build Stage 03 builds its optimization unit, runs nothing, and checks that
the plan is unchanged. The `AbstractOperation` enum repeats 77 of its 94
variant names from Terminal Psi's `OperationKind`. The target-operations phase
owns two rewrites (`UnsignedPowerOfTwoRemainderMask`, `X86ScaledIndexedLoad`).
Stage 05 instruction selection applies them. Stage 05 reads the legalized plan
and never names Stage 04's `TargetOperationPlan`.
[Stage 03 entry](source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/src/abstract_optimization/mod.rs) ·
[Phase list](source-rust/subsystems/compiler/foundation/optimization-selections/src/lib.rs) ·
[Stage 05 rewrites](source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/lib.rs)

Native Stages 02 to 04 and the abstract-operations representation hold about
132,000 lines of Rust, tests included. Nobody has measured how much a direct
lowering would delete. Legalization and the target replay record that the
requested object audit reads would have to move, not disappear.

**Recommend retiring the abstract-operations phase:** drop it from the phase
list and let the native compiler lower the target-bound program straight to
the legalized plan Stage 05 reads, keeping the target-operations/instruction
selection phase for its two rewrites. Deleting `TargetOperationPlan` is then
implementation work. Alternatively, keep the phase as the place for future
target-neutral native rewrites, and keep Stages 02 to 04 as they are. Which
should the phase list say?

<a id="stage-05-unselected-rewrites"></a>
### 2. Are Stage 05 call inlining and vector promotion legalization or optimizations?

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
### 3. Does the product offer receiver admission, or does the Windows handle release proof wait?

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
