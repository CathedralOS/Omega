# Omega Package Manager

This is the operation-owning `omega-package-manager` crate. Start at
[`src/lib.rs`](src/lib.rs), then enter the operation you are following.

The ratified destination is Cargo-like repository install/update with
compiler-derived reachability, unsafe API, and assumption review. The lock
records pins, the graph, accepted baselines, and decisions; the project trusts
whoever lands it. No sealed/certified `PackageInstance` or certificate of lock
acceptance is required. The implementation described below still contains
redundant evidence-promotion and policy-replay machinery to simplify. Actual
compiler proof/reach checks and native artifact validation remain independent
of installation.

```text
manager/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs             Rust entrance
│   ├── operations/        complete package-aware operations
│   ├── admission/         current evidence promotion and native handoff gates
│   ├── declarations/      checked package declarations from build.omg
│   ├── resolution/        exact source selection and dependency closure
│   ├── lock/              persistent source-scoped project policy
│   └── review/            compile, compare, audit, and decide
└── tests/                 manager integration tests
```

`operations` is the only owner of complete user or compiler workflows.
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

Install and update belong in `operations/`. Their remaining work is the basic
resolve/fetch/review workflow and transactional lock/cache updates, with
accepted baselines and decisions recorded directly. Extending the promotion
layer or certifying lock acceptance is not a prerequisite. The source and
review crates remain subordinate to those operations.

`lock/decisions` records historical choices against the complete canonical
source subject. Its bounded ASCII section refers to that subject's sorted
package entries and binds the exact root, role, graph, and target. Recovery
needs only the retained source subject, not the old checkout or a recreated
compiler conflict. It preserves both acceptance and rejection without making
either a fresh authorization. Fresh capture still requires the complete exact
current resolution.

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
accounting. Atomic install/update publication remains open.
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
decisions. Missing-baseline handling, standalone candidate audit, and
transactional publication remain separate integration work.

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
separate graph additions/removals: a command changing both alias and source
must retain that explicit intent, rather than pair unrelated packages by name
or authored row position. Replacement findings share the changed-row count
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

`all_required_changes_accepted` describes only the represented choices, not
permission to publish. Explicit command intent, decision text/file handling,
removed-package history in the lock, fresh compiler obligations, and
transactional candidate/project-file rechecks remain integration work.

Return to the [package subsystem map](../README.md), or consult:

- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md)
