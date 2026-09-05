# Omega Package Manager

This is the operation-owning `package-manager` crate. Start at
[`src/lib.rs`](src/lib.rs), then enter the operation you are following.

The ratified destination is Cargo-like repository install/update with
compiler-derived reachability, unsafe API, and assumption review. The lock
records pins, the graph, accepted baselines, and decisions; the project trusts
whoever lands it. No sealed/certified `PackageInstance` or certificate of lock
acceptance is required. Source publication consumes checked review and exact
project decisions directly. Actual
compiler proof/reach checks and native artifact validation remain independent
of installation.

```text
manager/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs             Rust entrance
│   ├── operations/        complete package-aware operations
│   ├── admission/         separate compiler/native handoff checks
│   ├── declarations/      checked package declarations from build.omg
│   ├── resolution/        exact source selection and dependency closure
│   ├── lock/              persistent source-scoped project policy
│   └── review/            compile, compare, audit, and decide
└── tests/                 manager integration tests
```

`operations` is the only owner of complete user or compiler workflows.
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

`operations::review_package_change` checks an already-resolved candidate for an
exact target, rejects outstanding ordinary contract obligations in any package,
and compares its full policy against an optional accepted lock section. Missing
baseline means fresh review; unavailable old source is not needed for comparison.
The resulting `PackageChangeReview` exposes compiler findings and the ordinary
editable review document's input, without a reconstruction question, native
artifact, or evidence-promotion prerequisite.

After resolving the exact comparison, `propose_lock_target` requires all choices
to accept, rechecks retained source snapshots and selections, and constructs the
proposed target from complete candidate policy and direct decision history.
Rejected, foreign, or stale choices cannot produce a proposal through this
operation. The result remains an unwritten `PackageLockTarget`, not permission
to publish. `publish_reviewed_package_change` joins the staged edit and all
retained target reviews to exact current project bytes, then records commit
intent and publishes the pair under a project mutex. Interruption recovery
completes forward only from recorded old/new contents; unrelated edits stop it.
Ordinary project preparation coordinates with existing pending state before
snapshotting. See [publication](src/operations/publication/README.md) for the
file protocol and platform limits. The [command operation](src/operations/package_commands/README.md)
owns package/alias selection, persisted per-target findings, exact candidate
resume, and reviewed publication for `omega install` and `omega update`.

`prepare_local_project_for_target` selects an accepted lock target before source
acquisition. It preserves dependency pins, fetching only recorded commits when
needed, while allowing ordinary edits to the local application's source. Its
dependency projection, identity, and role must still match; local dependencies
must retain their accepted content. Declaration or dependency changes require
an explicit update. This mutable-root preparation is distinct from strict
whole-closure recovery. Local root identity remains tied to its canonical path.

Candidate checking executes the existing scoped build evaluator, so it is not
side-effect-free: package-input reads, disposable-output writes, and compiler
logging can occur even when later checking fails. Runtime boundary services are
rejected before authored build execution. Package runtime capability decisions
do not grant additional build or resolver authority. Accepted project files are
not modified by this operation.

Its package-aware native operation consumes a `PreparedLocalProject`, selects
one exact target child, compiles the final production review, reconstructs its
current initial conflict set, and recovers a caller-selected root-policy record
only against that set. Accepted evidence then enters the manager-owned retained
native route with a distinct receiving permission policy. Preparation retains
the source closure needed for this handoff but neither discovers policy nor
infers permissions. A root-policy file is required exactly when the fresh
review has blocking rows.
`declarations` reads the statically checked package declarations in
`build.omg`; Omega has no second package manifest. `resolution` binds those
declarations to immutable sources and reconciles one exact closure. `review`
turns that closure into compiler-issued facts and root-owned decisions. Its
`reconstruction/root_policy.rs` gate rederives fresh obligations and conflicts
from the same closure, requires exact conflict bijections for open accepted
claims, dangerous authorities, and external executable supplies, then accepts
blocking rows only through their exact candidate-bound root policy. Contract-
entailment obligations remain unadmittable while open; canonically recorded,
locally rechecked assumption discharges compose separately across the complete
closure with their original package owners. The result remains review and
policy state.
`admission` owns the stronger consumer-side
boundary: it rechecks live source custody and the complete reconstruction and
policy replay before producing in-memory accepted ordinary evidence. That
evidence retains exact compiler-consumed semantic bindings scoped to their
consuming package, while every resulting blocker still requires fresh root
policy. It still has no codec, `omega.lock` mutation route, or transaction
authority. Its promotion layer is current implementation, not a requirement to
add `PackageInstance` certification before implementing install/update.

`compile_resolved_package_candidate_reviews` is the install/update candidate
entrance. It uses one preliminary compiler review only to discover supported
package-owned semantic surfaces, then recompiles with exact consumer-scoped
bindings. Only that final review may proceed to conflicts and admission; the
discovery pass is neither policy nor evidence that an audit occurred.
Each returned package review also exposes borrowed access to its complete typed
`PackagePolicyBaseline`, projected from that same final checked source and
target. The closure retains at most 64 MiB in aggregate canonical policy
encoding size; this is not an exact heap-size limit. Legacy comparison rows,
commitments, and obligation checks remain separate and unchanged. This retained
finding is not accepted-lock storage or an additional authorization stage.
For a requirement-only service candidate, the preliminary review exposes the
exact checked `ServiceSchema` beside the proposed binding. This is
non-authoritative review material: a consumer may use complete requirement
identities from that schema to author terminal-permission rows, but neither the
candidate nor the manager assigns classes from service paths or method names.
The resulting binding must still survive the complete final recompilation and
ordinary root-policy admission.

Production-bearing operations use
`compile_resolved_package_candidate_for_production`. Its non-clonable result
exposes the same final review rows while retaining the exact checked
application root that produced them. Admission consumes that root directly
into unpublished Terminal/native production after fresh evidence comparison;
it does not rerun `build.omg` or recover generated source from staging.

Install and update belong in `operations/`, with accepted baselines and decisions
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
one complete typed `PackagePolicyBaseline` for every source package. Exact target
sections are sorted by canonical target identity and must agree on the same
target-independent source graph. Each baseline matches its source package and
target; decisions match the complete target-bound source subject. These are
inert project records, not fresh compiler authorization.

Evidence enumerates every concrete retained package owner, including owners
inside canonical type/callable identities; the manager requires membership in
the exact transitive source graph. Foreign symbolic boundary demands also join
the owning baseline's exact operator telescope. Neither check requires direct
dependency edges or public availability of arbitrary carried declarations.

The versioned `omega.lock` text embeds the canonical source, named policy, and
historical-decision sections verbatim, using explicit byte lengths. No whole
child is replaced by an opaque escaped payload. Loading requires no checkout,
source acquisition, compiler run, proof certificate, or native replay. Recovery
checks outer framing and every child's canonical meaning and associations.
`PackageLockRecoveryLimits` bounds the entire input, requested owned storage
(including validation scratch), target sections, package rows, authored and
selected dependency requests, policy sequence elements, semantic identity
traversal nodes, and decisions. Child
usage is deducted from the same aggregate limits rather than reset per target.
Allocator overhead and already borrowed input are not part of owned-storage
accounting. Reviewed publication owns recoverable pair writes; install/update
commands retain all accepted targets when publishing.
Serialization uses the same recovery accounting one child at a time, discarding
that scratch before the next child. It therefore refuses an output that exceeds
the chosen recovery ceilings even when its text fits. This is a resource check,
not a second compiler review or a certificate that project choices are correct.

`operations::recover_locked_sources` selects one recorded target before storage
verification or source acquisition. It accepts a typed caller root request and
requires its exact spelling to match the retained request; encoded lock path
bytes are never decoded into filesystem authority. It rebuilds the closure from
fresh source custody and declaration projections, matching each dependency by
requester and authored ordinal, then compares the complete canonical subject.
Git acquisition is offline by default. Explicit fetch permission acquires only
the recorded commit and root tree, including named workspace members, without
refreshing the authored selector. Local and workspace sources are recaptured
and must still match their recorded identity and content; a missing live local
source currently fails even if its snapshot remains cached.

Recovery borrows the accepted lock, so unavailable source or content drift does
not destroy the readable policy baseline. Its result is a source closure usable
by ordinary compiler inputs, not a fresh analysis or renewed acceptance of old
decisions. Commands treat a missing baseline as fresh graph review. Old source
is not needed to compare retained policy; source-code diff acquisition and
optional audit-service integration remain separate work.

`operations::check_locked_sources` follows exact recovery with fresh checking of
the complete graph through the ordinary candidate-review entrance. It preserves
required semantic-binding discovery/final checking and generated-source handoffs;
it does not reuse old compiler analysis or add another replay/promotion step.
The result keeps the borrowed accepted target beside the fresh source closure
and compiler review set. Full normalized policy comparison joins exact package
keys, immutable resolutions, and target, then reports changed package keys in
canonical source order. Same-spelled names do not merge packages. An unchanged
policy is equality with recorded project policy, not renewed certification or
permission to publish. This coarse change report is not the detailed row-level
conflict/decision transaction, and missing old source never triggers selector
refresh or an implicitly chosen candidate.

`review::compare_package_policy_changes` compares an optional accepted target
with a freshly checked candidate and its exact current source closure. An absent
baseline is explicit initial review. Comparison joins the union of exact package
keys and complete evidence-owned policy rows, including removed packages whose
old source no longer exists. Old and new resolutions and dependency paths remain
separate; a removed package has no invented candidate source or path. Same-named
packages from different source lineages do not merge.

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

Return to the [package subsystem map](../README.md), or consult:

- [Scope and workflow](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md)
