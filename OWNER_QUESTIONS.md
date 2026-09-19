# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

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

## Open questions

1. **May the compiler-owned build vocabulary offer a constrained
   filesystem open/query/close chain?** (named decision:
   `build-vocabulary-filesystem-chain`). The two-axis review's remaining
   acceptance is a witness that an ordinary compile earns the evidence-bound
   explicit-empty release row. No authored source can produce the retained
   occurrence that witness needs. `BUILD_PRELUDE`
   (`source-files-to-assembled-syntax/src/source_assembly.rs`) declares only
   `BuildSource::{resolve, open, read, close}` and the `BuildOutput` and
   `BuildLog` facets, and `build_facet_filesystem_operation`
   (`checked-interpreter/.../evaluator/build_paths.rs`) maps exactly those
   names onto open, read, close, create and write, none of which is the
   constrained acquisition
   (`build-evaluation/src/evidence/replay_eligibility.rs` wants an
   `open_path_handle` attempt). Reaching the raw boundary instead is refused
   by a settled rule in `build-evaluation/src/admitted_build_program.rs`:
   "build.omg may not reach runtime boundary services; use the
   compiler-owned Build facets." Witnessed at 7be8d50205 (macOS ARM64):
   a compile running the widest lifecycle a build machine can request
   retains three receipted attempts and a verified replay record, and
   `filesystem_native_handle_query_release_contracts` still derives zero
   contracts (`compiler/tests/terminal_authority.rs`). Decision needed:
   (a) add one compiler-owned Source facet that issues the constrained
   chain over a `BuildPath`, the shape std already composes for Windows
   canonicalization (`open_path_handle`, `final_path_name_by_handle`,
   `close_handle`), which widens the authored build surface that
   [permissions](wiki/spec/build/permissions.md) and
   [declarations](wiki/spec/build/declarations.md) currently fix; or (b)
   the constrained row is earned only by program-side filesystem release
   and never by a build, in which case the two-axis acceptance should name
   a program customer instead, and the broad `Filesystem` summary retires
   on that route. Until answered, the summary stays, the settlement wiring
   stands unused, and **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** and
   **FILESYSTEM-RELEASE-CONTRACT** cannot close.

2. **May a provider's selected plan resolve an installation-bound row it
   owns, or does that wait on receiver-bearing selection?** (named decision:
   `installation-bound-row-nested-resolution`).
   [Interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
   settles what the completion row should be without ambiguity:
   acknowledgement "has a provider-neutral bounded abstract row beneath
   `MachineControl + PortIo`", and the language guide already prints that
   declaration. `source/library/core/interrupt.omg` is the laggard, still
   publishing an unbounded `reaches PortIo`, so no owner input is needed on
   the row itself. What is blocked is landing it. Migrating the declaration
   is a verified one-line change that takes `calling_policy_plans` from 60
   of 60 to 52 of 59: every realization of the installation-bound
   `InterruptEntry::enter` must settle the linear acknowledgement and so
   retains the nested bounded row, and the selected-row rejection that
   **BOUNDED-INSTALLATION-REACH-ROWS** tells us to keep then fires on the
   shipped route. No satisfier for `complete` can be authored today, because
   a checked body cannot discharge the linear receiver, which is the gap
   **TOP-LEVEL-BOUNDARY-REQUIREMENTS** already records. Decision needed:
   (a) installation resolves a nested bounded row against the same provider
   plan that owns it, which the item currently forbids by saying "no nested
   substitution step exists, so keep that rejection", and for which
   `RealizedMachineContractEnvelope::concrete_service_reach` looks like the
   representation; or (b) the whole bullet waits on receiver-bearing
   requirement selection, and the library declaration stays unbounded and
   knowingly stale until then. Until answered, the completion route cannot
   migrate. The rejection itself is now driven from authored source by
   `calling_policy_plans/opaque_boundaries.rs::
   selected_realization_with_an_unresolved_installation_bound_row_rejects`,
   so whichever route is chosen, losing that fence is a red test.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
