# Omega Package Manager

Start at [`src/package_manager.rs`](src/package_manager.rs): recover the project
transaction, admit install/update/resume requests, resolve the candidate,
and reconcile findings for publication. Application Build's
[`package_commands.rs`](../../application/build/src/package_commands.rs) runs
the target-review loop. Compiler preparation and read-only inspection have
separate entrances in [`operations`](src/operations/mod.rs).

[Source selection](../../../../wiki/spec/packages/sources.md) owns the
requester-local graph and workspace-member contract. Acquisition enforces the
[resolver security boundary](../../../../wiki/spec/packages/resolver_security.md).

The ratified destination is Cargo-like repository install/update with
compiler-derived reachability, unsafe API, and assumption review. The lock
records pins, the graph, accepted policy, occurrence-scoped compilation inputs,
and decisions; the project trusts
whoever lands it. No sealed/certified `PackageInstance` or certificate of lock
acceptance is required. Source publication consumes checked review and exact
project decisions directly. Actual
compiler proof/reach checks and native artifact validation remain independent
of installation.

```text
package-manager/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs             Rust module wiring and exports
│   ├── package_manager.rs command admission, decision join and publication
│   ├── package_manager/   edit planning, review files, proposal recovery
│   ├── operations/        compiler preparation, inspection, publication
│   ├── declarations/      checked package declarations from build.omg
│   ├── resolution/        exact source selection and dependency closure
│   ├── lock/              persistent source-scoped project policy
│   └── review/            admit review passes, join evidence, compare and decide
```

`PackageCommandTransaction` keeps the project lock and retained review-file root
alive while application Build executes its admitted target requests. Completion
requires the whole target roster from that exact admission before writing decision documents or
publishing accepted files. The original proposal handle remains borrowed across
that work, so completion checks the file that admission read, not a replacement.
Binding discovery, exact occurrence reuse, policy comparison and cache admission
remain here. Application Build executes the admitted dependency work through
one whole-pass executor, shared by commands and audit. Package policy does not
prepare Builds, publish components or finish compiler reviews.
Supporting packages are siblings under [packages/](../README.md).
The independent [declaration](../build-declarations/README.md),
[output](../build-output/README.md) and [compilation-input](../compilation-inputs/README.md)
packages are below both the compiler and this command application. Their placement
does not make the compiler depend on install/update orchestration. Source
acquisition constructs input custody; compiler consumption records the exact
source units it used; review joins those results to package policy.

[`PackageInspectionTransaction`](src/operations/inspect_packages/README.md)
admits `omega audit packages`: current project source, pinned dependencies,
fresh compiler findings, and comparison with accepted policy without publishing
changes. Default summaries and `--details` separate readable inspection from
the complete normalized policy.
`operations::stage_build_dependency_edit` joins the conservative declaration
planner to a proposed source snapshot without writing the live `build.omg`.
It accepts an automatic replacement only and checks the planner's old-file
digest. The staged local project resolver reads the proposed declaration while
preserving the original root path/context for package identity and relative
dependency lookup. Keep the edit plan and staged original identity through
review and publication. The pin-aware staged resolver takes `GitDependencyPins`
from the accepted source graph and exact selected package keys. Unchanged Git
locator/revision requests keep their recorded commit, tree, and content; an
empty update selection preserves all existing Git requests for installation.
Selected Git repositories refresh as a unit, including their reachable workspace
members and relative Path edges. Unrelated repositories, including unchanged
transitive requests, stay pinned. New or changed requests resolve normally.
Missing preserved pins use either offline failure or explicit exact-commit fetch,
never selector refresh. This policy changes resolution only, not accepted policy.

`GitResolutionOptions::offline` separately forbids all fresh Git selection,
including new transitive requests, and overrides preserved-pin fetch permission.
CLI `--offline` uses that restriction for install/update and reuses proposed pins
on resume. Missing old-source bytes only limit code diffs, not lock policy
comparison; historical recovery also honors the network restriction.

`operations::review_package_change` checks an already-resolved candidate for an
exact target, with ordinary contract obligations rejected by Psi checking,
and compares its risk-bearing projection against an optional accepted lock section. Missing
baseline means fresh review; unavailable old source is not needed for comparison.
The resulting `PackageChangeReview` exposes compiler findings and the ordinary
editable review document's input, without a reconstruction question, native
artifact, or evidence-promotion prerequisite.

After resolving the exact comparison, `propose_lock_target` requires all choices
to accept, rechecks retained source snapshots and selections, and constructs the
proposed target from compact candidate acceptance rows and direct decision history.
It also retains each review's exact semantic bindings and compiler input identity,
captured before checking and rechecked afterward and before proposal. The identity
joins the compiler's embedded build-input digest and selected toolchain library
content; it invalidates stale inputs and is not a compiler or audit attestation.
Rejected, foreign, or stale choices cannot produce a proposal through this
operation. The result remains an unwritten `PackageLockTarget`, not permission
to publish. `publish_reviewed_package_change` joins the staged edit and all
retained target reviews to exact current project bytes, then records commit
intent and publishes the pair under a project mutex. Interruption recovery
completes forward only from recorded old/new contents; unrelated edits stop it.
Ordinary project preparation coordinates with existing pending state before
snapshotting. See [publication](src/operations/publication/README.md) for the
file protocol and platform limits. The [command operation](src/package_manager/README.md)
owns package/alias selection, persisted per-target findings, exact candidate
resume, and reviewed publication for `omega install` and `omega update`.

`begin_local_project_preparation` opens recovery and snapshot policy before
application-owned execution resolves the source graph. Accepted discovery
preserves dependency pins, fetching only recorded commits when needed.
Accepted compilation requires exact source identities for every occurrence,
including the local root, together with the dependency projections, identities,
roles, and selected edges. A source edit requires explicit `omega update` review
before ordinary accepted compilation; there is no mutable-root exception.
Local package identity remains tied to its canonical path.

The application's `BuildRequest` accepts invocation-local `offline` selection.
Cached accepted Git pins and unlocked local-only graphs
remain usable. Missing recorded Git content fails without transport. Offline
selection does not waive source/compiler identity matching or change the lock
format.

Per the owner ruling of 2026-10-03, ordinary checks and native production use
accepted compilation, not the candidate-review pipeline. The application
[`omega-build`](../../application/build/README.md) owns their compiler invocation,
including inspection and SDK consumers. Package management's
`admit_project_compilation` returns ordered requests plus a completion owner
retaining staging, aggregate sponsors and source custody. Completion maps occurrence
failures back to packages, checks compiler-library identity and reports disposal
failures. Neither admission nor completion invokes compilation.
Every accepted occurrence's source and compiler input identities must match its
lock record before Build execution. A match supplies the exact accepted semantic
bindings for that occurrence; a mismatch rejects with the package name and
`omega update`. Neither route performs an inline audit or rewrites acceptance.
Dependency product checking follows the consumer's `use` items and their
transitive module closure rather than compiling each dependency's package root.
The requested root source entry, scoped Build execution, generated-source custody,
ordinary trust settlement, and requested compiler observations remain intact.
Reporting uses the retained checked root; it does not run Build again.

Check-only observation without a lock is permitted but creates no accepted state
or native-production authority. Malformed and older locks reject rather than
becoming absent acceptance. Full-package discovery/bound review remains an
explicit review, acceptance, or update operation.

Candidate checking executes the existing scoped build evaluator, so it is not
side-effect-free: package-input reads, disposable-output writes, and compiler
logging can occur even when later checking fails. Runtime boundary services are
rejected before authored build execution. Package runtime capability decisions
do not grant additional build or resolver authority. Accepted project files are
not modified by this operation.

Opaque declaration visibility, selected conformance validity and by-value
representation demands are checked during compilation, without reading a
review projection. The consumer's Build selects the representation for the
combined source compilation; a dependency's historical root-build choice is
not a second selection. Independently compiled artifact composition is a
different boundary and still requires representation agreement.

Its package-aware native operation consumes a `PreparedLocalProject` carrying
the source closure and accepted `omega.lock` section for one exact target.
Missing acceptance rejects before Build. Matching source/compiler identities
permit use of the retained occurrence bindings without a fresh production review
or a second approval file; mismatches direct the user to `omega update`.
Native compilation never writes acceptance. Source-only changes remain visible
in explicit package inspection and review. Retained permissions still constrain
production, while receiving permission policy remains a separate input.

The CLI update/resume regression reaches the independent native publication
gate; executable publication remains blocked by the missing function-validation
evidence producer under [`TARGET-MATRICES`](../../../../TASKS_OPTIMIZER.md#verification-and-rollout).
Retained native
artifact validation is not a claim of successful publication.

`declarations` reads the statically checked package declarations in
`build.omg`; Omega has no second package manifest. `resolution` binds those
declarations to immutable sources and reconciles one exact closure. `review`
turns that closure into compiler-issued facts and root-owned decisions; there
is no parallel initial-conflict comparison or second approval decision. Source
contract validity belongs to Psi's call and
normal-exit checks, including a return through a tail call. The result remains
review and policy state.

Ordinary accepted compilation consumes accepted package inputs without
repeating review: the driver reads the lock's Terminal permission rows
directly. Build-host permissions never project into the product's native
authority, even when both roles use the same target. Changed requirements need
ordinary project review.

A receiving-policy verdict
belongs to explicit artifact admission, not ordinary package/native production.
Absence of receiver policy makes no receiver-admission claim; it neither blocks
ordinary production by itself nor invents an execution grant. Install/update must
separately surface and accept restricted build requests before execution, retain decisions in the lock,
and require actual host grants under
[restricted-build acceptance](../../../../wiki/spec/packages/acceptance.md#restricted-build-acceptance).

Captured source and named inputs with bounded private output staging need no
restricted-action decision. The filesystem sponsor owns that fresh staging
session and cleanup; generic sponsors for supplied directories carry no such
exemption. Broader host scopes still project restricted requests, whose retained
lock meaning is distinct from actual invocation grants.

The top-level invocation owns one `PackageBuildSession` across its review
targets and rounds. Application Build's
[`package_inspection.rs`](../../application/build/src/package_inspection.rs)
creates and disposes the audit's owner and executes its ordered target roster.
The CLI's
[`packages::run`](../../application/cli/src/cli/packages/mod.rs) retains the
install/update owner across automatic acceptance and resume rounds, lending it
to `omega_build::execute_package_command`. A later user-issued command creates its own
session. Discard requires only project-state recovery, without a Build owner.
Package commands close staging before publishing accepted project files;
operation and decision-file failures return to the owner for disposal.

Start candidate review at
[`passes.rs`](src/review/candidate/passes.rs): select discovery, derive consumer
bindings, then select a bound pass only if needed. Each pass borrows the caller's
session; it neither creates sponsors nor disposes them. Every fresh occurrence
in a target/round pass receives a new
sponsored directory from the invocation's monotonic sequence. Component demands
join the completed Build's opaque continuation before Product checking; they do
not trigger a second compilation or replay Build effects. Closing an owner
prevents another review from using it.
If a persisted dependency review lacks a demanded component description, refresh
that dependency while retaining completed sibling reviews and the consumer's
Build continuation. Reuse still compares complete inputs, entry and snapshot;
changed generated inputs require fresh work. Interrupted review sets are private
progress, not an accepted root-wide review.
Its [`occurrences.rs`](src/review/candidate/passes/occurrences.rs) admits each
build/product occurrence and joins compiler evidence, using the same roster as
lock comparison and reconstruction. Application Build's
[`package_review.rs`](../../application/build/src/package_review.rs) owns the
dependency loop, compiler calls and cache-miss recovery. Acquisition and parsed
source are shared; checked targets,
generated bundles, bindings, component descriptions and reviews are not.
Build session and staging ownership are shared through `build_session`; review
policy and ledger budgeting remain with the review owner.
[`review_set.rs`](src/review/candidate/review_set.rs) describes the issued output.

`review_resolved_packages` is the candidate entrance for install/update and
audit. Both discover bindings before the final bound pass; observational review
projects restricted requests but never grants them. Accepted production builds
admit grants separately through `accepted_compilation`.
Each package uses its authored `main.omg` when present, otherwise
its authored `build.omg` through the compiler's existing build-file entrance.
This creates no source exports: missing imported modules still reject. Explicit
root check entries and native application entry requirements remain unchanged.
It uses one preliminary compiler review only to discover supported
package-owned semantic surfaces, then recompiles with exact consumer-scoped
bindings. Only that final review may proceed to policy comparison and admission;
the discovery pass is neither policy nor evidence that an audit occurred.
The passes share one compiler session. It retains prepared source, completed
Pure Builds and exact language judgments; package review owns source bindings,
dependency order, occurrence consent and sponsor accounts, not compiler caches.
Each activation still validates custody, selections, generated inputs and grants.
A matching prior judgment lends checked storage only after fresh admission.
`PackageSourceBindings` records resolved roots by package position; it is not a
frontend checkpoint table. The command's `PackageBuildSession` persists across
review rounds and targets, so sponsors are not replenished per target.
Ordinary accepted compilation constructs a dependency-ordered request from those
inputs. Application Build schedules dependencies and joins generated outputs;
the compiler retains opaque Build continuations and publishes demanded independent
components. Package operations no longer drive
`prepare_build` / `check_build` or retain their continuations. Review requests
component descriptions through the compiler too, without a backend-publication
dependency in this package.
Check, native and inspection requests name their finished product up front.
The compiler owns root permission comparison and completion; ordinary package
operations do not return checked source or hand it back to finish production.
Each returned package review also exposes borrowed access to its complete typed
`PackagePolicyBaseline`, projected from that same final checked source and
target. The compiler's `review_package_build` returns that evidence together with
an opaque `PackageReviewSource`; review cannot extract checked trees, Build
configuration or execution plans from it. Its source-free facts support reuse
and cache capture, and its component publication operation runs only on demand.
The compiler checks each projection against the caller's remaining policy budget
before nomination; the closure still owns aggregate accounting and decisions.
The closure retains at most 64 MiB in aggregate canonical policy
encoding size; this is not an exact heap-size limit. Legacy comparison rows,
commitments, and obligation checks remain separate and unchanged. This retained
finding is not accepted-lock storage or an additional authorization stage.
Binding nomination belongs to compiler-owned
[`package-evidence::capture`](../review-evidence/src/capture/semantic_bindings.rs),
which reads checked callable reach/invocation facts, entry requirements and
selected provider plans. Package review consumes its inert candidates, attaches
consumer context to projection errors, and scopes permission proposals; it no
longer walks checked machines, symbols, traits or provider plans, and has no
provider-planning, flow-effects or symbols dependency. The review exposes the
exact checked `ServiceSchema` beside each proposed binding. This is
non-authoritative review material: a consumer may use complete requirement
identities from that schema to author terminal-permission rows. Requirement-only
services do not gain authority from readable names. Hosted leaf proposals use
the compiler's fixed permission policy and remain subject to root approval.
The resulting binding must still survive the complete final recompilation and
ordinary root-policy admission. Hosted permission proposals are retained with
the candidate, but only the current product root carries them into policy;
dependency/build-host uses strip them even on a cache hit. Nomination does not
read rendered callable or provider review rows. Ordinary accepted compilation
consumes the resulting retained bindings and does not repeat nomination or review.

Install and update belong in `package_manager.rs`, with accepted baselines and decisions
recorded directly. Their command flow does not extend the promotion layer or
certify lock acceptance. The source and review crates remain subordinate.

`lock/decisions` records historical choices against the complete canonical
source subject. `HistoricalPackagePolicyDecisions::capture_policy` consumes the
complete-policy comparison and its exact resolved project choices. Version 2
retains the comparison identity, optional prior source association, and typed
row/root-role/source-replacement subjects. Removed-package decisions do not
index the candidate-only graph. Version 1 records remain readable in their
original indexed form; loading does not invent a modern comparison for them.
Recovery needs only the retained source subject, not an old checkout or
recreated compiler conflict. It preserves acceptance and rejection as trusted
project records, without making either fresh authorization or audit evidence.

`lock::PackageLock` composes that history with the immutable source graph and
one `PackagePolicyOccurrence` per source-package/purpose pair. Each contains
compact `PackagePolicyAcceptance` and required `AcceptedPackageCompilation`
with the accepted compiler identity and exact semantic binding roster. Exact target
sections are sorted by canonical target identity and must agree on the same
target-independent source graph. Each baseline matches its source package and
target; decisions match the complete target-bound source subject. These are
trusted project inputs whose use requires current source/compiler identity
matching, not a fresh review or an attestation.

Acceptance retains complete readable canonical rows for admission-claim and
external-realization callables, external supplies, dangerous capabilities, and
terminal permissions. It is not a recovered `PackagePolicyBaseline` or fresh
compiler evidence. The complete API, ordinary checked callables, provider tables,
representations, semantic dependencies, and symbolic demands remain in explicit
whole-candidate review and audit, not in the lock. Retained binding inputs include
only exact semantic role/owner/declaration, schema and selected-plan digests,
and explicit terminal permissions; they are not these full review tables.

Version 4 `omega.lock` text embeds canonical source, compact acceptance, required
accepted-compilation, and historical-decision sections using explicit byte lengths.
Older versions, including version 3, reject rather than inferring compiler identity
or bindings. The compact acceptance child uses policy row schema 3. No whole
child is replaced by an opaque escaped payload. Loading requires no checkout,
source acquisition, compiler run, proof certificate, or native replay. Recovery
checks outer framing and every child's canonical meaning and associations.
`PackageLockRecoveryLimits` bounds the entire input, requested owned storage
(including validation scratch), target sections, package rows, authored and
selected dependency requests, policy/binding/permission/class sequence elements, semantic identity
traversal nodes, and decisions. Child
usage is deducted from the same aggregate limits rather than reset per target.
Allocator overhead and already borrowed input are not part of owned-storage
accounting. Reviewed publication owns recoverable pair writes; install/update
commands retain all accepted targets when publishing.
Serialization uses the same recovery accounting one child at a time, discarding
that scratch before the next child. It therefore refuses an output that exceeds
the chosen recovery ceilings even when its text fits. This is a resource check,
not a second compiler review or a certificate that project choices are correct.

Commands treat a missing baseline as fresh graph review. Old source
is not needed to compare retained policy. Command source diffs use exact old
Git acquisition, verified cached local snapshots, matching current custody, or
the unchanged live root. Historical local lookup uses the current resolver-known
origin and accepted content pin, not a filesystem path decoded from the lock.
Missing or corrupt old bytes produce explicit standalone-candidate output.
This diagnostic recovery does not relax live-source checks for compilation.
Optional model wiring is separate.

`review::compare_package_policy_changes` compares an optional accepted target
with a freshly checked candidate and its exact current source closure. An absent
baseline is explicit initial review. Comparison joins the union of exact package
keys and complete canonical risk-bearing rows, including removed packages whose
old source no longer exists. Old and new resolutions and dependency paths remain
separate; a removed package has no invented candidate source or path. Same-named
packages from different source lineages do not merge.

Detailed capability comparison prepares one borrowed predecessor tree for its
candidate closure. Requested explanation paths retain shortest-path selection,
authored-edge tie order, purpose and dependency ordinal; only final paths copy
keys and aliases. A one-off path query still stops when its target is discovered.

Root role changes retain their directional compatibility finding: package to
application breaks dependency compatibility, while application to package
breaks application activation. Both require a decision for the same exact root.
Changing root identity requires a separate source-replacement decision.
Dependency replacements pair the exact requester key and resolved local alias,
then report different old/new selected keys. Each occurrence remains distinct,
including transitive and diamond uses; added/removed policy rows still apply.
These are source-selection decisions, not proof claims or new capabilities.

`source_replacements()` orders the root first, followed by requester/alias
bindings in canonical order. Full old/new keys and the complete comparison
context identify each finding. A revision change preserving the key is not a
replacement. Reordering declarations or renaming an alias while keeping its
package does not manufacture a replacement either. Different aliases remain
separate graph additions/removals. Changing both alias and source can use those
ordinary findings, with full review of the new package; no special paired-edit
command is required. Never pair unrelated packages by name or authored row
position. Replacement findings share the changed-row count
ceiling; their retained keys and binding scratch use the context-byte ceiling.

The report retains added, removed, and changed rows with full old/new readable
meaning, decision requirements, and audit recommendations. Unchanged candidate
packages remain visible for retained dangerous authority, external supplies,
and slack. Source changes also remain reviewable. Complete source subjects,
normalized policies, and fresh candidate compiler/source/build commitments bind
the versioned comparison fingerprints; historical policy is never adapted into
a fabricated compiler review or execution receipt. Both graphs share resource
ceilings rather than receiving a fresh budget per package.

Rebuildable API and representation changes alone require no approval. Risk-row
changes, source replacements, and root-role changes retain their exact decision
coverage. Source changes remain audit recommendations; neither comparison nor
historical lock recovery compiles old source.

`review::resolve_package_policy_decisions` consumes that report and the digest
retained with the project's choices. Each required row, root-role change, and
source replacement needs exactly one accept/reject choice. Removed-package rows do not
need the old checkout or a candidate-graph package index. Missing, duplicate,
unknown, advisory-only, and wrong-comparison choices reject. The result stores
choices in canonical subject order and preserves rejection; a comparison with
no required choices accepts an empty set without manufacturing approval work.
This uses the checked report directly, with no compiler reconstruction or
evidence-promotion step. It records decisions, not whether an audit happened.

`review::render_package_policy_review` places editable `pending` decisions beside
the complete comparison's readable before/after policy rows, source replacements,
and root-role changes. Source pins and dependency paths identify each side;
audit recommendations also remain visible when no choice is required. Package
prose never enters this document. Source identifiers are quoted data and policy
strings use the evidence codec's escaping.

`review::recover_package_policy_review` accepts only per-change `accept` or
`reject` edits. It regenerates the findings from the current comparison and
requires unchanged framing, identifiers, and displayed meaning before resolving
all choices. The versioned text uses LF and a caller-selected byte ceiling for
both rendering and recovery. This is ordinary resume consistency, not an audit
certificate, reviewer receipt, or authentication of the project author.

`all_required_changes_accepted` describes only the represented choices, not
permission to publish. Reviewed publication joins these choices to candidate
and project-file checks. Review-file loading/resume is command-owned.

Related contracts and usage:

- [Package workflow](../../../../wiki/language_guide/packages.md)
- [Build declarations and identity](../../../../wiki/spec/build/declarations.md)
- [Package acceptance](../../../../wiki/spec/packages/acceptance.md)
- [Ordinary libraries and semantic bindings](../../../../wiki/spec/packages/toolchain.md)
- [`TASKS.md`](../../../../TASKS.md)
