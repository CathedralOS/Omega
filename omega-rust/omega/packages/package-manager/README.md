# Omega Package Manager

Start at
[`src/package_manager.rs`](src/package_manager.rs): it recovers the project
transaction, admits install, update and resume requests, resolves the
candidate, and reconciles findings for publication. Application Build's
[`package_commands.rs`](../../build-system/orchestration/src/package/package_commands.rs) runs
the target-review loop. Compiler preparation and read-only inspection have
separate entrances in [`operations`](src/operations/mod.rs).

[Source selection](../../../../wiki/spec/packages/sources.md) owns the
requester-local graph and workspace-member contract. Acquisition enforces the
[resolver security boundary](../../../../wiki/spec/packages/resolver_security.md).

The ratified design is Cargo-like install and update from repositories, with
review of reachability, unsafe API and assumptions that the compiler derives.
The lock records pins, the graph, accepted policy, the compilation inputs of
each occurrence, and decisions; the project trusts whoever lands it. No sealed
or certified `PackageInstance` or certificate of lock acceptance is needed.
Publishing source uses the checked review and exact project decisions directly.
The compiler's proof and reach checks and native artifact validation stay
independent of installation.

```text
package-manager/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs             Rust module wiring and exports
│   ├── package_manager.rs command admission, decision join and publication
│   ├── operations/        compiler preparation, inspection, publication
│   ├── declarations/      checked package declarations from build.omg
│   ├── resolution/        exact source selection and dependency closure
│   ├── lock/              persistent source-scoped project policy
│   └── review/            admit review passes, join evidence, compare and decide
```

`PackageCommandTransaction` holds the project lock and the review-file root
while application Build runs the admitted target requests. Before it writes
decision documents or publishes accepted files, completion needs every target
from that admission. The original proposal handle stays borrowed throughout,
so completion checks the file admission read, not a replacement. Binding
discovery, exact occurrence reuse, policy comparison and cache admission live
here. Application Build runs the admitted dependency work through one
whole-pass executor that commands and audit share. Package policy does not
prepare Builds, publish components or finish compiler reviews.
Supporting packages are siblings under [packages/](../README.md).
The independent [declaration](../build-declarations/README.md),
[output](../build-output/README.md) and [compilation-input](../package-compilation/README.md)
packages sit below both the compiler and this crate, so the compiler does not
depend on install/update code. Source acquisition creates input custody, the
compiler records the exact source units it used, and review joins both to
package policy.

[`PackageInspectionTransaction`](src/operations/inspect_packages/README.md)
runs `omega packages audit`. It checks the current project source and pinned
dependencies, collects fresh compiler findings and compares them with accepted
policy, without publishing changes. The default summary is readable; `--details`
prints the complete normalized policy.
`operations::stage_build_dependency_edit` applies the declaration planner's
edit to a proposed source snapshot without writing the live `build.omg`. It
accepts only an automatic replacement and checks the planner's digest of the
old file. The staged local project resolver reads the proposed declaration but
keeps the original root path and context for package identity and relative
dependency lookup. The edit plan and original identity are kept through review
and publication. The pin-aware staged resolver takes `GitDependencyPins`
from the accepted source graph and exact selected package keys. Unchanged Git
locator/revision requests keep their recorded commit, tree, and content; an
empty update selection preserves all existing Git requests for installation.
Selected Git repositories refresh as a unit, including their reachable workspace
members and relative Path edges. Unrelated repositories, including unchanged
transitive requests, stay pinned. New or changed requests resolve normally.
A preserved pin whose content is missing either fails offline or fetches its
exact commit, never a fresh selector. This changes resolution, not accepted
policy.

`GitResolutionOptions::offline` forbids all fresh Git selection, including
new transitive requests, and overrides permission to fetch preserved pins. The
CLI's `--offline` applies it to install and update and reuses proposed pins on
resume. Missing old source bytes only limit code diffs, not lock policy
comparison; historical recovery also respects the network restriction.

`operations::review_package_change` checks an already-resolved candidate for
one exact target (Psi checking rejects broken contract obligations) and
compares its risk-bearing rows with an optional accepted lock section. No
baseline means a fresh review; old source is not needed for the comparison.
The resulting `PackageChangeReview` gives the compiler findings and the input
for the editable review document. It needs no reconstruction, native artifact
or evidence promotion.

Once the comparison is resolved, `propose_lock_target` requires every choice
to be accept, rechecks the retained source snapshots and selections, and builds
the proposed target from compact candidate acceptance rows and the decision
history. It keeps each review's exact semantic bindings and compiler input
identity, captured before checking and rechecked after it and before the
proposal. The identity combines the compiler's embedded build-input digest
with the selected toolchain library content; it catches stale inputs and does
not attest to a compiler run or audit. Rejected, foreign or stale choices
produce no proposal. The result is an unwritten `PackageLockTarget`, not
permission to publish. `publish_reviewed_package_change` joins the staged edit and all
retained target reviews to exact current project bytes, then records commit
intent and publishes the pair under a project mutex. After an interruption,
recovery only completes the write, from the recorded old and new contents;
unrelated edits stop it. Ordinary project preparation settles pending state
before taking a snapshot. See [publication](src/operations/publication/README.md) for the
file protocol and platform limits. The [command operation](#install-and-update)
owns package/alias selection, persisted per-target findings, exact candidate
resume, and reviewed publication for `omega packages install` and `omega packages update`.

`begin_local_project_preparation` opens recovery and snapshot policy before
the application resolves the source graph. Accepted discovery keeps dependency
pins and fetches only recorded commits when needed. Accepted compilation needs
exact source identities for every occurrence, including the local root, plus
the dependency projections, identities, roles and selected edges. After a
source edit, even to the root, `omega packages update` must review it before
ordinary accepted compilation. Local package identity stays tied to its
canonical path.

The application's `BuildRequest` accepts `offline` for one invocation. Cached
accepted Git pins and unlocked local-only graphs stay usable; missing recorded
Git content fails without network access. Offline mode keeps source and
compiler identity matching and the lock format unchanged.

Per the owner ruling of 2026-10-03, ordinary checks and native builds use
accepted compilation, not candidate review. The application's
[`orchestration`](../../build-system/orchestration/README.md) calls the compiler
for them, including inspection and SDK consumers. Package management's
`admit_project_compilation` returns ordered requests and a completion owner that
holds staging, total sponsors and source custody. Completion maps occurrence
failures back to packages, checks compiler-library identity and reports
disposal failures. Neither step runs the compiler.
Every accepted occurrence's source and compiler input identities must match its
lock record before Build execution. A match supplies the exact accepted semantic
bindings for that occurrence; a mismatch rejects with the package name and
`omega packages update`. Neither route performs an inline audit or rewrites acceptance.
Dependency product checking follows the consumer's `use` items and their
transitive module closure rather than compiling each dependency's package root.
The requested root source entry, scoped Build execution, generated-source custody,
ordinary trust settlement, and requested compiler observations remain intact.
Reporting uses the retained checked root; it does not run Build again.

Checking and native compilation without a lock are allowed, including
`omega run`, but create no accepted state and supply no accepted bindings,
restricted-build grants or root terminal permissions
(`admit_accepted_contexts` passes no terminal permissions when there is no
lock). Malformed and older locks are rejected, not treated as missing. Full-package discovery and bound review happen only in an explicit
review, acceptance or update operation.

Candidate checking runs the scoped build evaluator, so it has side effects:
it can read package inputs, write disposable outputs and log, even when later
checking fails. Runtime boundary services are
rejected before authored build execution. Package runtime capability decisions
do not grant additional build or resolver authority. Accepted project files are
not modified by this operation.

Compilation checks opaque declaration visibility, selected conformance
validity and by-value representation demands without reading a review
projection. The consumer's Build selects the representation for the
combined source compilation; a dependency's historical root-build choice is
not a second selection. Independently compiled artifact composition is a
different boundary and still requires representation agreement.

A package-aware native build takes a `PreparedLocalProject` with the source
closure and the accepted `omega.lock` section for one exact target. Without
acceptance it fails before Build. When source and compiler identities match,
it uses the kept occurrence bindings with no fresh review or second approval
file; a mismatch tells the user to run `omega packages update`. Native
compilation never writes acceptance. Source-only changes remain visible
in explicit package inspection and review. Retained permissions still constrain
production, while receiving permission policy remains a separate input.

The CLI update/resume regression test reaches the independent native
publication gate. Executable publication is still blocked: nothing produces
the function-validation evidence tracked under
[`TARGET-MATRICES`](../../../../TASKS_OPTIMIZER.md#verification-and-rollout).
Native artifact validation does not mean publication succeeded.

`declarations` reads the statically checked package declarations in
`build.omg`; Omega has no second package manifest. `resolution` binds them to
immutable sources and settles one exact closure. `review` turns that closure
into compiler-issued facts and decisions owned by the root, with no separate
initial-conflict comparison or second approval. Psi's call and normal-exit
checks, including returns through tail calls, judge source contracts; the
result is review and policy state.

Ordinary accepted compilation consumes accepted package inputs without
repeating review: the driver reads the lock's Terminal permission rows
directly. Build-host permissions never project into the product's native
authority, even when both roles use the same target. Changed requirements need
ordinary project review.

Only explicit artifact admission gives a receiving-policy verdict; ordinary
package and native builds do not. Without a receiver policy, nothing is
claimed about receiver admission: production is not blocked and no execution
grant is invented. Install/update must
separately surface and accept restricted build requests before execution, retain decisions in the lock,
and require actual host grants under
[restricted-build acceptance](../../../../wiki/spec/packages/acceptance.md#restricted-build-acceptance).

Reading captured source and named inputs, with bounded private output
staging, needs no restricted-action decision. The filesystem sponsor owns that
fresh staging session and its cleanup; generic sponsors for supplied
directories get no such exemption. Broader host scopes still raise restricted
requests, and what the lock records for them differs from the grants an
invocation actually receives.

The top-level invocation owns one `PackageBuildSession` across its review
targets and rounds. Application Build's
[`package_inspection.rs`](../../build-system/orchestration/src/package/package_inspection.rs)
creates and disposes the audit's owner and executes its ordered target roster.
The CLI's
[`packages::run`](../../application/src/commands/packages.rs) retains the
install/update owner across automatic acceptance and resume rounds, lending it
to `orchestration::execute_package_command`. A later user-issued command creates its own
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
Completed application and dependency reviews can be reused across commands
when every recorded input still matches. The source resolver keeps its cache
locations on the acquired root as well as dependencies. The existing full key
and current source-consumption check govern reuse; ordinary decisions and lock
publication still run. [The Cathedral measurement](../../../../wiki/drafts/measurements/root_review_reuse_2026_11_04.md)
records the unchanged-resume result and input-change controls.

If a persisted dependency review lacks a demanded component description, refresh
that dependency while retaining completed sibling reviews and the consumer's
Build continuation. Reuse still compares complete inputs, entry and snapshot;
changed generated inputs require fresh work. Interrupted review sets are private
progress, not an accepted root-wide review.
Its [`passes_occurrences.rs`](src/review/candidate/passes_occurrences.rs) admits each
build/product occurrence and joins compiler evidence, using the same roster as
lock comparison and reconstruction. Application Build's
[`package_review.rs`](../../build-system/orchestration/src/package/package_review.rs) owns the
dependency loop, compiler calls and cache-miss recovery. Acquisition and parsed
source are shared; checked targets,
generated bundles, bindings, component descriptions and reviews are not.
Build session and staging ownership are shared through `build_session`; review
policy and ledger budgeting remain with the review owner.
[`review_set.rs`](src/review/candidate/review_set.rs) describes the issued output.

`review_resolved_packages` is the candidate review entry point for install,
update and audit. Both discover bindings before the final bound pass;
observational review shows restricted requests but never grants them. Accepted
production builds admit grants separately through `accepted_compilation`.
Each package uses its authored `main.omg` if it has one, otherwise its
authored `build.omg` through the compiler's build-file entry. This creates no
source exports: missing imported modules are still rejected. Explicit root
check entries and native application entry requirements are unchanged.

A preliminary compiler review only discovers the package's supported
semantic surfaces; the package is then recompiled with exact consumer-scoped
bindings. Only that final review goes on to policy comparison and admission;
the discovery pass is neither policy nor evidence that an audit happened. The
passes share one compiler session, which keeps prepared source, completed Pure
Builds and exact language judgments. Package review owns source bindings,
dependency order, occurrence consent and sponsor accounts, not compiler
caches. Each activation still validates custody, selections, generated inputs
and grants. A matching earlier judgment lends checked storage only after fresh
admission. `PackageSourceBindings` records resolved roots by package position;
it is not a frontend checkpoint table. The command's `PackageBuildSession`
lasts across review rounds and targets, so sponsors are not refilled per
target.

Ordinary accepted compilation builds a dependency-ordered request from those
inputs. Application Build schedules dependencies and joins generated outputs;
the compiler keeps opaque Build continuations and publishes the independent
components that are needed. Review also asks the compiler for component
descriptions, so this crate has no backend-publication dependency. Check,
native and inspection requests name their finished product up front. The
compiler owns root permission comparison and completion; package operations do
not return checked source or hand it back to finish production.

Each package review also lends its complete typed `PackagePolicyBaseline`,
taken from the same final checked source and target. The compiler's
`review_package_build` returns that evidence with an opaque
`PackageReviewSource`, from which review cannot extract a checked program,
Build configuration or execution plan. Its source-free facts support reuse
and cache capture, and its component publication runs only on demand. The
compiler checks each projection against the caller's remaining policy budget
before nomination; the closure still owns total accounting and decisions. The
closure keeps at most 64 MiB of canonical policy encoding in total; this is
not an exact heap limit. Legacy comparison rows, commitments, and obligation
checks remain separate and unchanged. This finding is not accepted-lock
storage or an extra authorization stage.

Binding nomination belongs to the compiler's
[`review-evidence::capture`](../review-evidence/src/capture/semantic_bindings.rs),
which reads checked callable reach and invocation facts, entry requirements and
selected provider plans. Package review takes its inert candidates, adds
consumer context to projection errors, and scopes permission proposals; it does
not walk checked machines, symbols, traits or provider plans, and has no
provider-planning, flow-effects or symbols dependency. The review shows the
exact checked `ServiceSchema` beside each proposed binding. This material
grants nothing: a consumer may use the schema's complete requirement
identities to write terminal-permission rows. Requirement-only services gain no
authority from readable names. Hosted leaf proposals use the compiler's fixed
permission policy and still need root approval. The binding must survive the
complete final recompilation and ordinary root-policy admission. Hosted
permission proposals are kept with the candidate, but only the current product
root carries them into policy; dependency and build-host uses drop them, even
on a cache hit. Nomination does not read rendered callable or provider review
rows. Ordinary accepted compilation uses the kept bindings and does not repeat
nomination or review.

Install and update live in `package_manager.rs`, which records accepted
baselines and decisions directly and certifies nothing; the source and review
modules serve it.

`lock/decisions` records past choices against the complete canonical source
subject. `HistoricalPackagePolicyDecisions::capture_policy` takes the
complete-policy comparison and its exact resolved project choices. Version 2
keeps the comparison identity, an optional link to the prior source, and typed
row, root-role and source-replacement subjects. Decisions about removed
packages do not index the candidate-only graph. Version 1 records stay
readable in their original indexed form; loading does not invent a modern
comparison for them. Recovery needs only the kept source subject, not an old
checkout or a recreated compiler conflict. Acceptances and rejections stay
trusted project records, not fresh authorization or audit evidence.

`lock::PackageLock` composes that history with the immutable source graph and
one `PackagePolicyOccurrence` per source-package/purpose pair. Each contains
compact `PackagePolicyAcceptance` and required `AcceptedPackageCompilation`
with the accepted compiler identity and exact semantic binding roster. Exact target
sections are sorted by canonical target identity and must agree on the same
target-independent source graph. Each baseline matches its source package and
target; decisions match the complete target-bound source subject. They are
trusted project inputs: using them requires matching current source and
compiler identities, not a fresh review or attestation.

Acceptance retains complete readable canonical rows for admission-claim and
external-realization callables, external supplies, dangerous capabilities, and
terminal permissions. It is not a recovered `PackagePolicyBaseline` or fresh
compiler evidence. The complete API, ordinary checked callables, provider tables,
representations, semantic dependencies, and symbolic demands remain in explicit
whole-candidate review and audit, not in the lock. Retained binding inputs include
only exact semantic role/owner/declaration, schema and selected-plan digests,
and explicit terminal permissions; they are not these full review tables.

Version 4 `omega.lock` text holds canonical source, compact acceptance,
required accepted-compilation and historical-decision sections, each with an
explicit byte length. Older versions, including version 3, are rejected rather
than guessing compiler identity or bindings. The compact acceptance section
uses policy row schema 3. No section is stored as an opaque escaped payload. Loading requires no checkout,
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
Serialization applies the same recovery accounting one section at a time,
freeing scratch before the next, so it refuses output that exceeds the
recovery limits even when the text fits. This is a resource check, not a
second compiler review or proof that project choices are correct.

Command source diffs get old source from
the exact old Git commit, verified cached local snapshots, matching current
custody or the unchanged live root. Historical local lookup uses the current resolver-known
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

Detailed capability comparison builds one borrowed predecessor tree for the
candidate closure. Explanation paths keep shortest-path selection, ties broken
by authored edge order, purpose and dependency ordinal; only final paths copy
keys and aliases. A one-off path query stops when it finds its target.

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

The report lists added, removed and changed rows with full readable old and
new meaning, required decisions and audit recommendations. Unchanged
candidate packages stay visible for kept dangerous authority, external
supplies and slack, and source changes stay reviewable. Complete source
subjects, normalized policies and fresh candidate compiler, source and build
commitments feed the versioned comparison fingerprints; old policy is never
turned into an invented compiler review or execution receipt. Both graphs
share one set of resource limits, not a fresh budget per package.

API and representation changes that only need a rebuild need no approval.
Risk-row changes, source replacements and root-role changes each need their
exact decision. Source changes are audit recommendations; neither comparison
nor lock recovery compiles old source.

`review::resolve_package_policy_decisions` takes that report and the digest
kept with the project's choices. Each required row, root-role change and
source replacement needs exactly one accept or reject choice. Rows for removed
packages need neither the old checkout nor a candidate-graph package index.
Missing, duplicate, unknown, advisory-only and wrong-comparison choices are
rejected. The result stores choices in canonical subject order and keeps
rejections; a comparison with no required choices accepts an empty set. It
uses the checked report directly, with no compiler reconstruction, and records
decisions, not whether an audit happened.

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

`all_required_changes_accepted` reports only on the recorded choices; it is
not permission to publish. Reviewed publication combines the choices with
candidate and project-file checks. The commands load review files and resume.

Related contracts and usage:

- [Package workflow](../../../../wiki/language_guide/packages.md)
- [Build declarations and identity](../../../../wiki/spec/build/declarations.md)
- [Package acceptance](../../../../wiki/spec/packages/acceptance.md)
- [Ordinary libraries and semantic bindings](../../../../wiki/spec/packages/toolchain.md)
- [`TASKS.md`](../../../../TASKS.md)

## Install and update

Enter through [`../package_manager.rs`](src/package_manager.rs), which also
declares the commands and outcomes. The CLI at
`../../application/src/arguments/packages.rs` parses arguments; this operation owns resolution,
checking, review, and publication.

```text
src/
├── package_manager.rs         command admission, decision join and publication
├── planning.rs                dependency edits and exact update selections
├── change_decisions.rs        compiler findings rendered and the per-change decisions loaded
├── source_review.rs           separate bounded source-code diagnostics
├── source_review_old_sources.rs  exact old-source recovery through custody issuers
├── state.rs                   retained proposal and review files
├── proposal.rs                restart record
└── proposal_framing.rs        bounded framing and codec tests
```

```text
omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]
omega packages update [package-or-alias...] [--to <revision>]
omega packages install --resume
omega packages update --resume
omega <install|update> --discard-review
```

Both commands accept `--project <dir>` (default current directory). New commands
accept repeated `--target <name>`; they retain every existing lock target and
add requested ones. First acceptance defaults to the host when none is named.
The project must already have a valid `build.omg`.

New commands also accept repeated `--build-input <path>` and
`--optional-build-input <path>`, using the same root-relative immutable inventory
as compilation and package audit. Include all root source files, including
`build.omg`; directories include their admitted subtrees. Only the root's view
is selected: dependency builds keep their own inventories. Missing required
inputs fail before the affected build executes. Neither option grants host
access or accepts policy.

A pending review keeps that selection for every target and both review
passes. `--resume` uses it, rechecks source custody and compiles fresh before
applying decisions; it never widens the view. Resume
and discard reject input flags. To change inputs or their bytes, discard the
proposal and start a fresh command. Proposal format 2 records this intent;
older pending proposals must be discarded and recreated. Existing accepted locks
are unchanged by this proposal-format revision.

`--offline` restricts repository acquisition for this invocation. Local source
work and cached accepted/proposed Git pins remain usable; missing recorded
content fails without fetching or changing accepted files. New or explicitly
refreshed Git requests reject even if a moving selector was cached earlier.
In particular, `omega packages update --offline` does not silently preserve the Git
repositories it was asked to update. Resolve an update online first, then use
`omega packages update --resume --offline` to finish its review from cached proposed pins.
The same applies to install/resume, including named workspace packages.

This option also prevents old-source diff recovery from fetching. Missing old
bytes produce standalone candidate diagnostics; accepted lock policy still
supplies the capability comparison. Offline mode does not disable compiler
checking, scoped build outputs, review decisions, or publication recovery.
It is not a sandbox for a program later executed by the user.

Install supports Git HTTPS/SSH and local sources.
The fetched `build.omg` supplies the package name and default import alias;
`--as` is an optional local override. Git defaults to `HEAD` when `--rev` is
omitted. `--package <declared-name>` selects one Git workspace member by its own
package declaration. The resolver discovers the member inside the verified
workspace; no member path is accepted. Omission keeps root selection, and local
sources use their package directory directly. The member's declared name still
supplies its default alias; `--as` overrides only that local alias. Unknown or
duplicate declared names reject before publication. For example:

```text
omega packages install https://example.org/team/libraries.git --package exact-math
omega packages install https://example.org/team/libraries.git --package exact-math --as math
```

Selection stays in the proposed declaration and source graph during review.
Resume uses it without repeating `--package`. Automatic additions stay in the
direct build entry, before any transfer or nested states. Ambiguous inline
placement and other complex declaration layouts receive a manual-patch diagnostic
rather than an unsafe automatic rewrite.

Update without selections refreshes the graph. Selections resolve a root alias
first, otherwise a unique package name. Selected Git repository members move
together; unrelated repositories keep exact pins. `--to` requires one selection
with a root-authored Git request. A missing lock requires an unselected update
and fresh graph review; unsupported lock formats reject before acquisition.
Existing review files do not supply a missing lock baseline or automatically
approve the fresh review. Dangerous dependencies still require their exact
decisions before a new lock is published.

Blocking findings return exit status 3 and leave accepted project files intact.
In the reported review files, change each `pending` decision to `accept` or
`reject` (nothing else is editable), then run the matching command with
`--resume`. A rejection keeps the proposal unpublished. Findings hold the
compiler's complete policy changes and audit recommendations, not package
prose or proof that anyone audited the source. Nonblocking candidates publish
directly; retained dangerous authority still recommends an audit on upgrades.

Compiler failures return status 1 with the package, failed phase, and diagnostic
reasons on stderr. Diagnostics use escaped text, not editable review records.
Output shows at most 16 diagnostics and 4 KiB of input bytes per message;
package names abbreviate after 256 bytes. Every omission is explicit. Available
source-unit byte spans are retained; filenames already present in messages are
preserved, but the error API has no source map for reconstructing missing paths
or line numbers. Rejection does not publish dependency or lock changes.

Ignored `build/package-manager/` contains:

- `proposal`: candidate pins, proposed build bytes, targets, root input selection,
  and original project-file/source identities needed to resume.
- `review-<target>.txt`: compiler-rendered findings and editable decisions.
- `source-diff.txt`: separate, escaped source-code patches, regenerated on resume.
  Its contents are hostile source data and cannot supply project decisions.
- `check-<target>/`: disposable compiler/build outputs.
- `transaction.lock` and `pending`: the separate publication mutex and
  commit-intent journal, owned by [publication](src/operations/publication/README.md).

Resume fetches only the proposed exact pins, when needed and not offline,
recompiles, and rejects any change in source, graph, accepted files or
findings. Source diagnostics recover recorded
Git commits and named members without refreshing selectors. Changed local
sources use verified historical snapshots from the existing cache; lookup binds
the known local origin to the accepted content pin. Unchanged candidate custody
or an exact live local root can also supply the baseline. Missing or corrupt old
snapshots receive standalone candidate output while policy comparison continues.
The report names unavailable old source, binary content,
and rendering limits explicitly. Source patches share an 8 MiB output ceiling
and the renderer's independent per-package limits; they never enter capability
review or decision recovery.

`--discard-review` removes the proposal, leaving review files as diagnostics.
It does not roll back accepted files or discard publication recovery. Every
operation recovers pending commit intent before using the accepted pair.

The application coordinator submits dependency requests as Build execution emits
them; package management does not run or scan Build source to discover a graph.
It resolves and reviews each immutable request before an implementation can be
installed as a typed Build binding. The graph becomes complete only when every
Build has returned and all outstanding requests join. A missing transitive
package may therefore reject after earlier benign Build observations or
disposable staging writes, but no failed graph publishes a result.
Runtime boundary services and resolver credentials are not supplied to builds.
Source and output paths stay relative to their compiler-supplied roots; absolute
paths and parent traversal reject. Publication checks build-input content even
when changing an input would leave the generated source and policy unchanged.
Generated machines may combine retained service identities into new normalized
reach rows while preserving every existing row ID and meaning. They receive
ordinary final effect checking and capability review. Broader generated
declaration shapes remain subject to the compiler's supported continuation;
accepting a review decision cannot enable an unsupported compiler case.
Separate compiler/native admission policy remains outside this transaction.
