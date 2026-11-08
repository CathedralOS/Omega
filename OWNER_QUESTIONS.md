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

### Q1 - Can one Build call select one provider for many slots or targets?

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

### Q2 - Must every compile check library code its program cannot reach?

Decision name: `reachable-closure-checking`.

**Customer.** Every user compile. A one-line `omega --check` (`main.marker = 0;`)
runs Psi 05 on 837 machines across its three checks: core's Build, the user's
`build.omg`, and the product. Only 8 of them are reachable from those checks'
entries, even counting every contract, proof, lemma and requirement-satisfier
dependency the checker itself records. The product check covers 555 machines
and reaches one, `Main::main`. The other 554 are 408 core machines, the 46-machine
build prelude (also checked in both Build checks), 13 std machines, and 86
machines across all six `std/targets` directories. 269 of the 555 have no
incoming dependency edge at all (counts at `541152f9d5c`). On a quiet 16-core
Linux host at `a35508126ea`, Psi 05 is 1.43 s of the 2.8 s release compile, and
Jai (beta 2-026) builds the same program to a linked executable in 0.14 s. Binding one hosted target would raise
the product's reachable set to 28 machines.

**Gap.** The contracts settle the current behavior. They do not settle whether
the per-compile cost is required.
[Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
allow checked-tree pruning only after all authored source is parsed, resolved,
typed and checked, because deleting unreachable code must not make invalid
source valid.
[Component publication](wiki/spec/build/component_publication.md) says unused
library functionality "does not waive its ordinary source checks".
[Proof contracts](wiki/spec/proofs/contracts.md) already compute assumption
closure over the reachable checked declaration graph, and state that unused
declarations are not dependencies merely by sharing a package. None of these
says where the bundled library's ordinary checks must run: in every user
compile, or in a gate that admits that library.

**Audit.** The dependency graph used for the count is the checker's own
`declaration_call_dependencies`. It follows contracts, requires/ensures, proof
facts, lemma calls, specialization selections and requirement satisfiers. Body
calls alone give the same 8 machines. Type and data references were not
traversed, so a reachable-closure rule would also have to follow those. Reuse of
earlier check results is ruled out for now, so any saving must come from not
doing the work, not from replaying it.

**Options.**

- Keep the rule: every compile checks all authored source, including the whole
  bundled library. Per-compile cost grows with the library, not with the program.
- Check user-authored source in full, but check bundled library machines only
  within the program's reachable dependency closure. Each library release must
  then pass a library-wide check gate (corpus or CI) before it ships. A broken
  library proof that no program reaches would fail that gate, not a user's
  compile.
- As the second option, plus every implementation of each target the Build
  binds, so a compile still checks the program's declared targets in full.

Under any option, an invalid reachable declaration is still rejected, and
pruning after checking keeps its existing meaning.
