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

### Q1 - How does an ordinary Run split or merge express its cell transfer?

Decision name: `typed-run-cell-transfer-surface`.

**Customer.** Dense `Vec<T>` must append, remove and move owned elements without
per-element tags. Its live storage needs ordered split/merge, including zero-sized
elements that still owe cleanup.

**Gap.** [Typed runs](wiki/spec/resources/typed_runs.md#retained-custody-and-transformations)
requires ordinary checked split/merge bodies to prove a typed-cell transfer, and
explicitly says byte/count equations are insufficient. The
[content contract surface](wiki/spec/resources/content_custody.md#source-conservation-contracts)
provides interval and quantity projections with `old` and `separate`, but no
projection or operation naming a run's ordered cells. The compiler-owned ordered
custody summary is ratified; the question is how an ordinary authored body
requests its transformation, not how Psi represents it.

For example, two zero-footprint runs have identical byte geometry whether merged
as left-then-right or right-then-left. Their count sum and granted-byte equations
cannot state that ordering. Recognizing a helper name or adding it to
`established by` would not check the promised transfer.

**Audit.** `core/run.omg` has empty/singleton operations but no split/merge;
there is no implemented ordered-cell contract surface to reuse. Ordinary vacant
extent transformations conserve intervals, not occupied values. The local merge
experiment supplies geometry and count equations but no cell-transfer proof;
its rejection is not evidence that those equations should suffice.

**Options.**

- Keep split/merge ordinary bodies and add a proof-only way to name, partition
  and concatenate opaque cell sequences. It exposes no contained resources and
  carries no runtime metadata, but adds an authored proof surface.
- Make split/merge compiler-realized custody operations, alongside initialize,
  extract and relocate, with ordinary library wrappers. The compiler checks
  exact inputs, partition index, placement compatibility and retained obligations.
  This changes the explicit ordinary-body decision, but adds no sequence-proof
  syntax. **Recommended for the smaller public mechanism.**

Neither option changes dense payloads, allocator ownership or the requirement to
transfer actual cells rather than recreate authority from their type and count.

### Q2 - Can one Build call select one provider for many slots or targets?

Decision name: `bulk-provider-selection`.

**Customer.** Every compile checks and runs core's
[build program](source/library/core/build.omg) before anything that depends on
it can start. That program makes 104 `Build::select_provider` calls. 98 of them
sit in `configure_float_defaults`, which runs once for each of five targets,
and 6 in `configure_directed_fma_defaults`. Every call names `NativeFloats`.
On the quiet Mac (release, one-line `--check`, 8 alternating rounds at
`ccc4a7caa6f`), swapping that file for one with a single selection cut median
`build.resolve_sources` from 1,098 ms to 856 ms and whole-process wall from
3,154 ms to 2,860 ms (-9%). That swap is a lower bound on the saving, not a
configuration anyone could ship.

**Gap.** [Provider selection](wiki/spec/build/provider_selection.md#declaration-and-choice)
resolves `Slot` to one exact boundary trait or one package-qualified
boundary-operator family, and keys each row to one target. Its
[operator-family rule](wiki/spec/build/provider_selection.md#operator-families-and-indexed-requirements)
covers every overload of one family, such as `Float::add`, atomically. It also
rules out hidden per-coordinate overrides. No contract lets one selection cover
several distinct requirements (`F32::negate`, `F64::square_root_toward_zero`,
...) or several targets. `NativeFloats satisfies FloatNativeOperations`, but
`FloatNativeOperations` is an ordinary trait whose requirements are separate
slots, so selecting it would not record the `Float::*` rows. The spec also says
that overrides "retain authored selection identity/order". A bulk form would
therefore need a defined expansion order.

**Audit.** The typed lowering
(`03_.../expressions/build_selections.rs`) accepts only
`select_provider<Slot, Provider>(builder, target[, mode])`. Each call lowers to
three checked calls: `prepare`, `build_selection_target` and `select_provider`.
Omega has no loop that iterates static slot operands. A generic helper over
`Op` would still need one call site per operation. No ordinary rewrite of the
file records the same rows with fewer checked calls.

**Options.**

- Add a target-set form, for example
  `Build::select_provider<Slot, Provider>(builder, [targets])`. It records the
  same per-target rows in the listed order and cuts the target dimension only.
  The 98 operation calls already appear once in source, so most of the measured
  saving remains unreached.
- Let a selection name the provider for every boundary requirement it
  satisfies, for example `Build::select_satisfied<NativeFloats>(builder, target)`.
  This expands to one row per `satisfies` clause, in canonical requirement
  order. It reaches nearly all of the measured saving, but adds a rule that
  derives slots from a provider's declarations.
- Keep the surface and make each `select_provider` call cheaper to check. This
  is implementation work and needs no ruling. Answering "no" to both forms
  above leaves only this route.

Under any option, each requirement still records its own row for each target.
No option changes `Float` meaning or lets a provider replace primitive operator
semantics.
