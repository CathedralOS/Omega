# Tasks: Package Manager

Status: corrected implementation plan, 2026-08-26.

This file tracks the Cargo-like source/package service under `omega`. The
governing design is:

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`

## Trust status

The release surface now contains reviewed corrected-model building blocks for
source custody, typed identity/closure, compiler handoff/review, row conflicts,
and triage. The legacy manifest, name-keyed lock, whole-section receipt, and
install/update scaffolding has been deleted. No caller-authored
manifest JSON, package name, alias, free-form review receipt, or synthetic
security artifact may re-enter a production trust path.

Do not wire mutating `omega install` or `omega update` until every P0 task is
complete.

## Settled model

- A fetched package declares its identity through the ordinary build surface in
  its own `build.omg`: `builder.package("name")`. The retired `const PACKAGE:
  Package` literal and its bespoke shape-matching parser are superseded
  (settled 2026-08-25). Every `build.omg` states its kind explicitly —
  `builder.package`, `builder.member`, `builder.application` — and no role is
  ever inferred from an absent declaration.
- A workspace root lists members by **path**, so relocating a subtree is a
  one-line manifest edit rather than a repository-wide rewrite.
- `SourceIdentity { kind, locator, resolved }` keeps `kind` open. Git is one
  supported kind, not the blessed model. There is no package version field:
  identity is the package name plus the resolved source revision, and a
  moving locator (branch) resolves once and is pinned in the lock.
- One `omega.lock` lives at the workspace root. A dependency's lock is never
  read by its consumers — the lock belongs to whoever builds an artifact, and a
  library does not pin its consumers' graph. What composes upward is the
  dependency's manifest and disclosed admissions, which are separate files.
- `omega::language::core` stays bundled with the compiler by decision, not by
  omission: it is the language, its version is the language version, and two
  versions of it can never coexist in one graph. `omega::language::std` is an
  ordinary fetchable package.
- `PackageName` is human identity; `PackageKey` adds canonical source lineage;
  `PackageInstance` adds exact source and artifact subjects, per-subject
  obligation-semantics identity, re-derived discharge results, and transitive
  open assumptions. Compiler/toolchain identity is review metadata, not a seal.
- Normal dependency rows name only a source request. Default aliases are
  derived from the fetched package name; explicit aliases are exceptional.
- Dependency-source projection is hermetic and completes before dependency
  build execution.
- `build.omg` records update intent. `omega.lock` records exact reconciliation,
  the normalized accepted capability/API baseline, and representation-TCB
  review rows.
- Capabilities are compiler-derived from checked candidate source/build output.
- Ordinary admission uses a total internal `PackageAdmissionProjection` from
  coherent compiler-owned semantic state after successful checking. Its rows
  may draw from different private representations. The lock stores only
  versioned canonical evidence; raw IR and compiler-private identities never
  become lock format.
- Checker placement (Q5, ratified 2026-08-26): the compiler admission
  projection may read each evidence fact from the
  earliest coherent compiler-owned representation in which that fact is
  semantically settled. This may be private pre-Psi structure or Psi state;
  structural identity may be joined to later checked acceptance only after the
  compilation succeeds. The final projection is total, but no one internal
  stage must contain every row. Do not introduce nominal Chi merely to collect
  or stabilize compiler internals across versions. Psi may repeat an invariant
  as a downstream backstop without becoming the required source from which the
  package checker reconstructs an already-settled fact.
- Proposition and named-evidence rows join the typed structural application to
  its checked acceptance, witness interface, and admission disposition. Never
  use checked diagnostic renderings as identity. If an exact binder, argument,
  or witness coordinate is not retained structurally, extend the existing
  typed/checked carrier that owns it rather than adding a report-only IR stage.
- Ordinary compiler-internal package-qualified type identities distinguish an
  exact package, toolchain source, or unresolved owner. Exact package-review
  projection rejects unresolved ownership; generic binders are alpha-normalized
  in their separate owner-free lane.
- Numbered fields and retired identities on ordinary public `data` are the wire
  contract. The retired standalone `wire data` representation is not a second
  package API surface.
- Complete Terminal coverage is not an ordinary admission prerequisite.
  Terminal evidence is a separate class required for final-realization claims
  and hardened profiles. No partial/completeness bit may imply a Terminal claim.
- Install compares against an empty baseline. Missing lock evidence causes
  fresh graph admission. Missing old source causes standalone source audit but
  does not erase a valid lock baseline.
- Every update receives source/provenance triage. Capability/API changes block;
  retained dangerous authority always recommends code audit.
- Conflict resolution is row-specific and candidate-bound, never a blanket
  yes/no receipt.
- Package review is regenerated by the selected local compiler so dependency
  source cannot publish its own capability result. It remains permanently
  review-only. Package evidence is accepted only after local reconstruction
  from exact source and artifact subjects and checking of exact certificates.
  Compiler/toolchain identity remains provenance for reproduction, cache
  partitioning, and incident response, not proof that the producer is honest.
- Evidence composes transitively per subject. Each subject retains its own
  obligation-semantics/schema version and certificate provenance; open
  obligations propagate upward and each consumer independently admits or
  rejects them. Checked schema deltas may reuse only explicitly unchanged
  classes. Verification, admissions, and provenance render separately.
- Audit recommendations and LLM triage are advisory. Resolution records express
  root-project policy decisions; the accepted commit and the organization or
  infrastructure controlling it are the authority. Stronger reviewer, quorum,
  signature, isolation, bootstrap, and reproducibility requirements remain
  deployment policy rather than a portable Omega “proof of audit.”
- Package review takes a caller-supplied workspace. Orchestration creates a
  fresh disposable child session beneath it, keeps resolver snapshots
  immutable, and publishes results only after the child is cleaned up
  successfully. Ordinary standalone compiler build roots remain caller-owned.
- The one `Build` activation exposes an immutable package-source root and a
  fresh writable staging root as ephemeral capabilities. Checked resolution
  binds canonical relative `Path` bytes to one exact root occurrence; virtual
  `/source` and `/output` spellings are evidence serialization only. Generated
  content enters compilation only through explicit successful handoff.
- Claim-free opaque boundary data always emits package-qualified
  representation-TCB evidence. Introduction or material change recommends
  code/ABI audit but is not, by opacity alone, a blocking trust claim; exact
  dangerous mechanisms, accepted claims, authority, executable supply, and API
  incompatibility retain their independent blocking policies.
- Implementation vocabulary is discovery-driven: reuse ordinary Omega data,
  machines, arithmetic, and existing provider machinery; do not add a public
  boundary trait or package-specific policy axis unless a concrete fixture
  exposes an irreducible contract that needs it.

## P0 — Replace invalid foundations

- [x] **PACKAGE-SCAFFOLDING-AUDIT.** Review every production path in
  `omega-packages` and classify it as retain, rewrite, or delete.

  Known suspect evidence points include unrestricted string identities,
  section fingerprints produced from Rust `Debug` output, coarse section-level
  rather than checked-row diffs, capability-flow counts without provenance,
  and lock trust-receipt identifiers without retained evidence.

  Acceptance: the review covers source/cache process isolation, identity,
  manifests, locks, install/update plans, graph audit, review receipts, CLI
  exposure, persistence, and test provenance. No retained API accepts an
  unverified caller-constructed security artifact.

  Completed 2026-08-23: the file-by-file and trust-path classification is in
  [`omega-packages/SCAFFOLDING_AUDIT.md`](source/omega-rust/omega/orchestration/omega-packages/SCAFFOLDING_AUDIT.md).

  Cleanup 2026-08-25: the name-keyed manifest/lock, Rust-`Debug` diff,
  free-form receipt, fabricated install/update, legacy audit, and test-only
  command modules were deleted after a consumer audit proved the corrected
  production path no longer referenced them. Typed graph closure validation,
  exact-row conflicts, fixed-vocabulary triage, source diagnostics, and the
  strict internal JSON reader remain with their corrected consumers.

- [x] **QUARANTINE-PROTOTYPE-CLI.** Prevent the existing manifest-file,
  receipt-file, lock-assembly, and install/update-plan commands from being
  mistaken for package admission.

  Acceptance: help/output labels them diagnostic and untrusted, or the commands
  are removed until locally regenerated compiler evidence exists. No command
  can write an accepted production lock from standalone JSON manifests.

  Completed 2026-08-27: manifest-based package audit, plan, review, and lock
  commands and their rejecting name gates are absent from the production CLI.
  Source-audit commands remain separately marked as unhardened until
  `HARDEN-SOURCE-RESOLVER` closes.

- **HARDEN-SOURCE-RESOLVER.** Re-audit the current Git/local resolver as a
  hostile-input boundary.

  The production helper/snapshot/receipt contract is recorded in
  [`SOURCE_RESOLVER_SECURITY.md`](source/omega-rust/omega/orchestration/omega-packages/SOURCE_RESOLVER_SECURITY.md).

  Progress 2026-08-23: diagnostic source commands now require an explicit
  `local` or `git` adapter; unknown URLs are no longer guessed to be Git. Local
  source identity now uses injective versioned framing over raw relative-path
  bytes, entry kind, directory presence, symlink target spelling, Unix
  executable mode, length, and content, rejects special files and links into
  excluded Git metadata, and checks entry limits before allocation. Directory
  permissions normalize to the read-only snapshot policy rather than preserving
  irrelevant host-checkout state. Git caches now use full policy-versioned
  keys, exclusive per-entry locking, staged publication, exact resolver
  metadata and canonical local configuration verification, sealed Git
  execution, and
  pre-materialization rejection of `.gitmodules` and gitlinks. Git source is
  now read from a parent-authenticated selected object graph. The resolver
  requires an exact requested object ID to equal the selected commit, recomputes
  the raw commit and every blob object ID, collision-checks SHA-1, checks the
  commit's root tree edge, retains every explicit child-tree edge, reconstructs
  all canonical tree objects including empty trees, and compares the resulting
  Merkle root before any snapshot stage can exist. SHA-1 and SHA-256 exact
  revisions are covered through fixed vectors and real repositories.
  Materialization preserves explicit empty directories and then proceeds
  without checkout, filters, hooks, or submodules; the snapshot is re-hashed,
  made read-only, atomically published, and revalidated before reuse. The Git
  parent is now selected only
  from a closed platform list of absolute concrete paths, never ambient `PATH`;
  macOS excludes Apple's `/usr/bin/git` dispatcher. Its canonical regular-file
  bytes are hashed under a 256 MiB ceiling, retained as a diagnostic source
  observation, guarded by stable file identity around every launch, and
  re-hashed when the complete resolution returns. Drift rejects. Every
  launch clears the inherited environment, installs a fixed Git/protocol/
  locale/helper-path environment, requires an explicit absolute working
  directory, and receives resolver-owned stdin, concurrently bounded
  stdout/stderr capture, and a deadline. Stdin is null except for the exact
  object-ID request file supplied to `cat-file --batch`. It
  also runs in a fresh Unix process group or Windows Job Object. Every exit
  path attempts termination, preventing ordinary helper/SSH descendants from
  surviving or holding capture pipes open in tested cases; overflow or timeout
  rejects once cleanup returns. Cleanup and reaping receive a separate bounded
  two-second deadline. A whole-resolution budget permits at most 64 launches,
  independent of package file count, and ten minutes of ordinary elapsed
  execution, including bounded cache-lock acquisition, and passes only the
  smaller remaining interval to each command. Cleanup failure outranks ordinary
  budget expiry; on Unix only `ESRCH`, not `EPERM`, proves that a process group
  is absent. One
  exactly framed `cat-file --batch` launch reads all validated blobs in tree
  order. Blob entries retain shared ranges into that one bounded response and
  release it before staged-source revalidation, avoiding a second
  package-sized resident copy. This is a portable-executor floor, not strict hostile-process
  confinement. Fetch requests only the selected revision at depth one and
  disables Git automatic maintenance and garbage collection; unrelated
  reachable history is not traversed merely to resolve one package revision.
  This bounds history amplification but is not an enforced transferred-byte or
  object-store quota. Local
  sources now follow the same custody shape: a bounded capture is
  re-materialized into a content-addressed, read-only, atomically published
  resolver snapshot. Publication keys bind both canonical live-source lineage
  and content identity, so byte-identical packages from different paths cannot
  collapse onto one compiler source root;
  source/cache overlap and ordinary concurrent mutation reject, and diagnostics
  expose the snapshot path rather than the live tree. Mutable local-package
  capture now excludes the compiler-reserved root `build/` output directory
  without consulting package-authored ignore files; nested `build` directories
  remain source, and symlinks cannot reintroduce excluded output. Exact Git and
  resolver-owned materializations remain exact-tree checks.
  Resolver work limits now survive transport erasure into closure custody.
  Review compilation revalidates canonical read-only modes and re-hashes every
  transitive snapshot under those original limits both before and after each
  compiler invocation; changed custody rejects before review rows are issued.
  The same parent custody walk now rejects accepted Git cache entries whose
  file and symlink logical lengths exceed
  `min(3 * source-byte-limit + 64 MiB, 1 GiB)` and local publications above
  `min(source-byte-limit + 64 MiB, 512 MiB)`. This bounds cache state accepted
  after a helper or publication step; it is not a during-write disk quota or a
  measurement of transferred bytes.

  Remaining suspect points:

  - Windows cache ownership and DACL enforcement remain for the native
    isolation backend; the portable non-Unix floor currently checks only
    concrete kinds and bounded topology;
  - the local before/after check does not defend against a deliberately hostile
    same-user process racing both observations;
  - cache locking coordinates resolver processes but is not protection against
    an independently hostile process that can mutate the cache directory;
  - on Unix the selected Git executable now has resolver/root ownership,
    non-writable/non-set-id executable mode, and safe owned ancestry checks;
    Windows executable ownership/DACL custody, provenance, the already loaded
    image, and executable components Git may launch other than the separately
    observed HTTPS transport helper and SSH client remain;
  - the Git subprocess has no OS sandbox or CPU/memory/process/transfer
    ceilings; process-container cleanup contains ordinary descendants but not a
    hostile Unix process that deliberately changes session; cleanup has its own
    two-second allowance, so neither the per-command nor whole-resolution
    deadline is a strict wall-clock guarantee; the launch ceiling is not a CPU,
    memory, during-write object-store, or transfer-work budget; the post-helper
    logical resident ceiling can reject an oversized cache but cannot prevent
    temporary disk exhaustion while Git is running;
  - SSH uses a content-observed absolute client with Unix custody checks, user
    configuration disabled, batch mode, zero password prompts, and strict
    host-key checking, but still consumes the user's default known-host and key
    files without explicit credential custody;
  - resolver process/network/filesystem authority is not yet represented by a
    hardened execution boundary and receipt.

  Milestone 2026-08-24: parent-owned Git object authentication recomputes
  commit/blob identities, proves the commit-to-root-tree edge, reconstructs the
  canonical recursive tree graph from authenticated leaves and explicit child
  edges, and checks destination containment before snapshot staging. A follow-up
  hostile review closed three gaps: exact requested IDs are now compared with
  the selected authenticated commit, SHA-1 hashing detects and rejects known
  collision attacks, and explicit empty subtrees are authenticated and
  materialized rather than dropped. Object-byte, commit, edge, tree-root,
  collision, exact-pin, and destination mismatches reject without creating a
  stage. Reuse now compares the published source directly with identity derived
  from freshly authenticated entries; rewriting both source and descriptive
  snapshot metadata cannot self-authorize replacement bytes. This is real
  resolver evidence, but it does not weaken any remaining isolation,
  cache-custody, resource, SSH-custody, or receipt requirement.

  Milestone 2026-08-24: symbolic revisions no longer silently choose a SHA-1
  cache. A sealed, launch/output/deadline-bounded `ls-remote` preflight asks for
  only `HEAD` and the requested selector, rejects absent, malformed, or mixed
  object-ID formats, and initializes the quarantine for SHA-1 or SHA-256. Its
  answer is setup input only; commit/blob rehashing and reconstructed-tree
  authentication remain the evidence boundary. Real SHA-256 repositories pass
  with both exact and symbolic revisions.

  Milestone 2026-08-24: local traversal now bounds each directory listing by
  the remaining source-entry budget plus only the one or two names that the
  toolchain itself may exclude, before retaining and sorting the listing. Git
  paths and symlink targets now reject drive/alternate-stream colons, Windows
  forbidden characters and controls, trailing dots/spaces, and reserved device
  names during portable preflight rather than relying on host path behavior.

  Milestone 2026-08-24: Git no longer stores or consults a cache-local remote
  origin. Fetch receives the exact resolver request directly, while the parent
  writes one byte-exact SHA-1 or SHA-256 bare-repository configuration and
  validates it without asking Git to describe itself. Any added setting or
  spelling drift rejects. Git and local caches now receive a separately bounded
  65,536-node parent traversal before and after use. On Unix every cache node
  and lock must be owned by the resolver's effective user and cannot be group-
  or other-writable; canonical ancestry must be root/resolver-owned and not
  replaceable through a non-sticky writable directory; special kinds reject.
  On macOS the same walk reads native extended ACLs for every ancestry, cache,
  publication, staging, and lock node. Any allow entry rejects even when
  ordinary mode bits appear private; deny-only ACLs do not broaden custody, and
  an unreadable ACL fails closed. Symlink nodes are inspected without following
  them, while concrete nodes are checked at their targets. This closes the
  ordinary cross-user ownership/configuration gap independently on Unix,
  including macOS ACL authority, but not hostile same-user racing, Windows DACL
  policy, or native process isolation.

  Milestone 2026-08-25: every Git and local publication lock now proves that
  its opened, locked handle still identifies the current lock pathname after
  acquisition, using device/inode identity on Unix and volume/file-index
  identity on Windows. Path replacement while opening or waiting rejects
  instead of silently splitting synchronization across old and new lock
  objects. This does not claim handle-relative cache custody or protection
  against a same-user replacement after the checked observation.

  Milestone 2026-08-25: the Unix Git executor now rejects a selected binary
  unless its canonical regular file is owned by root or the resolver's
  effective user, executable, non-set-id, and not group/other-writable. Every
  concrete ancestor must likewise be root/resolver-owned and may be externally
  writable only with sticky-entry protection. These conditions are rechecked
  before and after launches alongside the existing inode/content observation;
  the Git cache policy advanced to v9 so weaker fetch custody is not silently
  reused. This closes ordinary cross-user path ownership on Unix, not same-user
  replacement, macOS ACL custody, executable provenance, loaded-image identity,
  helper custody, Windows DACL policy, or native confinement.

  Milestone 2026-08-25: the validated Git request now retains its execution
  transport independently of normalized package lineage. HTTPS grants only
  Git's `https` protocol; SSH URL and SCP-like forms grant only `ssh`; `file`
  remains reachable solely through the test adapter. The cache key and exact
  resolver metadata bind this profile, and policy v10 invalidates cache custody
  created under the former HTTPS/SSH/file union. Resolved-source observations,
  source-audit output, and diagnostic cache-policy schema v3 retain the selected
  profile rather than showing only transport-neutral hosted lineage. This
  closes cross-protocol authority and cache reuse, not effective endpoint, TLS,
  known-host, credential, transport-helper, or native-network custody.

  Milestone 2026-08-25: SSH execution now resolves one exact client path,
  applies the same canonical-file/content/metadata and Unix ownership/mode/
  ancestry checks as the parent Git executable, rechecks it around every Git
  launch, re-hashes it at resolution completion, and retains the observation on
  `ResolvedGitSource`. HTTPS and test-file commands no longer receive latent SSH
  configuration. Cache policy v11 prevents reuse of entries predating this
  floor. This observes the selected client; it does not prove its loaded image,
  confine it, or replace explicit known-host, key, credential-provider,
  endpoint, macOS ACL, or Windows DACL custody.

  Milestone 2026-08-25: HTTPS execution now resolves `git-remote-https` only
  from a closed install-relative candidate set beside the selected Git
  installation. The resolver retains both the exact invocation entry and its
  canonical executable target, hashes the target, applies the executable and
  ancestry custody floor to both identities, and rechecks them around every
  launch and at resolution completion. `GIT_EXEC_PATH` and `PATH` expose only
  that observed helper directory to HTTPS Git commands. Cache policy v12
  prevents reuse of entries fetched before this helper-binding floor. This
  closes ambient HTTPS transport-helper selection; it does not prove the
  loaded image or TLS implementation, bind certificate stores or endpoints,
  inspect macOS ACLs or Windows DACLs, or replace native confinement.

  Milestone 2026-08-26: macOS executable custody now inspects native extended
  ACLs through the narrow compiler-owned `omega-platform-custody` wrapper while
  retaining `forbid(unsafe_code)` in `omega-packages`. The wrapper returns only
  a closed allow-entry fact and never resolves ACL principals through ambient
  identity services. Any allow entry rejects on the selected Git executable,
  the HTTPS/SSH invocation entry and canonical target, or any executable
  ancestor; deny-only entries do not manufacture broader authority. ACLs are
  rechecked wherever mode, owner, ancestry, and executable identity are already
  rechecked around launches. Native tests add an allow entry to both an
  executable and its ancestry and prove rejection, while the selected concrete
  system Git still passes. This closes the ordinary macOS extended-ACL grant
  gap, not same-user replacement, loaded-image identity, executable provenance,
  Windows DACL custody, or native process confinement.

  Milestone 2026-08-25: legacy source-cache policy records now recover only
  from bounded, byte-identical canonical tool-owned encoding. Persistence
  resolves one existing canonical parent as the operation root, rejects
  non-regular or symlink leaves on read, creates a fresh same-directory stage
  exclusively with private Unix mode, synchronizes and re-reads the staged
  bytes, publishes without overwriting through one atomic hard-link creation,
  removes the stage, and synchronizes the parent directory on Unix. Existing
  destinations reject unchanged, including symlinks; stale predictable
  temporary names no longer exist. Parent identity is checked around the
  operation. This hardens diagnostic file behavior only: schema-v3 free strings
  and mutable cache paths remain non-authoritative, hostile same-user
  handle-relative custody remains open, and no diagnostic record can become the
  future opaque resolver receipt or accepted lock evidence.

  Milestone 2026-08-26: resolver-owned Git configuration replacement no longer
  removes `repository/config` and recreates it through an exposed pathname
  gap. A synchronized same-directory stage remains open across a
  handle-relative atomic rename; the parent then confirms exact bytes, file
  identity, and directory synchronization. Local/workspace snapshot
  publication lock acquisition now also polls under a compiler-owned two-minute
  deadline and returns a typed timeout instead of blocking indefinitely. Git
  lock waits continue to consume the whole-resolution budget. These close an
  avoidable control-file race window and one unbounded availability wait; they
  do not claim hostile same-user exclusion, during-write quotas, Windows DACL
  custody, or native process confinement.

  Milestone 2026-08-27: local-package and exact-materialized source capture now
  acquires the canonical source root by walking from its filesystem anchor and
  opening every directory component no-follow, then walks only retained
  directory capabilities. Child directories open without following the final
  component; regular files open no-follow and are read immediately from that
  retained handle, so a leaf reclassified as a symlink cannot redirect the
  later read.
  Symlink spelling and target validation are likewise rooted through the open
  source capability; absolute local-link spellings reject because they cannot
  remain rooted in the published snapshot. Canaries replace a previously
  classified file, directory, and not-yet-opened root with symlinks and replace
  the root pathname after opening; capture rejects the symlink substitutions
  and remains bound to the original open root. This closes the concrete
  classify-then-pathname-reopen gap. It does not make the complete
  capture atomic, exclude a hostile same-user mutator, or close handle-relative
  cache publication, native confinement, or resource quotas. Strict SSH trust
  and credential custody is separately blocked on OWNER Q16.

  Milestone 2026-08-27: Git and local cache-custody validation now acquires the
  canonical cache root through a component-by-component no-follow walk and
  traverses only retained directory capabilities. Child directories are
  classified relative to their retained parent, opened no-follow, and required
  to preserve file identity before descent. Existing entry-count, logical-byte,
  Unix owner/mode, and macOS ACL checks remain in force. Canaries replace a
  classified child directory with a symlink for both cache classes and replace
  an already-opened root; symlink substitution rejects and the walk remains
  bound to the opened root. This closes the custody walk's concrete
  classify-then-`read_dir` redirection gap. Lock acquisition, metadata reads,
  staging, publication, and invalidation still contain pathname operations;
  path-based macOS ACL observation and a hostile same-user mutator remain open.

  Audit 2026-08-24: the authenticated object graph, exact-pin check, injective
  source identity, checkout-free materialization, bounded parent process, and
  immutable publication form a coherent portable core. The next strict-boundary
  slice was one validated typed Git request accepted by every public resolver
  route. Completed 2026-08-24: public Git resolution now requires a validated
  `GitSourceRequest`; accepts only HTTPS, SSH URLs, and SCP-like SSH locators;
  rejects ambient local paths, insecure protocols, embedded credentials,
  malformed locators, and refspec-shaped revisions; persists only sanitized
  lineage; and applies compiler-owned locator, revision, entry, byte, and depth
  ceilings. The local-repository route is explicitly test-only. Remaining P0
  work is native fetch/materialization confinement, effective endpoint and SSH
  credential custody, during-operation resource quotas, handle-relative cache
  custody, canonical build-observable source metadata, and a locally
  reconstructed opaque strict receipt. Public requests now admit only HTTPS and
  SSH transports; the sealed executor grants only the request's selected
  `https` or `ssh` protocol, disables HTTP, unauthenticated `git://`, every
  unselected protocol, and HTTP redirects, and permits file transport only
  through the explicit test adapter. Cache identity and metadata bind that
  execution profile even when hosted lineage normalizes HTTPS and SSH together.
  This removes cross-protocol authority and redirect-selected endpoint
  substitution but does not yet retain or confine the effective
  socket/DNS/TLS/SSH endpoint. Helper,
  diagnostic, and future resolver routes must not bypass the same request
  validator.

  Acceptance: cache ownership/origin is verified, identities use full
  collision-resistant keys, Git runs with sealed configuration in an isolated
  process boundary, materialization/archive policy is enforced before
  consumption, and source hashing is injective over every filesystem
  distinction that can affect compilation or build execution.

## P1 — Package declaration and identity

- [x] **PACKAGE-DECLARATION-VOCABULARY.** Add the toolchain-owned `Package` build
  data and require exactly one `const PACKAGE: Package` in each package
  `build.omg`.

  **Superseded 2026-08-25 by P8.** Identity is declared through
  `builder.package("name")` on the ordinary build surface; the `const PACKAGE`
  literal and its bespoke static parser are retired. The completion note below
  records what was built, not the settled model.

  Acceptance: extraction occurs hermetically before dependency resolution or
  build execution and rejects missing, duplicate, effectful, generated,
  dependency-dependent, or invalidly spelled declarations.

  Completed 2026-08-24: the compiler prelude owns `Package { name: &[u8] }`, and
  `omega-packages` now extracts the exact literal declaration through the
  ordinary Psi lexer/parser without loading imports or executing code. It
  rejects package-authored `Package`, malformed/scoped/duplicate declarations,
  nonliteral initializers, invalid bytes, and names that cannot map to a default
  Omega alias. Git, workspace-member, and external-local source custody now
  extract this declaration from the resolver-owned immutable snapshot and join
  it to typed lineage before recursive resolution. Extraction parses only the
  immutable root `build.omg`; it cannot execute generated, imported, effectful,
  dependency-dependent, or build-host code.

- **PACKAGE-KEY-AND-INSTANCE.** Replace name-keyed graph and lock APIs with
  `PackageKey` and `PackageInstance`.

  Acceptance: same-name/different-lineage packages cannot collide or spoof
  nominal symbols; source/name changes are replacement; exact commit, tree,
  content, artifact, obligation-semantics, discharge, and open-assumption
  identities bind one instance.

  Progress 2026-08-23: a typed identity core now binds `PackageKey` to
  `PackageName` plus `SourceLineage`, and typed immutable source resolutions
  reject a family that does not match the key lineage at graph/source custody
  boundaries. The earlier caller-constructible `PackageInstance` placeholder
  was removed: its replacement must join exact source and artifact subjects to
  re-derived certificate evidence by construction. The legacy name-keyed graph,
  lock, and evidence APIs have been deleted; constructing their accepted typed
  replacements remains. Resolved Git, workspace-member, and
  external-local package sources now carry `PackageKey` plus typed immutable
  source resolution but deliberately cannot construct an accepted instance.
  `PackageKey::identity()` now emits a domain-separated opaque 256-bit
  commitment shared with the compiler; it is stable across revisions and
  changes when package name or canonical lineage changes. A production-path
  canary now resolves two byte-identical, same-declared-name packages from
  distinct external-local lineages in one closure and proves compiler review
  keeps their package identities separate. Provider selection imported from
  one lineage retains that exact realizing, provider-type, and schema package;
  the same-spelled lookalike cannot replace it.

- **SOURCE-LINEAGE-NORMALIZATION.** Define canonical lineage for Git, URL
  archives, and local/workspace paths.

  Acceptance: HTTPS/SSH spellings of one known Git repository normalize
  together without asserting equivalence that cannot be established. Mirrors
  require explicit relocation/delegation evidence. Workspace members use
  workspace lineage plus member-relative path; external paths remain marked
  non-portable development sources. Each archive/protocol adapter defines
  lineage and immutable-content evidence instead of guessing from a locator.

  Progress 2026-08-24: conservative known-host lineage adapters normalize
  GitHub and hosted GitLab HTTPS, SCP-like SSH, and `ssh://` spellings. GitLab
  supports exact nested namespace paths while retaining path case; self-hosted
  and unknown hosts retain transport, user, port, case-sensitive path, and
  suffix distinctions.
  Workspace members bind a normalized relative path to workspace lineage, and
  external local sources bind canonical absolute path plus consuming context.
  An explicit external-local closure adapter now preserves that context through
  recursive relative or absolute Path requests, snapshots every selected
  package before projection, and keeps same-content paths distinct by canonical
  lineage. A separate contextual workspace entrypoint routes only live-workspace
  escapes into that same external-local lane; strict workspace resolution and
  every fetched Git snapshot remain confined. Neither entrypoint performs
  ambient workspace or lock discovery.
  Workspace-member custody now derives the live member solely from its
  normalized root-relative location, verifies it remains a strict canonical
  descendant of the workspace root, and snapshots only that member through the
  immutable local resolver before extracting its declaration and dependency
  requests.
  Archives, mirrors/delegations, additional protocols, recursive workspace
  traversal, and wiring resolver receipts into these types remain.

- **PACKAGE-QUALIFIED-NOMINAL-IDENTITY.** Thread `PackageKey` through package,
  symbol, boundary-trait, provider, and evidence identities.

  Acceptance: a same-spelled package or boundary declaration from another
  source lineage cannot satisfy or replace the admitted identity.

  Progress 2026-08-23: target-neutral Psi now owns only the opaque
  `PackageKeyIdentity` carrier, while source-lineage normalization remains in
  `omega-packages`. Managed compiler sources retain that identity and
  same-package checks prefer it over path spelling. Managed authored symbols
  recover it from retained source metadata. Provider plans and provider trust
  rows now retain compiler-derived package identities for the realizing
  machine, nominal provider type, selected service schema, and each inherited
  or direct requirement owner. Those identities enter the existing normalized
  plan fingerprint; readable labels are diagnostic only. That 64-bit
  fingerprint remains review/execution compatibility data, not sealed package
  admission identity. Post-resolution compiler symbols now require an existing
  derivation-origin symbol and inherit its exact package/toolchain provenance;
  source-free symbols remain deliberately unresolved. Pre-resolution generic
  normalization now retains each generated concrete data instance's exact base
  declaration and ordered structural arguments through resolved, typed, copied,
  and snapshot forms. Conformance matching unfolds only that retained origin;
  it neither parses synthetic names nor accepts same-spelled foreign symbols.
  The symbol pass binds each concrete instance to that already-resolved base as
  its derivation provenance. Package/toolchain ownership therefore survives a
  generated-source final compilation without interpreting synthetic display
  names such as `Optional<u64>`. Authored toolchain
  nominals in package review now bind a domain-separated SHA-256 commitment to
  the canonical toolchain-relative source path and exact source bytes;
  canonical virtual prelude coordinates use the same framing. Source-free
  compiler intrinsics remain explicitly unbound until their exact compiler-
  owned semantic subject and obligation origin exist. Checked-adapter rows now
  bind a canonical typed machine-overload identity to the exact package owning
  that machine, reject row transplantation across realizing packages, and
  resolve without short-name fallback in validation, dispatch, progress,
  external-root, TCB, and trust projections. Authored provider selections now
  retain exact resolved boundary-Trait and provider-Data symbols plus their
  package-qualified canonical paths; plan matching has no leaf-name fallback.
  Exact intrinsic semantic identity and the recheckable package-evidence
  projection remain. Terminal Psi evidence remains separately required for
  rows that make final-realization claims.

## P2 — Dependency projection and reconciliation

- **BUILD-DEPENDENCY-API.** Replace the transitional
  `build.depend("alias", path("dir"))` seam with ordinary typed source requests:
  `builder.depend(source)` and exceptional
  `builder.depend_as(alias, source)`.

  Acceptance: normal install supplies only source/revision; package name and
  default alias come from the fetched package. The editor rewrites only
  canonical direct rows and otherwise emits a non-mutating patch. The API is
  implemented with ordinary Omega vocabulary and may be simplified when
  compiler work proves a smaller existing mechanism sufficient.

  Progress 2026-08-24: the compiler-provided ordinary Omega vocabulary now
  defines `Source::Path`, `Source::Git`, `Build::depend(source)`, and
  `Build::depend_as(alias, source)`; the old free `path()` helper and mandatory
  alias overload are gone. The package-side projector consumes canonical direct
  forms and validates an exceptional explicit alias as an Omega snake-case
  identifier. A non-mutating editor now plans additions and exact-row
  replacements against the current `build.omg` SHA-256. It automatically edits
  only the canonical `machine build(builder: &mut Build)` entry, maps replacement
  rows from the validated projection back to direct token spans, preserves
  unrelated source, and re-projects the generated candidate before returning
  it. Missing build entries receive the ordinary canonical machine. Ambiguous,
  commented, or noncanonical rows produce only compiler-generated old/new
  statements and a manual-placement reason; source-controlled strings remain
  escaped Omega literals. Validated closures are already translated into exact
  compiler inputs and reviewed dependency-first. Atomic application after
  admission and broader target vocabulary remain.

- **DECLARATION-SELECTION-AND-CARRIED-IDENTITY.** Enforce the settled split
  between direct source authority and inferred nominal flow. Any authored path,
  member, field, case, operator, conformance, or ordinary explicitly named
  consuming machine must resolve to the current package or one direct
  dependency. A foreign type
  received through a declared dependency may still be moved, borrowed, stored,
  returned, passed back through that surface, and checked for copy/affine/linear
  behavior; compiler-planned layout and automatic cleanup do not grant source
  access to the owning package.

  Acceptance: diagnostics name the selected declaration's owning package and
  the missing direct dependency. The lock retains its existing transitive
  closure, while package/artifact evidence records exact declaration-level
  semantic dependencies with private-versus-public disposition. Whole-package
  keying remains a sound conservative gate until those exact edges land; no
  inferred type silently widens source nameability.

  Ratified design decision 2026-08-24: capture a package-agnostic authored-
  selection ledger during resolution, while exact source spans and public/
  private syntactic position still exist, then finalize it after successful
  checked lowering. Static paths and ordinary members may settle at resolution;
  receiver-dispatched calls, result overloads, operators, and inferred
  conformances join from the checked facts that settle them. Every occurrence
  must finalize exactly or reject. This is a compiler-internal sidecar assembled
  from existing semantic stages, not nominal Chi.

  The gate applies only to authored selections. Build a separate exact semantic-
  dependency set for carried nominal identity, layout/move/copy behavior, and
  automatic cleanup, promoting private disposition to public when any public
  surface exposes the dependency. These rows affect artifact/rebuild and public
  compatibility identity without granting source authority. No selected package
  or build-time code may execute before the selections applicable at that stage
  pass direct-authority admission. Final execution consumes finalized rows;
  earlier effect-free evaluation consumes exact early targets or fails closed
  when the compiler cannot confine the complete candidate set. Reorder or split
  evaluation where necessary.

  Implementation slices: retain exact symbols for every selected path segment,
  struct-literal type/case/fields, and case membership; capture explicit type,
  member, call, and conformance occurrences with exposure; reject authored
  selection of the reserved owner-attached cleanup hook before package
  admission; join dynamic
  calls, overloads, operators, inferred conformances, and automatic cleanup
  after checking; carry canonical package names through compiler handoff for
  diagnostics; emit semantic dependency evidence. Canary the full three-package
  `root -> middle -> leaf` matrix, including carried flow, inferred field/method,
  case construction/membership, operator and conformance selection, ordinary
  consuming `omega::core::drop` versus compiler-selected cleanup, rejected
  authored hook selection, toolchain declarations, spoofed same-name cleanup,
  and private/public evidence disposition.

  Progress 2026-08-24: resolved and typed expressions now retain exact symbols
  for every name-path segment, member, struct type/case/field, and case
  membership; contracts use the same exact resolver. Attached nominal identity
  now survives checked machine lowering and drives automatic cleanup without
  name fallback. Compilation carries canonical package names solely for
  diagnostics while exact package identity remains the gate key. A
  deterministic compiler-owned authored-selection occurrence ledger survives
  resolved, typed, copied, specialized, and checked trees, and expression
  handles retain every associated occurrence. Private state-body expressions
  now capture exact paths, members, calls, struct types/cases/fields, domain
  membership, and operators while source ownership and exposure are present.
  Checked lowering finalizes calls, effective inferred members, paths, struct
  rows, declared operators, and explicitly classified compiler intrinsics by
  exact occurrence; inconsistent clone resolution rejects transactionally.
  Source-less parser/lowering helpers are excluded at candidate capture rather
  than treated as authored code. Package-aware checked and native compilation
  reject any package-authored occurrence still late after successful checking,
  then reject every finalized user-package selection whose owner is neither the
  requester nor one direct dependency. A three-package canary rejects
  `root -> middle -> leaf` transitive-only selection. Build-time const-generic
  and placed-view probes lower with the assembled package source map instead of
  losing source ownership. An earlier global typed-lowering struct-literal
  identity check was removed: the exact authored-selection ledger, not ordinary
  typed lowering, is the package-admission boundary.

  Milestone 2026-08-25: package-aware checked and native compilation now clone
  the frozen ordinary typed graph into a complete checked preflight and admit
  every finalized authored selection before `compute_build_config` can execute
  the selected build machine. The ordinary final checked pass repeats the gate,
  including any explicitly handed-off generated source. A filesystem-backed
  canary places an illegal transitive selection after an attempted marker write
  in `build.omg`; both entrypoints reject with the marker absent.

  Milestone 2026-08-25: Psi's earlier effect-free evaluators now consume a
  package-neutral selection-authority interface backed by the reconciled direct-
  dependency graph. Const-generic calls, fixed-array const calls, const-domain
  facts, laid/placed layout policies, wire policies, and calling policies retain
  exact invocation custody and admit it before evaluation. Admission walks the
  concrete build-time call closure and checks each caller-to-callee edge plus
  every authored declaration selection in each reachable body. Shared policy
  results are reused only after every authored application site passes. Exact
  resolved targets are preferred; a target still awaiting later checking fails
  closed unless the compiler can prove its entire candidate set is confined to
  toolchain, self, or direct dependencies. Operators use the checked layer's
  conservative intrinsic judgment rather than spelling. Const substitution
  retains a provenance-only declaration symbol and attaches the authored const
  occurrence to the substituted expression, so value erasure cannot erase
  package custody. Root-middle-leaf canaries cover all of these entrypoints,
  including a package-internal undeclared selection reached through an admitted
  build-time call.

  This gate deliberately reads the earliest coherent private typed/probe state
  owned by the compiler. It need not wait for public or Terminal Psi, and it is
  allowed to move with compiler internals. There is no nominal Chi stage. Add a
  named stage only if implementation discovers a genuine reusable semantic
  invariant boundary. Additional consumers or transformations may reveal such
  a boundary; stability, layer purity, or local simplification alone do not.

  Milestone 2026-08-25: symbol-resolved nominal type references now retain
  exact authored-selection rows as they enter typed trees. Public `data`,
  `domain`, machine-head signatures, traits, and wire surfaces record public-
  interface exposure; private declarations, internal state signatures, local
  type annotations, casts, and public-machine owned storage record private-
  implementation exposure. Generic
  bases and named dynamic-trait conformances retain the same custody, while
  binders, locals, primitives, and source-free compiler nodes do not pretend to
  be package declarations. A root-middle-leaf canary rejects a public API that
  explicitly names a transitive-only leaf type and accepts it once the leaf is
  a direct dependency. Checked finalization also classifies logical, bitwise,
  and shift operators as compiler intrinsics directly: these operators have no
  declaration-spelling surface, so nested expression origins no longer leave
  an ordinary `&&` occurrence unresolved at package admission.

  Milestone 2026-08-27: proof-static member finalization now recovers the
  receiver's exact declared type from its retained symbol. A package field is
  selected first; only `len` on a checked fixed array or slice finalizes as the
  closed compiler-owned `CollectionLength` meaning. Package review v75 and
  canonical row v33 encode that meaning as a structural receiver expression,
  require exactly one public-interface authored-selection occurrence, and do
  not assign it a fictional package owner. A transparent public-proposition
  canary exercises the intrinsic, while a package field named `len` remains an
  ordinary package-qualified nominal member. Other unrepresented compiler
  intrinsics remain fail-closed.

  Milestone 2026-08-25: source-backed static conformance arguments on generic
  calls now retain the exact package-scoped conformance selected by each
  authored argument, including nested static applications. Checked trait-backed
  operators append the exact selected conformance at the operator token. For an
  unbound generic `where Element satisfies Trait` requirement, specialization
  validation now retains the one conformance whose uniqueness it proved,
  separates that inferred selection from explicit evidence arguments, and
  includes its package-qualified declaration identity in specialization
  fingerprinting. The checked ledger attaches that inferred declaration to the
  authored call occurrence. A root-middle-leaf package canary rejects root's
  inferred `GoodMarker` selection when leaf is only transitive and accepts it
  after root admits leaf directly.

  Milestone 2026-08-25: source-authored statement calls now enter the same
  ledger before symbol-resolved statement trees are rebuilt into tables. Exact
  targets settle immediately; unresolved targets retain a checked-call
  obligation which finalization joins to the exact checked flow call. Explicit
  static conformance arguments on statement calls retain their own source spans,
  and inferred generic conformances attach to the statement's call-target span.
  Compiler-owned build markers and lowered inline-assembly operations finalize
  to closed intrinsic variants rather than pretending to have package symbols
  or being silently skipped. Root-middle-leaf canaries reject both a
  transitive-only void statement call and a conformance inferred by one, then
  accept each after direct admission.

  Milestone 2026-08-25: every explicit source-backed static argument path on an
  expression or statement call now retains declaration custody. Conformance
  arguments keep their dedicated kind; type, static-machine, and forwarded
  binder paths use one `StaticArgument` kind because the selected declaration
  and callee telescope already determine the semantic category. Nested static
  applications recurse. Integer literals select no declaration, while named
  const substitution keeps its existing exact const-declaration provenance.
  An unresolved static path remains an explicit late obligation and therefore
  fails package admission rather than disappearing. A root-middle-leaf canary
  rejects transitive-only data and machine arguments independently and accepts
  both after direct admission.

  Milestone 2026-08-25: declaration-owned expression lowering now carries the
  owning declaration's public/private exposure into the authored-selection
  ledger. Public machine contracts, public data/domain predicates, and public
  trait contracts are public-interface positions; machine states, executable
  bodies, and `terminates by` ranking witnesses remain private implementation
  even when their machine is public. Standalone declarations without `pub`
  remain private. Proof-membership facts now retain their exact domain path and
  exposure as first-class rows; lexical parameters and locals no longer
  masquerade as late declaration selections. Compiler-recognized byte-sequence
  predicate calls finalize as intrinsics, while an exact declaration with the
  same spelling retains declaration identity. A root-middle-leaf canary rejects
  a public contract's transitive-only domain selection and accepts it with exact public
  exposure after direct admission.

  Milestone 2026-08-25: generic conformance bounds on machines and traits now
  retain the exact declarations authored on their right-hand side. In
  `Element satisfies Ranked`, `Ranked` is a trait selection; in
  `Element satisfies Card::PowerOrder`, `Card` is the selected carrier and
  `PowerOrder` is the selected package-scoped conformance. The subject and
  evidence binder remain lexical. Rows inherit the enclosing declaration's
  exposure. A root-middle-leaf canary rejects a public callable's named
  transitive-only conformance bound and accepts it after direct admission.
  This custody does not replace the declaration's settled publication rule.

  Milestone 2026-08-25: checked flow now retains a package-neutral exact
  semantic-dependency sidecar. Machine-head types, checked call-result types,
  and ownership-place types contribute exact nominal identity, layout, and
  ownership-behavior rows; automatic affine cleanup additionally retains the
  exact nominal cleanup machine selected through attached declaration identity,
  never a same-spelled unrelated machine. Duplicate private/public observations
  promote to public interface. A root-middle-leaf package canary permits root
  to carry a leaf-owned type solely through middle while retaining leaf's exact
  owner in root's private dependency evidence. This is a checked fact carrier,
  not nominal Chi. The compiler package-review projection now qualifies both
  the reviewed consumer and carried declaration by exact package ownership,
  emits blocking canonical rows for nominal identity, layout, ownership
  behavior, automatic cleanup, and the selected cleanup machine, and retains
  exact source anchors for both sides. Exposure is row value rather than row
  identity, so private-to-public promotion is one explicit changed row. A
  three-package canary proves the root review names leaf ownership exactly
  without granting root authored authority over leaf.

  Milestone 2026-08-26: package review no longer trusts the retained semantic-
  dependency sidecar by itself. Immediately before projection, the compiler
  rederives the complete canonical table from the final typed program and
  checked facts and requires exact ordered equality. Missing, extra,
  duplicated, reordered, or altered rows reject before package qualification;
  orchestration still receives only the versioned canonical projection, not
  private typed or checked handles. This reuses the existing coherent checked
  derivation and introduces neither a public IR contract nor nominal Chi.

  This is deliberately not yet total admission. Toolchain-authored bodies are
  outside package admission. Capture now covers private state-body expression
  forms, nominal type references on public/private declaration surfaces,
  explicit static conformance arguments, expression and statement calls,
  all source-backed static declaration arguments, inferred generic-call
  conformances, checked trait-operator conformances, and declaration-owned
  expression positions whose visibility is settled, and named conformance
  selectors in callable and trait bounds. Visibility-dependent nested positions
  are not yet total. The package manager stays disabled until those gaps close.
  The first exact carried-semantic-dependency carrier and its versioned
  canonical review projection have landed. Total coverage and accepted
  artifact/lock admission are not complete. Visibility implementation for all
  independently selectable roots remains in the task below.

  Milestone 2026-08-25: one exact-symbol validation pass now rejects every
  finalized authored selection of the reserved owner-attached `T::drop` hook.
  It runs before checked evaluation for already-resolved selections and again
  after checked finalization for receiver-dispatched and other late-bound
  selections. Calls, qualified paths, static arguments, and forwarded/static
  paths therefore share one rule; compiler-planned automatic cleanup is absent
  from the authored ledger, while free `drop`, `omega::core::drop`, and
  same-owner names such as `drop_counter` do not match by spelling. Real-source
  and package checked/native canaries cover the callable paths. The ordinary
  consuming `omega::core::drop<T>(value)` implementation remains separately
  sequenced after generic cleanup-row lowering under
  `CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP` in `TASKS.md`.

- [x] **COMPLETE-STANDALONE-DECLARATION-VISIBILITY.** Extend ordinary `pub`
  retention through syntax, symbols, typed/checked trees, snapshots, package
  review, and compatibility identity for independently nameable `operator`,
  `proposition`, and `const` declarations. Carrier-qualified roots own their
  visibility; only genuine members of one exact semantic owner inherit it.
  Reject every public-interface selection of a private declaration. Declared
  measures are always package-private ranking witnesses: `pub measure` rejects,
  `terminates by` and its selected measure remain private proof evidence even on
  a public machine, and another package cannot cite that measure directly.
  Compiler intrinsics remain a closed non-package selection category.

  Milestone 2026-08-25: ordinary `pub proposition` now survives syntax copying,
  resolved and typed lowering, checked proposition vocabulary, snapshots, and
  source profiling. Psi validation rejects a public-interface occurrence that
  selects a private proposition even without package mode; package admission
  separately rejects private proposition selection across package ownership,
  including from a declared direct dependency. Review v52 and canonical row
  v12 emit one blocking `PublicProposition` row for every package-owned public
  proposition, including unused bodyless declarations. Rows retain
  alpha-normalized binders, parameter types, witness interfaces, and normalized
  transparent expansion. Publishing a bodyless declaration creates no checked
  proposition application or admission. Public transparent aliases remain
  absent from Terminal proposition identity but are deliberately present as
  source API compatibility rows. Operator visibility and const declaration-
  site compatibility must land before public transparent proposition leakage
  is total; measure remains an independent slice.

  Milestone 2026-08-25: ordinary `pub const` now survives syntax copying,
  resolved declaration-symbol retention, typed/checked trees, snapshots, and
  source profiling without introducing runtime const storage or value identity.
  Psi validation rejects a public-interface occurrence selecting a private
  const in ordinary compilation; package admission separately rejects a
  private const selected across package ownership even when the owner is a
  direct dependency. Const substitution continues to retain the exact selected
  declaration symbol, so value erasure cannot erase this gate.

  Milestone 2026-08-25: review v53 and canonical row v13 emit one blocking
  `PublicConst` row for every package-owned public const, including unused
  declarations. The row binds the exact package-qualified declaration, exact
  typed declared-type identity, and canonical structural declaration value;
  source initializer text, display text, and runtime storage identity are not
  compatibility material. Declared types recursively reject private data.
  Public const forms for which the compiler cannot establish this identity,
  including constrained declarations pending declaration-site proof checking,
  reject instead of falling back to source spelling. Private const-v0 behavior
  remains unchanged. Changing the declared type or canonical value therefore
  produces a blocking compatibility conflict and a source-backed
  `public_const` review row.

  Milestone 2026-08-25: ordinary `pub operator` now survives syntax copying,
  source profiling, resolved and typed lowering, checked trees, and snapshots.
  Operator visibility is independent of a carrier-qualified path; one shared
  exact-symbol lookup covers root and domain-homed declarations. Operator
  symbols retain their own authored source provenance rather than a joined
  diagnostic path, so package ownership remains recoverable. Proof-static
  public-contract selections without executable use facts finalize only when
  exact typed operands select one declaration, then the ordinary visibility
  gate is repeated after checked finalization. Package admission rejects a
  private operator selected across package ownership while allowing private
  implementation use by its owner.

  Milestone 2026-08-25: review v54 and canonical row v14 complete the
  standalone blocking `PublicOperator` lane. Its key is the package-qualified
  declaration family plus the compiler's canonical operand and result-dispatch
  identities, so overload matching is exactly the checked language rule rather
  than a package-manager reconstruction. The row retains boundary status,
  fixed-token spelling, lifetime/static telescopes, parameter modes and types,
  return type, and directly projected declaration contracts even when the
  operator is unused. Binary public-contract expressions now distinguish an
  exact declared overload from compiler-owned builtin meaning; a bare token no
  longer substitutes for semantic selection. Public operator changes render as
  source-backed blocking conflicts. Proof-static member selections which
  checking cannot finalize still reject closed instead of producing partial
  rows; that is compiler coverage work, not an open package-manager design
  choice.

  Milestone 2026-08-25: the visibility canary audit removed one stale premise:
  `pub measure` deliberately parse-rejects because a declared measure belongs
  to the private `terminates by` ranking witness, while `terminates` alone is
  the published guarantee. A package-aware canary now proves a public recursive
  machine may cite its same-package private declared measure. The public-source
  survey also found two interrupt obligation carriers exposed by public traits
  while still private. Both now use Omega's existing coherent opaque surface,
  `pub boundary data ... [linear];`; provider settlement identity is no longer
  published as structural fields. The language guide and hardware/privilege
  briefs describe the same opaque representation boundary.

  Settled 2026-08-25: complete name-first conformances are independently
  nameable declarations, package-private by default, and publish through
  ordinary `pub`. Exact `machine ... satisfies Trait::requirement` edges follow
  machine visibility; an optional `as Name` groups requirement-local satisfiers
  and does not mint standalone whole-trait evidence. Implement syntax, symbol,
  typed/checked-tree, authored-selection, snapshot, and package-review retention
  for the complete item. Add one blocking `PublicConformance` row containing
  the package-qualified declaration, normalized telescope, optional subject,
  exact trait application and complete checked requirement interface while
  excluding member/proof bodies and physical code identity. The referenced
  public trait row owns each requirement's law; conformance validation must
  discharge those laws before this row can be issued.

  Milestone 2026-08-25: the ordinary visibility gate is implemented through
  syntax, symbol-resolved, typed/checked trees, snapshots, and package-aware
  selection. Complete named conformances now retain `is_public`; private ones
  remain selectable inside their package but reject from public interfaces and
  dependencies, while `pub` conformances additionally require the declaring
  package as a direct dependency. Public headers expose their exact subject,
  trait, and static-argument selections, so private declarations cannot leak
  through a published conformance. Review v55 and canonical row v15 now emit
  the blocking `PublicConformance` compatibility row. Its key is only the exact
  package-qualified conformance identity. Its value retains the
  alpha-normalized lifetime/static telescope, subject, exact trait application,
  and complete normalized inherited requirement interface. Trait requirement
  overloads use the compiler's canonical callable identity, never source
  ordinal or a same-spelled path. Closed and attached-machine source forms
  normalize to the same row; realization names, bodies, and physical code stay
  private. Validation checks every realization signature and substituted trait
  law before projection, so a deferred or unrelated proof cannot publish the
  conformance. The corresponding `PublicTrait` row owns the law text instead
  of duplicating it into every conformance row. Unsupported lifetime-parameterized
  target traits, inherited lifetime substitution, and proof-static trait
  parameters continue to reject closed until their existing IR carriers retain
  a complete identity.

  Milestone 2026-08-25: one shared typed-tree visibility resolver now gates all
  settled independently nameable declaration families in both public
  interfaces and cross-package authored selections. Genuine fields, variants,
  and states inherit their exact owner; carrier-qualified domains and operators
  retain independent visibility. Package canaries cover private/public data,
  domain, machine, and trait selection plus carrier mismatch. Qualified domain
  constraints now preserve their source span until carrier-aware normalization
  binds the exact domain and records it in the authored-selection ledger, so a
  private domain cannot cross a package boundary through a type annotation.
  Toolchain build vocabulary and the core layout/optional/filesystem/console
  surfaces now mark the APIs they actually publish; implementation helpers
  remain private.

  Milestone 2026-08-25: complete name-first conformances now retain ordinary
  `pub` independently from their subject and trait through syntax copying,
  source profiling, resolved and typed trees, checked compilation, and syntax/
  resolved/typed snapshots. Public conformance headers retain their carrier,
  trait, static telescope, trait arguments, normalized row map, and exact
  symbols; public headers reject a private carrier or trait. The shared exact-
  symbol gate now rejects private cross-package conformance selection and
  public-interface citation while preserving same-package private use. Calls
  through lexical conformance and machine binders inherit visibility from the
  enclosing declaration rather than treating the binder as a package API.
  An explicit `Trait::requirement = Machine::entry` row now retains its authored
  target span until closed-map normalization resolves the exact machine, then
  enters private-implementation selection custody; cross-package references to
  private realization machines reject even when the conformance is public.
  Cross-package canaries select an explicit conformance argument and prove that
  only `pub PowerOrder` crosses the package boundary. Review v55 consumes that
  custody in a blocking canonical `PublicConformance` row and retains authored
  provenance for compiler-normalized inline realization symbols without
  promoting those symbols to public API. Canonical recovery and upgrade
  conflict rendering cover the new row. No accepted admission is implied by
  visibility or review retention alone.

  Completed 2026-08-25: ordinary visibility, package-authority selection,
  checked retention, snapshots, and blocking compatibility rows now cover all
  independently nameable declaration families in this task. Cross-package and
  public-interface canaries cover proposition, const, operator, and complete
  named conformance visibility; carrier-qualified declarations retain their
  own visibility; exact-edge `as Name` labels cannot satisfy whole-trait
  bounds; public bare-`dyn` returns may carry private producer-selected
  evidence without making it receiver-nameable; bodyless propositions mint no
  facts; and public termination guarantees retain private ranking witnesses.
  `pub measure` remains a parser error by design.

  Coverage includes cross-package pass/fail canaries for every declaration
  kind, a carrier-qualified domain or operator whose carrier has different
  visibility, a public contract selecting a private proposition/const/operator,
  a public machine with a private ranking witness, parser rejection of
  `pub measure`, and a public bodyless proposition that grants no fact by
  declaration alone.
  Conformance coverage includes private same-package selection, public
  cross-package selection, private cross-package and public-contract rejection,
  an `as Name` exact-edge label that cannot satisfy a whole-trait bound, and a
  public `dyn` return carrying private producer-selected evidence without making
  that evidence nameable by the receiver.
  Survey every private nominal reachable from an existing public signature:
  publish it only when structural construction is intended, and stop for a
  separate opaque-public-type design if publishing its representation would be
  wrong. Core `Extent` is the deliberate structural case; its geometry is
  public while `Extent::Granted` remains the routed authority.

- [x] **HERMETIC-DEPENDENCY-PROJECTION.** Derive dependency source requests without
  executing build-host effects or imported code.

  Acceptance: dependency rows cannot depend on filesystem/network observations,
  generated files, clocks, or build outputs. Malformed or unsupported
  projection rejects explicitly; nothing is silently skipped.

  Completed 2026-08-24: `omega-packages` parses only the immutable root
  `build.omg`, accepts direct literal Path/Git rows through `depend` and
  `depend_as` in authored order, and rejects authored toolchain vocabulary,
  malformed/scoped builds, invalid aliases, nonliteral or nested/helper-mediated
  requests, and unsupported cases. An absent build machine projects no
  dependencies. Resolved package-source custody performs this projection before
  returning. The compiler has separate native and checked package-aware
  entrypoints that accept only a validated, closed, requester-local
  alias-to-`PackageKeyIdentity` graph and canonical source roots; this mode never
  invokes or combines syntactic discovery.
  A transport-neutral package-side resolver recursively closes those projected
  requests through an adapter callback before returning any graph. The narrow
  standalone compatibility scanner was removed 2026-08-26: standalone
  compilation now resolves only ordinary root-relative and toolchain imports,
  while requester-local aliases require validated `PackageCompilationInputs`.
  No compiler path reparses dependency build files, silently skips malformed
  rows, installs identity-free package roots, or shares aliases globally.
  Scanner-dependent placed-access canaries now use a closed package graph. The
  migration also rejects ambiguous policy/schema spellings, retains their exact
  source identities, and checks both declarations' package visibility and
  direct-dependency authority before synthesis. Because `Placed<P, S>` is
  erased before ordinary type-selection capture, both nominal inputs must be
  public even for local use; this prevents a public signature from laundering a
  private declaration through the source-free shell. Its inert opaque field
  carriers follow shell visibility, while callable operation visibility follows
  exact `AccessExposure` and retains `BindingPrivate`. Statement-position
  accessor calls resolve to their exact generated state. Pre-check operator
  execution is intrinsic only when no authored declaration candidate exists;
  final checking still validates builtin semantics and authored operator
  selection. No owner decision was required.

- [x] **CLOSURE-RECONCILIATION.** Resolve the complete source closure before any
  dependency build receives providers.

  Acceptance: one `PackageKey` resolves to one immutable instance in v1;
  conflicts report every requesting dependency path. Resolver authority never
  enters package build execution.

  Completed 2026-08-24: a typed pre-admission source graph validates exact
  roots and edges, one immutable resolution per `PackageKey`, requester-local
  alias uniqueness, closed reachability, and same-name/different-lineage
  separation. Package dependency cycles conservatively reject in v1. A sealed,
  transport-erased custody record is derivable only from resolved package
  source custody and retains exact key, resolution, immutable snapshot root,
  and projected requests. Recursive reconciliation delegates transport and
  requester-relative path interpretation to an adapter, derives default aliases
  from fetched declarations, honors explicit aliases, reuses exact duplicate
  custody, and resolves the complete finite closure before constructing the
  validated graph under package-count, dependency-request, and depth ceilings.
  Conflicting resolution or custody-root observations retain every root-to-
  request path and reject. The returned value keeps both the validated graph
  and exact source custody; it has no lock, persistence, or admission API. A
  compiler-side handoff independently rejects missing/duplicate/overlapping
  roots, invalid or duplicate requester-local aliases, missing targets,
  unreachable rows, cycles, source-root drift, toolchain overlap, dependency
  `build.omg` imports, and symlink escapes. The package side now translates only
  a validated source-custody closure into those compiler inputs; the compiler
  independently canonicalizes and revalidates every root and edge. A production review-only
  orchestrator now compiles every closure package in deterministic
  dependency-first order. Each package is temporarily re-rooted over only its
  transitive dependencies, so unrelated siblings cannot enter its compiler
  graph. The caller supplies a workspace; orchestration creates one fresh
  disposable child session beneath it and assigns package-and-source-specific
  writable roots inside that session. Resolver snapshots remain immutable.
  Orchestration withholds every returned row until the child session has been
  cleaned up successfully; cleanup failure rejects the review. The returned
  rows have no public constructor and retain
  exact `PackageKey`, selected immutable resolution, compiler projection, and
  canonical comparison bytes. Package-aware checked compilation now also
  issues one domain-separated source-consumption commitment over the exact
  reconciled root/package/alias graph and every loaded package/toolchain source
  path and byte sequence. Absolute custody paths and source-load order are
  excluded. Every transitive source snapshot is mode-checked and re-hashed
  under its retained resolver limits before and after each package compilation;
  the compiler additionally re-reads every physical source against the bytes
  it retained, both before returning and after the resolver's post-check. This
  closes the missing compiler-consumption identity while preserving the stated
  hostile same-user race limitation. Whole compiler/toolchain commitments and
  CLI invocation remain; the result is still not sealed admission.
  The first concrete closure adapter resolves an explicitly named workspace
  member, requester-relative in-workspace Path rows, and Git rows. Each fetched
  Git snapshot becomes its own registered immutable workspace for nested Path
  rows. Absolute, nonportable, unknown-context, and escaping paths reject before
  target access; no parent-directory or protocol discovery occurs. A separate
  explicit external-local root adapter now resolves relative and absolute Path
  dependencies across directory boundaries under one supplied consuming
  context while preserving non-portable lineage. A contextual workspace adapter
  now routes an escaping live-workspace Path request into the same lane, while
  the context-free adapter still rejects and fetched Git snapshots can never
  escape. Deriving the context from the accepted lock and additional protocol
  adapters remain.

## P3 — Compiler-derived package evidence

- **PACKAGE-ADMISSION-COMPILATION.** Add a library/package compilation profile
  independent of executable entry selection.

  Acceptance: after successful checking, the compiler derives a total
  `PackageAdmissionProjection` for every public callable and build machine by
  joining each fact from its earliest semantically complete compiler-owned
  representation. It
  includes declared and realized reach, authority flows, provider realization/
  provenance, trust/claims, proof status, installation rows, operational
  contracts, executable TCB, observations, and reproducibility. Required
  unresolved or unprojectable facts reject. The canonical output contains no
  arena handles, display strings as identity, or other compiler-private IDs.
  Implementation may read each fact from the earliest coherent compiler-owned
  representation in which its semantics are established. Rows may join private
  pre-Psi structural identity to later checked facts after successful
  compilation and may otherwise use different internal representations. This
  coupling moves with the compiler and does not create a stable public IR stage
  or justify nominal Chi merely for stability. Unchecked syntax, diagnostics,
  or a merely convenient earlier shape are never admission evidence.

  Progress 2026-08-24: checked trees own much of the current semantic core and
  are one input to the target-scoped admission projection; other facts may come
  from earlier semantically complete compiler-owned representations. A
  `RealizedMachineContractEnvelope` retains contract identity, effective and
  concrete reach, unresolved installation rows, synchronous invocation,
  suspension, blocking, termination, crashes, mutation, and exact capability
  flows. Source-authored symbols can be joined back to their opaque
  `PackageKeyIdentity` through the retained source map, and underdeclared reach
  already fails checking. The resulting compiler-owned, target-scoped admission
  projection is not yet admissible. The unused `export path [as alias]` item is
  retired through
  parser, syntax identity/snapshot, visualization, and source-profile rows;
  `pub` is the sole package-owned API marker. Ordinary `pub machine` visibility
  is now retained through syntax,
  resolved, typed, checked, snapshot, copy, and specialization paths. Public
  omission is a strict empty ceiling for service reach, synchronous invocation,
  suspension, blocking, and crash; checked underdeclaration rejects. Machine
  body presence now survives symbol-resolved and typed copies instead
  of being reconstructed from synthesized states. Package review v41 reports
  `inferred_transitive` reach only for an actual checked body and records an
  explicit no-checked-body disposition for bodyless boundary, accepted,
  requirement, and external supply. It never relabels a published ceiling as
  realized reach. For checked bodies, compiler-classified dangerous services
  present in the declaration but absent from exact inferred reach emit
  callable-and-service-keyed audit-recommended slack rows with exact authority
  and callable source coordinates; bodyless supply and package-authored
  lookalikes cannot mint slack. Ordinary `pub data` visibility, including
  numbered data's wire identity and retired
  identities, likewise survives parsing, copies, snapshots, lowering, and
  generic specialization. Public trait visibility survives the same frontend
  and checked-tree path. Normalized package-owned public-data rows are now in
  the review projection: supply mode, lifetime arity, alpha-normalized type and
  const parameters, copy/carry properties, zero gating, retired identities,
  fields, variants, payloads, relevance, and package-qualified type identities.
  Review v60 and canonical row v18 give public data a closed ordinary/quotient
  discriminant. A quotient row binds the exact carrier-family type identity
  and package-qualified public relation declaration. Package review reruns the
  complete quotient-formation validator and requires exactly one formation
  fact rather than trusting retained typed quotient metadata. The relation's
  existing public-proposition row owns its telescope, body, and evidence
  classification; those facts are not duplicated in the data row. The selected
  `Equivalence` conformance licenses formation but does not enter quotient API
  identity, so switching valid proof implementations leaves the public-data row
  unchanged. Its authored selection is nevertheless retained as private
  implementation custody for package admission. The sealed core relation-law
  traits are public package-consumable toolchain declarations. Data
  declarations do not admit proposition parameters.
  Review v61 and canonical row v19 add exact raw byte-sequence literals to the
  public contract-expression vocabulary. Projection consumes the decoded
  octets retained by typed Psi without UTF-8 interpretation; equivalent source
  spellings such as `"A"` and `"\x41"` therefore have identical identity, while
  any changed octet changes canonical evidence. Unsupported aggregate and
  advanced call forms remain fail-closed.
  Review v62 and canonical row v20 close inherited requirement projection for
  lifetime-generic public conformances whose selected trait itself has no
  lifetime telescope. Declaring-trait arguments now apply the complete inherited
  type substitution before deriving alpha-normalized lifetime topology; binder
  renames and private realization bodies remain irrelevant, while selecting a
  different lifetime ordinal changes canonical evidence. Conformances targeting
  a lifetime-parameterized trait remain fail-closed pending the declaration-site
  application decision in `OWNER_QUESTIONS.md`.
  Review v63 and canonical row v21 close selected generic-conformance
  applications in public machine and trait bounds. Psi retains the authored
  nested application through syntax, resolution, typing, checked closure, and
  specialization. Review independently rejoins the exact public conformance
  declaration and encodes alpha-normalized lifetime arguments, categorized
  type/const/machine arguments, the instantiated subject, and the exact public
  trait plus its instantiated type arguments. Missing, extra, or category-wrong
  arguments reject during checked closure; non-public selections and
  proposition/evidence arguments reject before canonical evidence. Binder
  renames are stable, while changing any selected application argument changes
  review identity.
  Review v64 and canonical row v22 add proof-only `zero_value<T>()` to the
  public contract-expression vocabulary. Transparent propositions and other
  reviewed contracts retain the exact package-qualified, alpha-normalized
  target type rather than the diagnostic spelling or the layout result. The
  proposition resolver now stamps both the nominal family and its local binder
  arguments before typed lowering. Binder renames are stable, while changing
  the observed type changes canonical evidence. Quotient targets continue to
  reject at the existing representation-observer fence before review.
  Review v65 and canonical row v23 close outcome-specific `ensures` custody.
  Every public guarded guarantee remains an `ensures` fact and additionally
  retains its exact package-qualified result-data/result-case coordinate,
  public selector when named, and checked evidence-lane position. Projection
  requires exactly one matching producer-side checked guarded-guarantee row;
  missing, duplicate, or selector/case-mismatched carriers reject. Group and
  row ordering are canonicalized, while moving a fact to another case or
  renaming a named selector changes review identity.
  Review v66 and canonical row v24 close public-operator crash ceilings.
  Checked lowering retains one operator-symbol-keyed crash-contract row for
  every root and domain-homed operator, including an empty ceiling, and review
  requires the complete retained table to equal compiler rederivation. The
  operator row carries canonical cause buckets and exact structural guard
  expressions; package-qualified call/member/declared-overload identity is not
  replaced by the checked runtime predicate's textual fallback. Empty clauses
  and `true` subsume guarded alternatives, while clause order, guard order, and
  duplicates are stable. Changed cause or guard changes canonical evidence;
  missing, duplicate, or altered checked custody rejects.
  Review v67 and canonical row v25 add ordered array literals to the public
  contract-expression vocabulary. Each element recursively uses the same
  closed structural projector and encoding limits, so nested arrays are
  representable without weakening unsupported children. Element order and
  value changes alter canonical evidence; an unsupported nested expression
  rejects the complete public row.
  Review v68 and canonical row v26 add nominal record and sum-case constructors
  to the public contract vocabulary. Rows retain exact package-qualified data,
  optional case, and field identities plus recursively projected values.
  Fields canonicalize by exact identity rather than authored order. Changing a
  case, field, or value changes evidence; unresolved or nonmatching symbols
  reject, and the existing public-interface gate rejects private constructors
  before review.
  Review v69 and canonical row v27 add indexed and ranged contract expressions.
  The parser retains `[` as an authored operator token; checked lowering must
  finalize exactly one public-interface `Index` or `Range` selection before
  review records its builtin or declared meaning. Rows retain the collection,
  scalar index or optional range endpoints, and inclusive-end bit recursively.
  Changing an index, endpoint, or inclusivity changes canonical evidence;
  absent, ambiguous, or mismatched checked selection custody rejects. Indexed
  children now remain exact inside arrays instead of forcing the parent row to
  fail closed.
  A checked-custody follow-up keeps the v69/row-v27 byte format unchanged while
  closing public operator declarations: every non-crash `requires`/`ensures`
  fact now retains one exact `OperatorDeclaration` owner keyed by the operator
  symbol, and review requires that row before projecting the existing
  structural contract. Missing, duplicate, wrong-owner, wrong-kind, or wrong-
  fact custody rejects. This does not add named operator-contract syntax or an
  evidence lane; Omega's operator contracts remain the existing unnamed
  surface.
  Review v43 and
  canonical row v3 represent static-machine parameters directly. Structural
  contracts retain their complete alpha-normalized nested telescope, value
  signature, proof/crash contracts, reach, invocation, suspension, blocking,
  and termination envelope. Nominal contracts retain the exact public trait
  and requirement identities. Nested structural binders receive exact checked
  contract and crash custody; malformed depth, missing checked rows, and
  non-public nominal requirements reject.
  Review v59 and canonical row v17 add public data default-domain invariants.
  Data-clause fields and static parameters receive exact declaration identities
  during symbol resolution. Every typed `where` fact then retains one exact
  checked definition row and ownership record; each structural dependency
  retains its exact root and path. Before projection, the compiler rederives
  this data-invariant row family's complete evidence graph from final typed
  Psi—currently the earliest coherent owner of all of its ownership records,
  semantic rows, references, contexts, symbol indexes, and structural places—
  and requires structural equality with the retained checked graph. This is a
  property of the current row family, not a requirement that unrelated package
  evidence be reconstructed from final typed Psi.
  Review projects
  representable expression, membership, and proposition facts through the
  existing canonical contract vocabulary, sorts and deduplicates the invariant
  set, and includes it in public-data identity. Missing, duplicate, altered, or
  path-spoofed checked custody rejects. Source forms outside the compiler's
  checked default-domain fragment remain fail-closed rather than receiving
  speculative evidence.
  Review v44 and canonical row v4 extend this representation to contract-call
  static-machine arguments. Each retains either the exact caller machine-binder
  ordinal or the exact concrete machine entry identity.
  Review v45 and canonical row v5 rejoin each contract call to exactly one
  selected callee static telescope and preserve every supported argument's
  category: direct concrete type identity, parser-canonical integer const
  literal, caller machine-binder ordinal, or exact concrete machine entry
  identity. Nested static applications, forwarded or symbolic type/const
  binders, proposition/evidence static arguments, quotient calls, compiler
  intrinsics, and malformed or ambiguous joins remain fail-closed.
  Review v46 and canonical row v6 add bounded recursive generic data-type
  static arguments in contract calls. Each application base rejoins exactly one
  checked data declaration, whose telescope is recursively classified; a
  changed nested type therefore changes canonical evidence. This rung admits
  zero-lifetime generic data applications only. Lifetime-bearing applications,
  generic machine/conformance applications, unresolved forwarded type/const
  binders, proposition/evidence static arguments, quotient calls, and compiler
  intrinsics remain fail-closed.
  Review v47 and canonical row v7 admit lifetime-bearing recursive generic
  data static arguments in contract calls after an exact data-declaration
  lifetime-arity join. Lifetime arguments retain alpha-normalized caller
  lifetime-binder ordinals: renames are stable, while selecting a different
  lifetime changes canonical evidence. Generic machine/conformance
  applications, unresolved forwarded type/const binders, proposition/evidence
  static arguments, quotient calls, and compiler intrinsics remain fail-closed.
  Review v48 and canonical row v8 admit contract-call forwarding of caller type
  and const binders. Each argument is validated against the exact caller and
  selected-callee telescope categories and encoded by its alpha-normalized
  caller static-telescope ordinal: binder renames are stable, while selecting a
  different binder changes canonical evidence. The frontend now resolves
  const-parameter carrier types on machines and traits. Symbolic const
  declarations or expressions, proposition/evidence static arguments, true
  nested machine/conformance applications, quotient calls, and compiler
  intrinsics remain fail-closed.
  Review v49 and canonical row v9 admit public-trait proposition-family
  parameters with their mandatory declaration-site value signature. Each
  retains the ordered, package-qualified and alpha-normalized value-parameter
  types. Trait, proposition, and value-parameter binder renames are stable,
  while changing a signature type changes canonical evidence. Non-default
  `const`/`mut`/`self` value-parameter modes remain fail-closed because current
  proposition-family compatibility checking does not certify those modes.
  Proposition-valued or evidence contract-call static arguments remain
  fail-closed, as do symbolic const declarations or expressions, true nested
  machine/conformance applications, quotients, and compiler intrinsics.
  Review v50 and canonical row v10 admit unnamed public contract facts whose
  proposition endpoint is a containing proposition-family parameter. The fact
  retains the exact static-telescope ordinal and ordered, checked contract
  expressions supplied to that family. Static-binder renames are stable, while
  selecting another proposition-family slot or changing its value arguments
  changes canonical evidence. Compiler validation rejects named generic
  proposition evidence because the unresolved family has no exact witness
  interface; proposition-valued contract-call static arguments remain a
  separate incomplete form. Generic proposition law conformance now compares
  the exact normalized proposition declaration and structural application;
  rendered labels are diagnostic only, and a same-spelled foreign endpoint
  cannot discharge the selected law. This compiler result still does not become
  standalone package proof until it is carried by the total recheckable package
  evidence artifact below.
  Review v51 and canonical row v11 admit the four compiler-owned byte-sequence
  predicate calls in public contract facts. The checked authored-selection row
  now retains the exact closed predicate instead of one undifferentiated
  intrinsic tag; projection cross-checks that identity against an unresolved,
  receiver-free call before encoding it. Changing the predicate changes
  canonical evidence, while a package declaration with the same spelling
  remains an ordinary package-qualified callable. Other compiler intrinsics
  remain fail-closed.
  In particular, true nested machine static applications such as
  `consumer<family<Selected>>()` now reject during compiler validation, before
  checked lowering. Treating the argument as the uninstantiated `family`
  declaration checked the wrong callable shape; monomorphization also has no
  closed recursive application identity in conflict equality, specialization
  keys/fingerprints, or retained specialization evidence. Supporting this form
  requires recursive specialization plus exact declaration-telescope,
  lifetime, and static-argument identity throughout those paths. This is
  distinct from already coherent bare generic-machine selection and call-target
  use such as `Schema<Selected>(...)`.
  Package-owned public domains now project exact identity, alpha-normalized type
  and const parameters, carrier type, and index arguments. Synthesized domain
  paths retain their owned semantic spelling and exact authored package
  provenance independently. Closed compiler-owned classifications and
  authorized establishment routes now retain exact route kind plus
  package-qualified trait and requirement identities; alternative routes sort
  and deduplicate canonically. Transparent aliases recursively flatten to
  sorted, deduplicated package-qualified atoms. Review v41 encodes compiler
  carry aliases as closed `CarryPermission` atoms in a distinct tagged lane,
  never as nominal declarations with fictional toolchain ownership. Only an
  unresolved compiler-reserved constituent can enter that lane; a valid
  package declaration remains an exact package nominal even if its diagnostic
  path resembles `Carry::*`. Whole compiler/toolchain commitment remains
  separate. Predicate-body presence and the currently representable structural
  expression/membership facts now retain `self` as the domain carrier, exact
  package-qualified members/domains, and canonical fact ordering. Every typed
  fact must join exactly one checked definition row and one fact-keyed checked
  ownership record; nested member paths must join exact checked dependency
  places, so missing, duplicate, wrong-origin, and member-spoofed rows reject.
  Proposition-shaped predicate applications use their exact checked rows. A
  simple total, pure callable predicate application now retains its optional
  receiver, exact checked package-qualified entry target, and ordinary
  arguments after joining exactly one public-interface declaration-selection
  row. The separate source-consumption commitment pins the helper body; the
  call row does not confuse signature identity with implementation identity.
  Symbolic const declarations or expressions, proposition/evidence static
  arguments, quotient calls, true nested machine/conformance applications,
  and other compiler-intrinsic calls still reject until their exact canonical
  rows are settled. Public domain semantic roles now project from the exact
  typed declaration as a closed role set. The projector requires every retained
  role to name that declaration's own semantic identity, then persists the
  package-qualified domain plus the closed role tag rather than the private
  semantic-domain ID. Public domain operators remain independently covered by
  the exact `PublicOperator` rows.
  Package-owned public traits now project exact identity, boundary status,
  alpha-normalized lifetime/type/const binders, ordered package-qualified
  parent edges, and ordered machine/operator requirement signatures including
  parameter names/modes, package-qualified types, fixed operator spelling,
  exact declared service reach, installation-bound status, synchronous
  invocations as exact non-`self` parameter ordinals or package-qualified
  services, suspension, and blocking. Trait and requirement lifetime arity is
  explicit; parent lifetime arguments and lifetime topology inside public
  data/requirement types are
  retained as alpha-normalized binder ordinals independently from runtime type
  identity. Renaming a lifetime is stable while changing a borrow relationship
  changes canonical review evidence. Termination guarantees retain exact
  package-qualified public progress profiles, receiver/non-`self` parameter
  roots, and package-qualified field projections.
  Generic conformance requirements on public traits now retain an optional
  alpha-normalized evidence-binder ordinal, exact subject ordinal, package-
  qualified public trait identity, and structural type arguments. Binder-free
  `where T satisfies Trait` rows remain explicitly binder-free rather than
  fabricating evidence. The frontend now preserves
  these settled trait-header binders and assigns them real conformance-
  parameter symbols instead of discarding them. Non-generic selected
  conformances retain exact package-qualified conformance, carrier, and
  underlying public-trait identities plus carrier/trait applications. Their
  declarations retain exact carrier and trait symbols instead of reselecting
  either by text. Callable rows retain the exact checked-body, boundary, or
  accepted supply tier: a bodyless boundary guarantee remains an explicit
  trust-bearing accepted claim, while a claim-free boundary symbol does not
  become one. Canonical review now emits an additional blocking
  `AcceptedClaim` row for each exact accepted callable, carrying its complete
  published envelope and declaration provenance. This keeps trust admission
  distinct from ordinary callable compatibility without reconstructing claim
  semantics in package orchestration. Public trait requirements now project
  named and unnamed `requires` and `ensures` through the same closed structural
  fact/expression and evidence lane as public callables, joined to exactly one
  checked `StateSignature` owner. Named inputs retain ordered proposition and
  evidence-interface identity while treating their source aliases as local;
  named outputs additionally retain their public selector identity. Generic
  selected-conformance applications retain their exact declaration, complete
  alpha-normalized lifetime/static telescope, instantiated subject, and exact
  public trait application. Proposition/evidence application arguments and
  unsupported expression forms still fail closed. Trailing `boundary host` / `boundary
  Name` clauses are retired at the source grammar rather than awaiting a
  package row. Exact
  checked crash capsules keyed by trait and requirement now project each
  abstract published crash ceiling as canonical cause-and-guard routes without
  fabricating realized body sites or calls; no public trait is silently omitted.
  Declaration kinds without retained visibility reject
  `pub` instead of silently compiling a private API. The remaining
  advanced call-bearing domain predicates, semantic-role/operator lanes,
  source-free compiler-semantic subjects beyond the closed builtin type atoms
  and collection-length contract projection,
  compiler-intrinsic provider-binding ownership, exact semantic-
  subject commitments, receipted build-operation transcripts, staged-output
  commitments, certificate closure, and reproducibility verdicts still need
  one recheckable projection. Direct nominal projection now follows mandatory
  derivation provenance for generated package and toolchain symbols while
  keeping genuinely source-free symbols unresolved. Structural type identity
  now uses the same exact SHA-256 toolchain source owner rather than collapsing
  every source-backed toolchain nominal to a generic marker; private `SourceId`
  joins do not enter review bytes, and a missing join rejects package review
  instead of degrading to the generic marker used by weaker local identity.
  Review v41 also assigns every one of the 22 compiler-installed source-free
  builtin types a closed atom selected by exact root slot and `BuiltinType`
  kind, never by spelling. Same-named package declarations and source-free
  generated symbols remain unresolved rather than inheriting compiler identity.
  Carry permissions likewise use closed enum atoms rather than nominal-owner
  placeholders.
  Audit 2026-08-25: arithmetic domains and aggregate carry policy are already
  retained as closed enums and encoded through exhaustive tags; their remaining
  diagnostic-name rendering is schema cleanup, not package-controlled
  authority. Typed domain constraints now retain a closed subject distinguishing
  declared domains, compiler carry permissions, value domains such as `Finite`,
  and `OmegaLayout` with a closed grammar; the layout schema remains an exact
  structural type argument with its declaration symbol. Symbol-backed package
  declarations always normalize back to `Declared`, regardless of a resembling
  diagnostic spelling. The subject survives type copying and typed snapshots.
  Review v41 encodes these compiler domains structurally and binds an
  `OmegaLayout` schema through package-qualified type identity. It rejects
  unclassified or legacy flattened layout spellings, malformed compiler-domain
  shapes, residual unevaluated const calls, unsupported index expressions, and
  missing, duplicate, or incomplete checked open-index selections instead of
  serializing fallback text. This uses the existing checked compilation seam
  and does not justify nominal Chi. Review v41 additionally decodes the
  compiler-reserved, source-unspellable canonical-const transport atom into a
  closed type-and-value term, omits diagnostic display text, and encodes decimal
  const leaves numerically rather than as unresolved nominals. Such values are
  admitted only in an exact declared const-parameter slot. Fixed-array and open
  expression binders must reconcile uniquely to the exact projected telescope
  and become alpha-normalized ordinals; residual const declarations and other
  source-spelled leaves reject. The legacy atom remains internal transport and
  never enters review bytes. Review v42 and canonical row v2 separate concrete
  proposition type arguments from machine-declaration arguments: types use the same exact
  structural identity, including closed compiler builtin atoms, while machines
  remain exact owned declarations. Any unresolved nominal owner now rejects
  both exact structural type projection and final canonical encoding; it is no
  longer serializable review evidence. Any remaining source-free compiler
  semantics still need equivalent closed treatment.
  Exact
  provenance for each provider schema, provider type, requirement declaration,
  and realizing machine is retained from derivation through review; readable
  plan and overload strings are not declaration identity. Canonical
  `AcceptedClaim` rows already close non-provider claim ownership under exact
  package identity. The legacy trust-lock path no longer creates authority from
  domain names, unmatched strings, or FNV statement fingerprints, and domain
  declarations no longer appear as grantable trust-report rows. Exact selected-
  provider grants remain; exact accepted-machine grants remain temporarily for
  standalone compilation only. Package-aware compilation rejects an individual
  accepted-machine grant because package claim admission must cover the complete
  exact `AcceptedClaim` inventory. The remaining v1 standalone receipt parser
  and accepted-machine compatibility lane must not be promoted into package
  evidence.
  Until the remaining joins exist, no projection may be persisted as accepted
  evidence. The compiler now exposes
  an explicitly review-only, in-memory projection for the reconciled root
  package under an exact target. It includes package-owned boundary and ordinary
  public machines plus the selected build machine while excluding private
  machines. Each callable row retains its exact canonical entry signature:
  lifetime arity, alpha-normalized type/const parameters, ordered parameter
  names and modes, package-qualified lifetime-sensitive parameter types, and
  result type. Lifetime and generic binder renames compare equal; changed type
  or borrow relationships compare unequal. Checked realizations of public,
  ordinary, lifetime-free traits retain exact package-qualified trait and
  requirement identities, alpha-normalized arguments, and optional conformance
  alias. Binder-free generic requirements, explicit conformance evidence
  binders, and selected conformances with representable complete applications
  on reviewed callables use the same canonical row as public traits. Selected
  proposition/evidence application arguments, non-public or lifetime-
  parameterized trait realizations, and operator realizations outside the
  checked public nongeneric lane reject until their complete rows exist rather
  than disappearing from review.

  Milestone 2026-08-26: review v71/canonical row v29 retains every public
  checked-body callable's exact unaliased realization of a public ordinary
  nongeneric, lifetime-free operator declaration, with or without a fixed
  token. Checked lowering retains the exact machine/operator
  symbols, conformance/admission form, normalized overload shape plus exact
  lifetime-bearing type nodes, both complete
  canonical contract sets, and exact typed semantic snapshots of the contract
  graphs in full, not as a hash. Projection requires exact equality with a
  fresh derivation, then reruns the compiler's signature-directed resolver and
  equality/`&&` `requires`/`ensures` contract-coverage judgment before
  joining the selected declaration's existing package-qualified overload
  coordinate into the callable value. Post-check redirection and coordinated
  mutation of the provider contract therefore both reject. Changing only a
  valid selected operator changes only that callable row. Private, generic/
  lifetime-parameterized, aliased, bodyless, and externally supplied operator
  realizations remain explicit fail-closed forms. A fixed-token realization
  uses the same exact declaration coordinate as the named call surface; the
  public-operator row, not the callable realization edge, owns the closed
  compiler spelling. Positive projection canaries join those rows without a
  duplicate package name or a new encoding field. Checked-body boundary
  realizations now use that same edge only for contract satisfaction; the
  existing selected-provider set separately names the active target plan,
  exact operator requirement, and realizing machine. A named-boundary canary
  joins all three rows under unique covering selection, and projection repeats
  that exact symbol, slot, binding, package, and machine join. Thus an
  unselected candidate is not mislabeled as selected and provider identity is
  not duplicated into the callable. Fixed-token boundary operators remain
  fail-closed until checked-adapter token dispatch exists. Authored override of
  a same-path overloaded boundary-operator family remains blocked on OWNER Q10.
  Operator-bound external supply still needs its own trust-bearing association.
  This is compiler-private
  retained checked baseline, not a persisted review row or a reason for nominal
  Chi. Trusted compiler components remain inside the TCB; this comparison does
  not claim to resist a component that rewrites both typed state and checked
  facts. Operators with outcome-specific or crash contracts, and providers with
  any nonempty checked crash behavior, reject until checked operator refinement
  covers those clauses.
  Public callable `requires` and `ensures` retain exact structural rows for the
  closed boolean/integer expression subset over
  parameter ordinals, `result`, generic binders, and package-qualified
  nominals. Domain-membership rows additionally retain the exact value and
  package-qualified public domain; exposing a private package domain rejects.
  The projection reads the earlier typed semantic tree only after checked
  compilation succeeds. Proposition applications now retain their exact
  package-qualified primitive endpoint, alpha-normalized binder schema,
  parameter types, structural binder/value arguments, and fact-only or witness
  classification. Transparent proposition aliases expand without minting
  identity. Witness interfaces retain exact package-qualified root arguments
  plus direct and inherited requirement surfaces. Named contracts join the
  exact checked evidence term and positional lane; local `requires` aliases do
  not enter canonical identity, while public `ensures` selectors do. Checked
  diagnostic strings are ignored and adversarially tested. A proof-static
  `evidence.member` binder argument retains the source named-`requires` lane,
  exact package-qualified declaring trait, structural requirement-argument
  template, and exact requirement. The source lane binds that template to the
  source proposition application's concrete arguments. The local evidence
  alias is deliberately absent. Projection requires matching checked evidence-
  term, interface, and projection facts; checked display strings remain
  diagnostic only. Direct parameter-rooted member paths in ordinary public
  contracts retain the receiver ordinal plus each exact package-qualified case
  and field symbol, joined to one checked semantic-place row. Changing only the
  receiver therefore changes review identity. Computed members, proposition-
  argument members without that checked join, unsupported advanced call
  forms, and aggregate expression forms still reject rather than falling back
  to text or a hash. Contract casts retain their structural operand, alpha-normalized
  target type, exact arithmetic policy, package-qualified semantic domain and
  arguments, and value/recast form. Diagnostic target/domain spellings are
  absent, and a private package domain cannot leak through a public cast.
  The settled proposition path is a join, not a new nominal stage: typed trees
  supply structural declaration, binder, and value-expression coordinates;
  checked proof facts supply acceptance, evidence-term/interface routing, and
  the eventual proof/admission disposition. Existing checked `String` fields
  are diagnostics only. Missing structural witness-interface arguments must be
  retained in their current owning representation before projection can accept
  them; they must not be parsed back from display text.
  The legacy 64-bit
  machine-contract fingerprint has left package-review bytes, so private
  state-machine shape no longer contaminates public package contract identity.
  Ordinary standalone checked compilation continues to accept a caller-owned
  writable build root. Package review instead receives a package-specific root
  inside the orchestration-owned disposable child session, so it never mutates
  a resolver-owned immutable source snapshot or publishes results before
  successful session cleanup.
  Selected build-machine execution now separately retains a versioned static
  observation ceiling and realized class. Exact statically reachable canonical
  toolchain filesystem use has a `Volatile` ceiling because the current scoped
  real provider emits no replay transcript; an actual filesystem call realizes
  `Volatile`, including a denied attempt, while pure, console-only, and declared-
  but-unreachable filesystem rows remain `Hermetic`. Console-only granted
  execution no longer installs real filesystem authority. Compiler-issued
  package review carries this summary outside v42 capability/API comparison
  bytes. It is explicitly not a receipt and makes no replayability or source-
  rebuildability claim.
  Exact rows for the unsupported forms and proof/admission dispositions still
  gate sealing.
  It retains
  package-qualified authored nominals, distinct declared/effective/concrete
  service rows, unresolved installation rows, exact capability-flow
  coordinates, operational outcomes, crashes, mutation, and selected provider
  mechanisms with exact realizing-package, provider-type, service-schema, and
  requirement-owner provenance. Checked-adapter bindings now retain and verify
  canonical overload plus realizing-package identity. Authored provider choices
  remain two structural paths through parsing and resolve to exact typed
  boundary-trait/provider-data symbols. Selection matches only exact package
  plus canonical-path identities; same-spelled cross-package slots/providers
  remain distinct and no leaf-name fallback remains. The exact selected plans
  survive cycle, ABI, and checked-fact construction without a name-based
  candidate rejoin. Package review now cross-checks every selected schema,
  optional provider type, requirement, and realizing declaration against the
  exact package owner carried by the selected plan, or against an exact authored
  toolchain-source identity where the plan has no package owner. Mismatched,
  package-less user, and unresolved/source-free ownership rejects. Remaining
  grant joins and whole-compiler/compiler-intrinsic toolchain identity are not
  yet fully sealed. Selected-provider provenance additionally retains the exact
  requirement declaration for every row and review v41 encodes exact nominal
  identities for the schema, optional provider type, each requirement, and each
  realizing machine. Build-bound progress
  obligations now retain and match the compiler-derived package owners of both
  the provider service and exact requirement, including through component
  manifests and audit rendering; no readable-name lookup remains on retained
  selected-provider facts. Installation-bound reach, termination premises,
  mutation frames, crash sites/calls, and permission frontiers now project
  package-owned semantic paths rather than arena-local handles/row IDs. Crash
  predicates retain only their existing source-independent canonical identity.
  Source-free structural children now inherit authored hierarchy provenance,
  closing the implicit-entry-state ownership gap discovered by the crash
  projection. Review identity now retains the exact deployment target profile,
  so profiles such as Windows and UEFI cannot collapse merely because they
  share a native ABI. A v41 length-framed binary comparison encoding now covers
  every retained public-domain, public-data, public-trait, callable,
  representation-TCB, crash/proof predicate, proposition/witness, authority
  flow, dangerous-authority classification, mutation, and selected-provider
  row. Public trait requirements retain named and unnamed structural
  `requires` and `ensures` rows plus whether the declaration supplies a default
  realization; named inputs treat source aliases as local while named outputs
  retain public selector and evidence-interface identity;
  changing either the public contract or body presence changes comparison
  identity without serializing compiler-private body IR. The checked body must
  satisfy the retained requirement envelope, instantiated uses contribute
  ordinary compiler-derived evidence, and every source update remains subject
  to source triage.
  It converts platform-width ordinals to portable `u64`, distinguishes exact
  deployment profiles, rejects interner-backed external-supply variants, and
  remains explicitly review-only rather than persistable admission evidence.
  It also covers declared and realized synchronous invocation as either exact
  non-`self` parameter ordinals or package-qualified service symbols; checked
  display strings never become comparison identity. Capability-flow state and
  propagated `via` state identities are package-qualified instead of display
  strings. Independent source-and-artifact obligation reconstruction, exact
  certificate checking, transitive open-obligation disclosure, and the
  remaining projection joins still gate sealed evidence; blanket Terminal
  coverage and producer pedigree do not.
  Compiler-generated symbols now inherit the exact authored provenance of a
  mandatory derivation origin. Authored toolchain symbols retain exact source
  commitments; truly source-free symbols remain visibly unbound rather than
  guessed. Review orchestration now separately commits to the exact producer
  executable file bytes observed before and after closure review, rejects a
  changed observation, and retains the same commitment on every review row.
  This is provenance outside canonical capability/API comparison bytes, not
  certification, complete compiler source identity, a reproducible-build
  receipt, or proof of the process image already loaded by the operating
  system. No completion of that provenance promotes review rows into package
  evidence.
  Standalone and target-free compilations reject projection.
  Package orchestration now invokes this path for every package in the resolved
  closure rather than projecting only the requested root. It re-roots compiler
  inputs to the package's exact transitive closure and retains the selected
  immutable resolution beside compiler-issued comparison bytes. These rows are
  deliberately and permanently review-only and cannot construct an accepted
  package instance.

- **BUILD-OBSERVATION-EVIDENCE.** Replace the conservative filesystem-touch
  summary with canonical replay evidence before issuing `Receipted`.

  Acceptance: interpreter dispatch selects a closed operation identity from the
  exact canonical toolchain signature before authority is touched; package-
  controlled type or method names cannot select a host operation. One ordered
  transcript retains every attempted operation, rooted relative path, scalar
  operand/result, input/output byte region, mutable cursor/result, logical
  handle identity, and post-operation error state, including failures and grant
  refusals. A successful run additionally commits to the complete staged output
  tree. Canonical evidence contains no cache/build absolute path or lossy path
  conversion. For package review, each staging root is fresh and empty inside
  the orchestration-owned disposable child session. Orchestration retains the
  review result privately, cleans up the complete child session, and only then
  returns it; cleanup failure rejects. Absolute-path-returning operations remain
  `Volatile` unless
  served through a stable virtual root. `Receipted` requires retained content
  plus a replay executor that rejects the first missing, extra, reordered, or
  changed event and reproduces the staged tree; a transcript alone is not a
  replay verdict.

  Progress 2026-08-24: a dedicated measured build-machine result keeps the
  realized filesystem-touch fact separate from deterministic evaluator usage.
  The compiler derives the static class from exact reachable toolchain service
  identity, retains the versioned Hermetic/Volatile summary through checked and
  full reports and compiler-issued package review, and keeps it outside v41
  capability/API comparison bytes. Console-only execution no longer installs
  real filesystem authority. Both statement- and value-position filesystem
  dispatch now require an exact requirement symbol owned by the canonical
  toolchain `filesystem_host.omg`; an explicit granted mode cannot activate a
  package-authored trait or same-named method. After that authority selection,
  the readable leaf maps into a closed 50-operation compiler enum with explicit
  append-only tags; virtual and real providers exhaustively match that enum, and
  a source-surface test rejects drift from the canonical Omega trait. Alias and
  platform operations retain distinct identities. An exact canonical signature
  lacking an encoded identity rejects rather than falling through to another
  boundary dispatcher. `read_link` is recognized as
  conditionally absolute-path-producing; `canonicalize` and
  `final_path_name_by_handle` are unconditionally so. Observation-summary
  schema v20 carries operation-attempt schema v18: an ordered successful-run call-start
  trace of exact provider, operation tag, normalized result, post-operation error
  state, and every direct scoped path authorization through compiler reports
  and package review. Each authorization retains exact operand ordinal,
  read/write access, closed Source/Output root identity, and canonical
  slash-separated root-relative UTF-8 bytes; physical compiler/cache paths do
  not survive. Nested output under source selects the most specific root.
  Duplicate root identities, one physical root with conflicting identities,
  unresolved roots, and unrepresentable rooted paths reject before host access;
  retained rooted-path bytes have a 16 MiB aggregate evaluator ceiling whose
  exhaustion non-catchably halts evaluation before host access.
  Grant-gate denials retain every exact operand ordinal, access, and closed
  reason, including both operands of a two-path operation. Host OS errors carry
  no fabricated refusal but retain any authorization that preceded the host
  failure; pure and console-only traces are empty.
  Every successfully typed descriptor/handle operand is immediately normalized
  as a Descriptor, Native, or Find lifetime with an exact operand ordinal and a
  closed Resolved/Null/Unknown disposition. A later argument or preparation
  failure retains that prefix; a fully prepared call must reproduce the exact
  logical-handle plan before provider access. Successful opens mint monotonic
  logical identities independent of provider tokens; duplicate outputs retain
  their source lifetime, `_get_osfhandle` outputs retain a borrowed source, and
  successful closes retain every invalidated lifetime. Provider-token reuse
  after close mints a fresh identity, failed closes retire nothing, repeated
  borrowed conversion preserves one alias, and a provider that successfully
  accepts an otherwise Unknown token non-catchably traps rather than publishing
  contradictory lineage. A raw token live in another logical domain rejects
  before provider access. The virtual duplicate provider now shares the source
  cursor as the canonical contract requires. A real descriptor also retains its
  rooted write grant across duplicates and borrowed native views; writes,
  extent/metadata changes, ownership changes, and host-visible locks reject
  before sponsor or host access when the origin was admitted only for source
  reads.
  Successful descriptor, native-handle, and find-handle results retain only the
  minted logical identity; provider token integers do not survive into compiler
  or package evidence. Non-handle results and failed handle-result sentinels
  remain exact scalar values. Package commitments type-tag the two result lanes.
  `open_at`/`unlink_at` names reject before provider/grant access unless they are
  one nonempty portable component, and real-provider path outputs no longer use
  lossy host-string conversion.
  Operation-attempt schema v18 retains each successfully typed non-handle
  scalar and immutable payload immediately as the argument cursor advances.
  If a later argument or preparation constraint halts, the failed attempt keeps
  that exact ordinal-ordered prefix; byte evidence consumes the same aggregate
  sponsor incrementally, before any provider access. Fully prepared calls
  cross-check the incremental rows against the canonical call projection
  before mutable pre-state is admitted.
  Every fully prepared call whose evidence reservation succeeds retains
  ordinal-ordered non-handle scalars with exact I32/U32/I64/U64 width and
  signedness. Immutable write/FILETIME payloads retain exact authored bytes,
  including trailing bytes beyond the provider's minimum read, while validated
  `open_at`/`unlink_at` components retain their exact portable spelling. Raw
  rooted/path-alias spellings never enter this payload lane. A distinct rooted-
  path resolution lane retains each successfully resolved operand's exact
  ordinal, closed Source/Output identity, and canonical relative bytes before
  physical provider-path lowering. It survives later preparation failure and
  is cross-checked exactly against a compiler-private semantic sidecar on a
  fully prepared call. It is not authorization evidence: later grant checking
  adds access and may select a different canonical rooted location after
  symlink or nested-root resolution. Each mutable byte
  or i64 carrier retains a distinct complete resolution-time snapshot as its
  operand is evaluated, so a later preparation failure keeps the prefix.
  Mutable byte carriers separately retain their complete capacity before and
  after the provider call, including unchanged tails; mutable i64 carriers
  retain exact provider pre/post values. Provider pre-state is captured only
  after every authored argument has been evaluated, because a later argument
  may alias and mutate an earlier carrier; the resolution and provider snapshots
  are deliberately not required to match. Post-state follows provider return or
  provider halt. Input-only ABI carriers remain explicit even when unchanged. A
  separate 256 MiB aggregate operand-evidence sponsor reserves immutable,
  path-like, rooted-resolution, and exact returned-path bytes, one mutable
  resolution copy, and both provider copies of every mutable byte carrier;
  exhaustion non-catchably halts that call. Prior or nested
  staging effects remain cleanup-contained rather than being denied
  retroactively. Package review commitments frame scalar tags, ordinals,
  rooted identities, relative/immutable bytes, and mutable states exactly and
  never render payload bytes as text.
  Exact directory-entry names, symlink targets, find patterns, and the other
  path-like byte operands not represented by rooted authorization now occupy a
  distinct ordinal-tagged lane. They consume the same aggregate byte sponsor,
  are retained as preparation advances, survive a later preparation halt, and
  are cross-checked against the fully prepared call; they never masquerade as
  rooted grant paths or immutable payloads. Successful provider write branches
  separately retain the exact meaningful bytes of `read_link`, `canonicalize`,
  and `final_path_name_by_handle` outputs, excluding NUL terminators and stale
  carrier tails. Rows retain exact output ordinal, closed operation-derived
  kind, and Complete/LimitReached disposition; provider-known target length
  distinguishes exact-fit from truncated `read_link`. Failures and final-path
  insufficient-capacity returns emit no output row. Capture occurs where the
  provider knows what it wrote rather than by scanning mutable post-state, and
  charges only the exact retained bytes to the aggregate sponsor. Package-rooted
  execution still rejects the two always-absolute operations; a
  `read_link` result remains inert payload and grants no path authority.
  Successful `read` and `read_at` calls now add one exact semantic designation
  over the already-custodied mutable post-carrier: output ordinal, closed
  `SequentialFileRead`/`PositionedFileRead` kind, zero offset, and exact
  returned length. The region length must equal the nonnegative scalar result and fit the
  retained post-state; successful EOF retains a zero-length row, while failure
  retains none. The bytes are recovered from that post-state rather than copied
  a fourth time, so the row consumes no additional byte sponsor. Successful
  `read_dir` uses the same rule with a closed `DirectoryRecords` kind. Successful
  `find_first` and entry-producing `find_next` designate the complete fixed
  320-byte output record as `FindEntry`; a no-entry `find_next` and directory EOF
  retain zero-length rows, while failed `find_first`/`read_dir` retain none. All
  regions are cross-checked against operation result, kind, output ordinal, and
  retained post-carrier before a returned outcome is admitted. Successful
  `read_metadata`, `read_file_metadata`, and `read_symlink_metadata` calls now
  retain one target-neutral canonical metadata row with exact follow/open/
  no-follow meaning and all 14 `StatRecord` fields. The compiler extracts the
  selected target's already-checked `StatLayout<StatRecord>` from its earliest
  coherent private typed/layout state, validates exact fields, widths, bounds,
  and non-overlap, then gives only that closed descriptor to the Psi evaluator.
  The evaluator serializes through that descriptor, zeroes the complete
  authored ABI carrier (whose API minimum is 144 bytes), and cross-checks the
  carrier against the canonical row.
  Package commitment binds both the semantic row and target-specific carrier.
  A filesystem-reaching build therefore loads and checks the standard
  filesystem layout policy before execution; console-only execution does not
  require it. This does not expose the private IR or introduce nominal Chi.
  The trace still lacks a complete replay executor and makes no receipt claim.
  Milestone 2026-08-25: the first bounded replay executor consumes exactly one
  successful Source-rooted, flags-zero `open` -> `read` -> `close` chain. It
  installs no virtual or real filesystem provider, lowers rooted values only
  to inert replay coordinates, supplies recorded results and mutable output
  bytes, reconstructs
  logical descriptor lifetimes, rejects the first extra, reordered, changed, or
  missing event, and requires exact result and complete-record equality. Summary
  v19 binds whether this replay succeeded. Milestone 2026-08-25: compiler
  replay-record v1 now canonically retains every lane of that verified chain,
  requires the exact observation and operation schemas, and rejects
  operation-inapplicable rows, inconsistent source authorization, descriptor
  lineage, transfer counts, observed regions, and mutable-carrier state. Review-
  baseline capsule v2 preserves those opaque bytes across restart under an
  aggregate capsule budget and binds their commitment to the parent build-
  observation commitment. Compiler recovery remains explicitly review-only;
  the checksum and parent association detect inconsistent custody but prove
  neither authenticity nor review. Milestone 2026-08-25: checked compilation
  can now strictly decode that reopened record into the PSI executor's exact
  typed three-event replay object and evaluate the selected build machine with
  no host filesystem provider. Recorded source bytes remain authoritative for
  that build call even if the host file has changed; a changed authored path,
  count, event order, output, or event count rejects through the existing exact
  replay checks. The record grants no ambient host authority and still proves
  neither provenance nor admission. The build remains `Volatile`: other
  operations, output mutation replay, staged-tree reproduction, package-command
  integration, and a complete replay verdict remain open. This uses the
  compiler's existing coherent checked-entry/evaluator seam; it does not expose
  a public IR contract or justify nominal Chi.
  Milestone 2026-08-26: the same closed rung now also accepts exactly one
  `open` -> `read_at` -> `close` chain. Summary v20 distinguishes the exact
  positioned operation, count, signed nonnegative file offset, positioned
  region kind, returned length, and complete mutable carrier; replay-record v2
  strictly rehydrates either the sequential or positioned form. Reopened
  positioned replay supplies retained source bytes without a host provider.
  Changed authored offsets and relabeled operation/region kinds reject. This is
  still one read-family event, not broad filesystem replay or a `Receipted`
  claim.
  Milestone 2026-08-26: the closed rung now accepts one Source-rooted,
  flags-zero `open`, one or more ordered `read`/`read_at` calls on that exact
  descriptor, and one retiring `close`. Summary v21 binds the broadened partial
  replay fact and replay-record v3 strictly retains every read in order.
  Sequential reads advance an implicit zero-based cursor by their exact
  successful result; positioned reads retain a nonnegative offset and do not
  advance that cursor. No cursor field is trusted or persisted because the
  ordered operation kinds, counts, positioned offsets, results, carrier states,
  and observed regions determine it completely. Empty sequences, failed reads,
  another descriptor, and any non-read middle operation reject. EOF rows remain
  exact. Canonical recovery and provider-free reopening support mixed sequences
  without changing operation schema v18, package review v69/row v27, or review-
  baseline capsule v2. This is still review-only, `Volatile`, and neither broad
  filesystem replay nor a `Receipted` claim.
  Milestone 2026-08-26: the same rung now accepts one or more complete,
  non-interleaved source-read chains. Each chain owns one distinct created
  descriptor, one Source-rooted flags-zero `open`, one or more ordered
  `read`/`read_at` calls, and its exact retiring `close`. Summary v22 binds the
  plural replay fact and replay-record v4 canonically retains every chain in
  order. Each chain starts its own sequential cursor at zero; positioned reads
  remain cursor-neutral. Descriptor identity reuse across chains, cross-chain
  reads or closes, interleaving, reordered/removed chains, and trailing
  incomplete chains reject. Provider-free reopening can now serve multiple
  retained source files. Operation schema v18, package review v69/row v27, and
  review-baseline capsule v2 remain unchanged; the result is still review-only,
  `Volatile`, and not a complete filesystem replay verdict.
  Milestone 2026-08-26: bounded replay is now an ordered source-input event
  stream rather than a read-chain-only list. In addition to the existing
  closed descriptor chains, it accepts successful Source-rooted
  `read_metadata` and `read_symlink_metadata` events before, between, or after
  chains. Each event retains the authored rooted path and separately resolved
  authorized target, exact follow/no-follow kind, all 14 target-neutral
  metadata fields, target-specific mutable resolution and provider pre/post
  carrier, scalar result, post-error state, and otherwise-empty operation
  lanes. Recovery rejects wrong kinds, roots, ordinals, noncanonical relative
  paths, lane shape, or undersized carrier structure. Provider-free replay
  reconstructs the complete zero-filled carrier from the semantic row and
  selected checked `StatLayout`, compares every field, padding, and tail byte,
  and requires exact event/result exhaustion.
  A followed symlink canary proves that authored and authorized paths remain
  distinct without leaking a host path. Failed metadata and descriptor-backed
  `read_file_metadata` remain outside this rung. Observation summary v23 and
  replay-record v5 bind the extension; review-baseline capsule v2 needs no
  framing change because it already treats the embedded versioned record as
  opaque bounded custody. The fact remains review-only and `Volatile`.
  Milestone 2026-08-26: observation summary v24 and replay-record v6 admit the
  first complete operation-replay grammar: one or more existing Source-input
  events, then exactly one Output-rooted direct-child ordinary-file
  `create(438)` -> full immutable `write` -> retiring `close`, followed by
  exactly one matching `include_source` handoff. No other filesystem event or
  output shape is inferred into this rung. Source events are served from the
  retained record, but the Output chain executes against a fresh virtual
  namespace. Replay requires exact operation/result/evidence equality, exact
  handoff, no live descriptor or extra virtual state, and the exact resulting
  path and bytes. The compiler independently reconstructs the one-file
  canonical tree from those executed operands. An initial sponsored run
  becomes `Receipted` only when that tree equals the separately captured
  physical staged tree. An unsponsored run cannot publish this record.
  Reopened v6 custody executes the same no-host replay and reconstructs the
  generated source without consulting changed host Source or Output bytes. The
  static filesystem ceiling remains `Volatile`; only this realized closed
  grammar is `Receipted`. Mode, path, payload, lane, descriptor, handoff, and
  event-order mutations reject. Review-baseline capsule v2 and operation schema
  v18 remain unchanged. Replay admission applies a separate 16 MiB aggregate
  retained-evidence ceiling before cloning, and validated attempt custody is
  shared across evaluator handoff. Broader output trees and filesystem
  operations remain outside this verdict.
  The granted evaluator's structured failure now retains partial usage and
  operation evidence, with each active call explicitly `Returned` or
  evaluator-halted rather than represented by placeholder zeroes. Worker
  creation/panic marks evidence unavailable. Omega emits fixed non-admission
  attempt/halt/refusal counts in the failure diagnostic and issues no package
  review row. A failure can follow an earlier staging side effect, so the fresh
  disposable child session is always cleaned up and no result is published
  unless cleanup succeeds. This evidence is not
  called a transcript or receipt. A shared canonical preparer now checks exact
  arity before evaluating anything, consumes every authored operand once in
  left-to-right order, and produces one closed typed call covering all 50
  operations. This includes ABI operands a provider does not otherwise use.
  Scalar and byte kinds reject instead of becoming zero/empty; mutable byte and
  scalar places resolve once and remain the same cells for input/output; count-
  bearing and fixed-layout inputs/outputs reject inadequate capacity before provider
  or grant access. Both virtual and real providers accept only prepared calls.
  Preparation failure remains attached to the outer operation attempt, and
  executable real-scoped canaries prove ignored-operand traps and invalid
  outputs occur before disk mutation or grant consultation. The canonical trait
  test pins each operation's exact operand order/kind and result width.
  Sponsored package review now commits the complete fresh Output tree after a
  successful evaluator has released its provider and descriptors and before
  cleanup-gated publication. The canonical tree includes sorted root-relative
  portable UTF-8 paths, empty directories, canonical directory/ordinary/
  executable/symlink modes, file lengths and content digests, and validated
  self-contained relative symlink spelling. It excludes ambient metadata,
  physical roots, inode identity, and hard-link topology. Capture requires a
  quiescent sponsor, cross-checks its namespace kinds, extents, and hard-link
  groups against the physical tree, and rejects unknown kinds, portability or
  custody disagreement, external symlinks, and bounded-resource excess. A
  successful hermetic build receives the explicit empty-tree commitment;
  unsponsored caller-owned build roots are not represented as package output.
  The package observation commitment binds the tree digest, entry count, and
  topology-independent unique-content byte count. Compiler review now retains
  the complete canonical tree content behind a non-constructible carrier, so
  exact ordinary/executable file bytes, empty directories, and relative
  symlink spellings survive disposal of the physical session. That carrier can
  materialize into an existing empty concrete directory and independently
  re-inspects every resulting path, kind, mode, target, and byte before
  returning the original commitment; invalid shape, nonempty or symlink
  destinations, write failure, extra/missing state, and drift reject. Hard-link
  topology is neither retained nor leaked through the content count. This tree
  custody alone is not a receipt; the v24/v6 grammar above is the first narrow
  case that joins it to complete operation replay and generated-output handoff.
  Broader operation/output coverage and complete remaining preparation-failure
  evidence remain. Same-user host racing is not solved by this custody rung.
  Raw byte-valued inputs reject above a
  compiler-owned 16 MiB ceiling before the provider clone/allocation. Raw
  transfer counts pass one checked
  conversion shared by both providers and reject negative, wrapped, or
  above-ceiling requests before allocation. This is an evaluator sponsor limit,
  not a language limit. Canonicalize enforces its declared
  1024-byte `PATH_MAX` carrier before provider or grant access.
  Scoped hard links now require write authority on both names; a read-only
  source object cannot be aliased into writable staging and then mutated through
  the shared inode. Namespace mutations authorize the canonical parent plus the
  actual leaf without following an existing leaf symlink; remove, directory
  create/remove, rename, hard-link, symlink, and `unlink_at` therefore cannot
  borrow write authority from a target inside staging while mutating a name
  outside it. Package review now owns one session-wide staging sponsor
  shared by every package compilation in the reviewed closure. Package build
  roots consume that account; namespace entries count by name, regular-file
  extents count once per unique object, symlink payload spellings count as
  bytes, and open-but-unlinked objects remain charged until their final
  descriptor closes. Hard links may only name objects already owned by the
  account. Mutations reserve the resulting account state before touching the
  OS and commit it only after the host operation succeeds; ceiling refusal is
  a distinct resource-exhaustion halt. Compiler policy currently permits 4,096
  entries, 256 MiB total logical bytes, and a separately enforced 256 MiB
  maximum object extent across the complete session. Process memory and CPU
  quotas remain.

- **PROOF-AND-BOUNDARY-ADMISSION.** Fail closed on false or incomplete evidence.

  Acceptance: open/deferred proofs reject, checked proofs are kernel-rechecked,
  accepted axioms/opaque claims remain trust-bearing, exact package-qualified
  boundary identities are enforced, underdeclared reach rejects, and dangerous
  overdeclared slack is reported.

  Progress 2026-08-23: concrete proof, contract, bounds, and termination
  obligations normally reject before checked trees are constructed; accepted
  axioms and admitted boundary qualifications remain identifiable. There is no
  implemented open/deferred-proof status yet. Contract entailment deliberately
  stands down for some out-of-engine-language claims. Package-aware checked
  compilation now audits the pristine typed graph, including generic
  templates, and retains exact machine/contract/fact coordinates plus a closed
  reason for every checked-implementation stand-down. The review projection
  rejects any such row; accepted/opaque supply remains in the trust lane rather
  than being mislabeled as an unresolved proof. This is fail-closed review
  behavior, not sealed evidence: kernel recheck receipts and a possible exact
  later-discharge ledger remain. Terminal propagation is required only for a
  row making a final-realization claim. Ordinary successful compilation is not
  itself a complete proof verdict. Dangerous overdeclaration is now exact for
  checked bodies: retained source-body presence selects inferred transitive
  reach, and v41 emits separate audit-recommended slack rows without treating a
  bodyless declaration as a failed realization. The standalone
  `psi-proof` boundary obligation ledger is not wired into production and must
  not be cited as enforcement.

- **COMPILER-REVIEW-HANDOFF.** Replace public construction/parsing of
  `PackageCapabilityManifest` as an admission input.

  Acceptance: production review orchestration accepts only review rows
  regenerated by the selected local compiler and bound to exact source and
  evidence-schema subjects. This is a custody boundary against package-authored
  review claims, not package acceptance or compiler certification. A standalone
  JSON file cannot impersonate local compiler output, and even authentic
  compiler rows cannot bypass `RECHECKABLE-PACKAGE-EVIDENCE`.

  Progress 2026-08-23: legacy manifest, lock, whole-section receipt, install,
  update, and graph-audit modules were first quarantined from the release
  `omega-packages` API and were deleted on 2026-08-25 after corrected production
  consumers no longer referenced them. The arbitrary public
  `PackageInstance` plus caller-derived toolchain/evidence fingerprint tuple was
  removed rather than adapted. Source diagnostics were split onto a retained
  production surface. Production package orchestration can now obtain
  non-caller-constructible review rows only by compiling every package from a
  resolver-owned source closure; callers cannot submit standalone manifest or
  review bytes to this path. The row retains the exact package key, selected
  immutable resolution, compiler projection, canonical comparison bytes, and
  compiler-issued source-consumption commitment. That commitment canonically
  binds the reconciled compiler graph and exact loaded source bytes without
  absolute cache locations. Package orchestration also re-hashes every
  transitive snapshot and rechecks the compiler-retained bytes after
  compilation. The orchestration additionally derives a compiler-owned digest
  of the current producer executable file before and after reviewing the whole
  closure, rejects drift, and retains the verified digest on every row without
  adding it to canonical capability/API bytes. It remains explicitly
  review-only: the digest is not compiler certification, complete source or
  toolchain provenance, a reproducible-build receipt, or proof of the already
  loaded process image; a hostile same-user process can still race filesystem
  observations. This task ends at review custody; only independent
  source-and-artifact checking may issue persistable package evidence.

- **FINAL-REALIZATION-EVIDENCE.** Keep Terminal evidence distinct from ordinary
  package admission.

  Acceptance: checked claims about Omega-emitted, native, or externally
  supplied executable code, lowering- or ABI-dependent guarantees, fixed native
  resource claims, and hardened profiles that request final-code replay require
  exact Terminal evidence. Opaque executable supply may remain an explicit
  trust/TCB row making no Terminal claim. Ordinary checked reach,
  authority-flow, provider, proof-status, and build-contract rows do not require
  blanket Terminal coverage. Every row carries an exact evidence class; absent
  Terminal evidence grants no Terminal claim, and no generic completeness bit
  weakens this rule.

- **EXTERNAL-EXECUTABLE-SUPPLY-REVIEW.** Project bodyless external realization
  as its own blocking trust/TCB row.

  Acceptance: every package-owned external realization, including a private
  implementation leaf, binds the exact package-qualified callable and tagged
  requirement application—trait conformance or operator overload coordinate—to
  one closed compiler-owned mechanism identity: import library and symbol,
  syscall number, compiler intrinsic, vtable slot, vtable field, or table-
  function field. The
  projector cross-checks the machine supply mode, satisfies binding, and
  external-binding table and requires exactly one satisfies application;
  missing, duplicate, mismatched, or unsupported state
  rejects rather than producing a partial row. This row remains distinct from
  callable API, declared/effective reach, boundary representation, accepted
  claims, and Terminal evidence. It records opaque executable supply and makes
  no claim that the supplied code was audited or that its realization was
  verified.

  Ratified design decision 2026-08-26: derive each component from the earliest
  coherent compiler-owned representation in which that component is
  semantically settled. Structural binding identity may come from private
  pre-Terminal state and join the checked callable/requirement association only
  after successful compilation. The projector may move with compiler internals;
  only its versioned canonical row crosses into package orchestration. Psi may
  repeat the consistency check as a backstop, but the package checker does not
  reconstruct an already-settled binding from reduced Terminal Psi. Do not add
  nominal Chi for this seam. Add a stage only if implementation discovers a
  reusable semantic boundary with independent consumers, transformations, or
  invariants; reuse an existing coherent representation such as Exact when it
  makes the implementation smaller without losing meaning.

  Milestone 2026-08-26: package review v70/canonical row v28 projects every
  package-owned external leaf as one `ExternalExecutableSupply` row with
  `OpaqueBlocking` risk. The callable/conformance application is the stable
  row key; the exact structural import, syscall, intrinsic, vtable, or table-
  function binding is row value, so a mechanism-only update changes this trust
  row without contaminating callable API identity. Private leaves do not
  become public callable rows. Projection validates exact-one conformance,
  bodylessness, supply/conformance/table agreement, mechanism consistency,
  payload bounds, attached table ownership, and canonical source accounting.
  Canonical recovery and conflict rendering retain the new row kind; a changed
  binding produces exactly one opaque-blocking supply conflict. Six-mechanism,
  private-leaf, stable-key, recovery, malformed-state, and conflict canaries
  cover the lane. No Terminal or audit claim is emitted.

  Milestone 2026-08-26: package review v72/canonical row v30 generalizes the
  external-supply key to one tagged exact requirement. The existing trait case
  is unchanged in meaning; the operator case admits a bodyless external leaf
  for a public, named, nongeneric boundary-operator slot. A public leaf also
  retains the operator coordinate in its callable row, while a private leaf
  still receives only the opaque supply and provider rows. Selected external
  operator plans must rejoin the exact operator, realization symbol, package,
  normalized machine identity, and structural binding; an unselected row never
  implies selection. The executable first lane is compiler-known intrinsics.
  Ordinary or private operators, aliases, generic/lifetime applications, and
  fixed-token boundary operators remain fail closed. Positive public/private,
  selected-provider, opaque-risk, recovery, and unsupported-neighbor canaries
  cover the new association.

## P4 — Lock and baseline

- **ORDINARY-PACKAGE-ARTIFACT-SUBJECT.** Finish and name the canonical semantic
  subject for ordinary package claims. It is the total versioned
  package-admission row set under one exact package key, target, dependency
  closure, and obligation-semantics schema. The compiler-issued review may
  carry candidate bytes in this same vocabulary, but review provenance and a
  matching hash confer no authority. A consumer accepts the artifact only by
  independently reconstructing the complete row set from the exact source
  subject and requiring byte-for-byte equality.

  Acceptance: the subject is source-handle-free, compiler-IR-free, proof-route-
  free, and complete for ordinary capability/API/build-contract obligations;
  unknown or omitted rows reject. Source bytes, compiler/process observations,
  certificates, decisions, and explanatory coordinates remain separately bound
  subjects or provenance. Native code and Terminal evidence are additional
  final-realization subjects rather than the ordinary package artifact. Do not
  create a placeholder `PackageInstance` or bless the current incomplete review
  bytes merely because the future artifact reuses their canonical vocabulary.

- **RECHECKABLE-PACKAGE-EVIDENCE.** Add the authority-bearing path that is
  deliberately absent from compiler-issued review. For every closure subject,
  retain the exact requested source, exact produced artifact, obligation-
  semantics/schema identity, canonical reconstructed obligation set, exact
  certificate bundle, derivation provenance, discharge result, and open
  obligations. A consumer must be able to re-derive the result without trusting
  a compiler- or verifier-authored verdict.

  Dependency results and open obligations compose transitively. Producer-side
  admission decisions do not compose; each consumer records its own decisions.
  Define checked schema-delta rows for unchanged, added, strengthened,
  reinterpreted, retired, and encoding-only classes. Unknown or meaning-changing
  deltas force re-derivation, and newly exposed gaps remain open until explicitly
  admitted. Canonical semantic identity excludes certificate route and producer
  pedigree while retaining both as replay provenance. Acceptance: tampered,
  missing, stale-schema, dependency-hidden, or admission-laundered evidence
  rejects under local replay.

  Ratified design decision 2026-08-25: local reconstruction may consume the
  earliest coherent compiler-owned IR in which each obligation is semantically
  complete, including private pre-Psi or pre-Terminal state. The checker moves
  with compiler internals and need not reconstruct reduced meaning from public
  Psi merely for layer purity. Persist only the versioned canonical obligation
  ledger, exact subjects, certificates, results, and open obligations; never
  persist private handles or make that IR a package format. Do not introduce a
  nominal Chi stage solely to stabilize this internal checker seam. Add a stage
  only if implementation discovers a genuine reusable semantic boundary, and
  collapse work into an existing coherent stage such as Exact when that makes
  the reconstruction smaller without losing meaning.

  Milestone 2026-08-25: Terminal verification now exposes one complete ordered
  `ReconstructedTerminalObligationSet` for operation, call-requirement,
  nominal-cleanup, and contract-`ensures` rows. Every row retains its exact
  semantic owner, proposition, obligation class, assumptions, reconstructed
  axiom order, and canonical-certificate disposition. `verify_module` consumes
  and retains that same set instead of privately rebuilding contract guarantees.
  A bounded canonical ledger codec binds the set to exact Terminal-Psi identity
  and the current verifier trust-graph identity while excluding proof route;
  decode is non-authoritative until local reconstruction matches it exactly.
  The trust graph now hashes a deterministic closure of every Rust verifier
  source file, closing the prior deep-module omission. Tampered questions and
  changed trust/program subjects reject; different valid proof routes retain
  one semantic ledger. Terminal artifact-manifest v2 retains that ledger
  fingerprint as a section identity distinct from semantic and proof identity.
  The source-independent lowering consumer can now decode persisted semantic,
  obligation, and proof sections, require exact local obligation reconstruction,
  then verify and lower; a canonical but substituted ledger rejects before
  proof checking. This is a real Terminal replay component, not package sealing.
  Whole-package source/artifact obligation reconstruction, ordinary
  capability/API evidence, transitive open obligations, schema deltas, and
  `PackageInstance` remain deliberately absent.

  Milestone 2026-08-26: resolved source closure now retains the exact validated
  root request separately from normalized lineage and immutable resolution and
  exposes one zero-copy request-set view joining every root/dependency request
  occurrence to the exact selected package key and resolution. Dependency
  selectors remain owned once by requester custody and are joined by authored
  ordinal, so distinct requests converging on one package remain distinct
  without copying hostile strings or choosing a primary request. Repository-
  root Git closure resolution now uses this path and keeps requested locator,
  normalized requested revision (including default `HEAD`), and transport
  profile separate from commit/tree/content identity. Mismatched root request/
  custody rejects. This is bounded
  resolver custody only: no selector enters the obligation ledger, no compiler
  review gains authority, and no lock, certificate, discharge, open-obligation,
  admission, or `PackageInstance` API exists. Canonical accepted-lock encoding
  and source-subject reconstruction remain open; Q2 separately governs multi-
  package Git selection.

  Milestone 2026-08-26: `CanonicalSourceClosureSubject` now projects that
  resolver custody into one bounded, source-handle-free canonical source-
  selection question. It retains the exact root request, every requester-owned
  dependency request by authored ordinal, every resolved alias, and the exact
  selected `PackageKey` plus immutable resolution/content for each occurrence.
  Distinct diamond requests remain distinct. Projection independently rejoins
  root/dependency request kinds to selected lineage, aliases to authored or
  package-derived names, contiguous ordinals to requester edges, and the full
  package table to one closed reachable acyclic graph. Strict recovery rejects
  unknown versions/cases, malformed or noncanonical ordering/framing,
  mismatched requests, aliases, targets, lineage, or resolution, open or cyclic
  graph state, trailing bytes, and resource-ceiling violations. A domain-
  separated fingerprint identifies only this exact question; a consumer must
  independently resolve and snapshot the closure, reconstruct the complete
  subject, and require exact equality. Snapshot/cache paths, resolver limits,
  source bytes, transport execution observations, compiler source-consumption
  and build observations, artifacts, review rows, certificates, decisions, and
  open obligations remain separate. This creates no accepted lock,
  `PackageInstance`, discharge result, admission, or promotion path. Q2 may add
  a future multi-package Git request case without blocking the current
  versioned sum.

  Milestone 2026-08-26: the current ordinary package-review vocabulary now has
  a source-handle-free `OrdinaryPackageObligationLedger`. It contains the exact
  package, target, compiler-consumed dependency closure, strictly ordered
  canonical semantic rows, risk, keys, and complete row bytes while excluding
  copied package display-name strings, source roots, resolutions, source bytes,
  and explanatory source coordinates. Each opaque package identity still binds
  its declared name and source lineage. The closure is projected only from
  validated compiler inputs and binds every reachable package identity and
  requester-local alias edge.
  Recovered row envelopes establish canonical framing only and must be joined
  to the separately reconstructed closure. The selected local compiler
  reconstructs the complete ledger from the earliest semantically complete
  compiler-owned representations after successful checking and requires exact
  equality; missing, reordered, stale, mixed-package, mixed-target, renamed-
  alias, and changed-closure subjects reject. Relocating the same graph does not
  change this coordinate. Fresh closure-review publication passes through this
  same second reconstruction gate before exposing compiler-issued rows.
  Reconstruction uses the existing typed/checked carriers and package projector;
  it introduces no nominal IR stage.

  This closes a replay mechanism for the current review vocabulary, not this
  task. The vocabulary is still incomplete for accepted package evidence, and
  the ledger deliberately has no lock-promotion path. Exact produced-artifact
  subjects, certificate replay and results, transitive open obligations,
  certificate/evidence-schema identity and checked deltas, transitive
  dependency-evidence composition, and root admission decisions remain
  required before `PackageInstance` exists.

  Milestone 2026-08-26: the ordinary ledger now names an explicit obligation-
  semantics schema independently from its outer codec and the review-row
  vocabulary. A bounded canonical whole-ledger encoding carries that schema,
  exact package, target, complete path-free package/alias closure, and every
  canonical row. Decode rejects unknown schema or row vocabulary, malformed or
  noncanonical framing, duplicate/open/unreachable/cyclic closure state,
  reordered rows, mixed package/target rows, trailing bytes, and every resource
  ceiling violation. Kind-specific row payloads deliberately remain opaque at
  this framing boundary; local reconstruction is what rejects a semantically
  malformed or incomplete question. A domain-separated ledger fingerprint
  identifies the complete framed replay question. Decoding and hashing confer
  no authority: local reconstruction and exact equality remain mandatory.
  Compiler-issued closure review retains the validated ledger rather than
  discarding it, under one overflow-safe 64 MiB aggregate retained-ledger
  ceiling for the whole review session. Neither the accepted lock,
  `PackageInstance`, certificates, results, open obligations, nor admission
  exists through this API.

  Milestone 2026-08-27: `CanonicalPackageReconstructionQuestion` now binds one
  complete `CanonicalSourceClosureSubject` to exactly one complete encoded
  `OrdinaryPackageObligationLedger` for every package in that source closure.
  Ledger frames pair positionally with the source subject's strict full-
  `PackageKey` order; distinct full keys colliding on compiler package identity,
  missing/foreign/swapped ledgers, mixed targets, wrong ledger roots, and
  source/ledger package or alias-closure disagreement reject. Fresh construction
  independently derives each package's transitive compiler closure through
  `package_compilation_inputs_for`, and fresh matching reconstructs the whole
  aggregate from current resolver custody plus a newly compiler-issued review
  set. Strict bounded recovery retains the complete nested source and ledger
  bytes and rejects stale versions, malformed/noncanonical framing, trailing
  bytes, and component or aggregate ceiling violations. Its domain-separated
  fingerprint identifies this replay question only. Compiler identity, build
  observations, source coordinates, artifacts, certificates, results, open
  obligations, admissions, accepted lock state, and `PackageInstance` remain
  absent; there is no promotion API.

- **ACCEPTED-LOCK-SCHEMA.** Replace name-keyed/fingerprint-only lock entries.

  Acceptance: `omega.lock` records `PackageKey`, `PackageInstance`, source
  request and immutable resolution, complete closure, per-subject obligation-
  semantics/schema identity, exact certificate provenance, re-derived results,
  transitive open obligations, normalized accepted capability/API baseline,
  build observations, separately labeled producer metadata, local admission
  decisions, and exact conflict-resolution references.

- **LOCK-BASELINE-RECOVERY.** Define missing-state behavior.

  Acceptance: committed lock alone is sufficient for capability comparison;
  unavailable old source triggers standalone source audit; absent accepted lock
  triggers fresh admission of the complete graph; missing normalized evidence
  behind a fingerprint is treated as missing admission evidence.

  Progress 2026-08-24: a bounded binary `ReviewOnlyBaselineCapsule` now
  checkpoints the exact `PackageKey` source graph, immutable resolutions,
  target and observed compiler commitment, source/build-observation and whole-
  review commitments, every complete canonical comparison row with its
  retained explanatory source sidecar, and the opaque canonical record for a
  successfully verified bounded source-read replay when present. The compiler
  strictly recovers row
  framing, risk, identity, target, and canonical source-coordinate shape;
  package code never decodes row semantics. Recovered rows have a distinct
  review-only type and cannot masquerade as newly compiler-issued evidence.
  Decode revalidates graph closure, graph/review bijection,
  singleton rows, ordering, checksums, canonical re-encoding, and independent
  resource ceilings. Replay-record recovery additionally requires exact
  semantic schema and operation-specific lane consistency; a domain-separated
  association binds it to the parent build-observation commitment, and capture
  accounts all replay bytes against one aggregate capsule budget. Its checksum
  and association detect corruption or inconsistent custody, not authenticity
  or serious review; project authority can replace them. A recovered capsule
  produces the same conflicts, triage,
  and source-review packets as live baseline state, including when all old
  source is unavailable. Capsule v2 now also has a capability-rooted file-
  custody layer. Trusted command orchestration supplies an already-open
  project-owned directory and one bounded lowercase portable direct-child
  filename. Recovery opens without following symlinks, reads under the capsule
  ceiling, performs the strict canonical decode, then rereads the retained file
  and rechecks its live pathname identity. New records use a synchronized
  private same-directory stage and atomic no-overwrite publication; Unix mode is
  `0600`. Nested names, existing destinations, symlinks, directories,
  corruption, and over-limit records reject. This closes the review-restart
  filesystem mechanism, not accepted lock persistence: the capsule has no
  `PackageInstance`, resolution, project-
  mutation, or lock-promotion path. Promotion remains blocked on
  `RECHECKABLE-PACKAGE-EVIDENCE`, transitive admission closure, and the accepted
  lock schema; producer provenance cannot promote it.

- **LOCK-CLOSURE-VALIDATION.** Port useful closure/reachability validation to
  `PackageKey` and instance identities.

  Acceptance: duplicate keys, conflicting instances, open edges, unreachable
  rows, stale evidence, and toolchain/source mismatches reject before use or
  persistence.

  Progress 2026-08-24: the review-only candidate join now has one shared
  validator used by capability comparison and source-review assembly. It
  rejects duplicate compiler rows, missing or unexpected custody, immutable-
  resolution disagreement, package/projection identity disagreement, mixed
  deployment targets, and mixed compiler-executable commitments before rows
  are compared or source is rendered. Baseline review sets receive the same
  identity, target, compiler, and duplicate checks; recovered baseline source
  remains intentionally partial and is checked separately. This is not an
  accepted-lock validator: conflicting package instances, stale or uncheckable
  certificates, mixed obligation-semantics identities, undisclosed transitive
  open obligations, and open/unreachable lock rows remain.

  Review-only baseline decode additionally reconstructs the complete typed
  source graph through `ResolvedPackageClosure`, rejecting duplicate,
  conflicting, open, unreachable, cyclic, over-depth, and resolution/lineage-
  mismatched rows before comparison. Accepted-instance, certificate replay,
  schema migration, and stale sealed-evidence validation remain part of the
  blocked accepted-lock schema.

## P5 — Admission, audit, and review

- **CAPABILITY-CONFLICT-MODEL.** Replace whole-section receipt approval with
  compact row-specific conflicts and exact resolution artifacts.

  Acceptance: conflicts name package/source identity, dependency path, old/new
  checked rows, risk, provenance, and source locations. Every blocking row must
  be resolved; artifacts bind the exact candidate, toolchain, evidence, and
  conflict fingerprint and are accepted only through root-project policy.
  Optional reviewer/signature/reason fields are governance metadata and never
  proof that an audit occurred.

  Ratified design decision 2026-08-24: conflict rows are projected by the
  compiler from the earliest compiler-owned representation in which each exact
  fact is semantically settled. This may include private pre-Psi structure;
  checked acceptance, effects, proofs, and realization are joined from the
  stage that establishes them, and no row is issued before successful checking.
  Rows need not share one source stage. The compiler may depend on those private
  representations because the projection moves with the compiler; only the
  versioned, source-handle-free row encoding crosses into package
  orchestration. Do not create nominal Chi solely to make this internal join
  look stable. Add a named stage only if implementation discovers a genuine
  reusable semantic invariant boundary. Additional consumers or transformations
  may reveal such a boundary; stability, layer purity, or local simplification
  alone do not. Freely collapse rows into an existing coherent representation
  (for example `Exact`) when that removes machinery without losing meaning.

  Progress 2026-08-24: the review projection exposes independently framed,
  compiler-owned rows for the projection header, public traits, domains, data,
  representation TCB, callables, accepted claims, dangerous authority, and the
  selected-provider set. Package orchestration must compare the complete row
  bytes and must not parse or reconstruct their semantics. Callable details
  are initially one exact envelope; selected providers deliberately remain one
  opaque blocking set even though the compiler retains their sealed
  identities. The package layer now stores those
  bounded rows when compiler review is issued, linearly compares exact
  `(kind, key)` coordinates, and emits added/removed/changed review-only
  conflicts without decoding compiler payloads. Each conflict retains complete
  old/new bytes and binds both immutable resolutions, both compiler and source-
  consumption commitments, whole-review evidence, one bounded explanatory
  dependency path, and a canonical commitment to the complete candidate
  closure. The normal renderer exposes fixed vocabulary, lengths, and row
  commitments rather than hex-dumping payloads; source patches remain the
  readable audit lane. Input rows, owned changed bytes, paths, conflict counts,
  and output are separately bounded and reject rather than truncate.
  Representation-TCB-only changes now recommend audit without becoming a
  blanket capability block; blocking and opaque-blocking row changes still
  block. Compiler rows now also carry separately bounded, canonical package-
  relative UTF-8 paths and exact byte spans for declaration anchors. Dangerous-
  authority rows retain both the canonical toolchain authority declaration and
  every reviewed package callable exposing it. These explanatory coordinates
  do not enter semantic row bytes, so moving a declaration does not manufacture
  a capability change; old/new coordinates do enter each changed-row conflict
  fingerprint and fixed-vocabulary renderer, binding a resolution to what was
  shown. Generated symbols follow their mandatory authored derivation origin,
  toolchain owners reuse the compiler's canonical source-custody framing, and
  absent authored provenance has a closed compiler-derived reason rather than
  an empty placeholder. Public trait, domain, data, representation-TCB, and
  callable projection now carries each exact declaration symbol beside its
  semantic row, and canonical sorting moves the pair together. Dangerous-
  authority projection likewise retains the exact service declaration and
  every exact exposing callable as it derives the row. None of these families
  rescans typed trees by reduced nominal identity after projection. Provider
  candidate derivation now captures an internal
  sidecar beside each semantic plan: its exact boundary schema symbol, optional
  nominal provider symbol, and the exact requirement plus realizing machine for
  every row, including external leaves and checked adapters. Selected-provider
  explanatory custody now emits a distinct canonical source role for every
  retained requirement declaration as well as every realization, so review
  cannot show only the implementation half of a provider row. Selection keeps
  that pair intact and adds the exact build-override or target-default call sites, or the
  closed `UniqueCoveringProviderSelection` reason for an implicit choice;
  semantic sorting moves plans and provenance together. The single selected-
  provider review row can therefore combine authored coordinates with compiler-
  derived reasons without reconstructing either from names, schemas, or
  fingerprints. Free external providers and an empty selected set also carry
  closed reasons.
  Milestone 2026-08-26: public-trait rows now retain every exact authored
  parent-identifier span beside the trait declaration under the closed
  `trait_parent` role. The typed trait edge already owned this span; projection
  carries it through canonical row sorting rather than reconstructing it from a
  parent name. Coordinates remain explanatory and outside semantic row bytes.
  Milestone 2026-08-26: syntax, resolved, and typed contract rows now retain the
  exact authored `requires`, `ensures`, or `crashes` keyword span independently
  from semantic facts. Direct machine, public-trait requirement, and public-
  operator contracts carry that anchor under `contract_clause`; every projected
  declaration family also walks structural static-machine parameter contracts
  recursively. This uniformly covers expressions, memberships, proposition
  applications, named evidence, and outcome groups without pretending an
  expression-node token is a complete fact span. Accepted-claim rows reuse the callable sidecar, so a bodyless
  trusted guarantee points at its `ensures` clause rather than only its
  declaration. Checked body calls now join each checked-flow call coordinate to
  the exact typed statement, expression, or named-transition call site while
  that join is still stable. Source statement and transition calls carry an
  explicit authored call-selection occurrence through resolution and typed
  lowering; expression calls reuse their existing attached occurrence. Checked
  lowering verifies target, receiver, receiver shape, and operational
  acknowledgement, then retains the exact authored span on the checked call.
  Package projection consumes that retained custody rather than rejoining
  transformed typed calls after provider settlement. A legitimately late-bound
  source target may be unsettled at capture time; the location does not pretend
  to prove target finalization. Missing, duplicate, unknown, or contradictory
  source custody rejects; compiler-synthesized calls emit no invented location.
  Authored `invokes` targets now lower to one typed occurrence that binds the
  diagnostic name, exact parameter-symbol/ordinal or boundary-trait symbol,
  and exact target-name span. Effect inference consumes that retained target
  rather than reselecting a same-spelled trait. Callable, public-trait
  requirement, and recursively structural machine-parameter rows carry the
  span under `synchronous_invocation`; top-level projection requires exact
  equality with the checked invocation plan. Exact symbolic published and
  inferred targets are retained during checked lowering because provider
  settlement may later rewrite typed call structure; package projection never
  re-infers effects from that transformed tree. Missing, malformed,
  duplicated, aliased, stale-target, or source-less custody rejects. Authored
  `reaches` clauses now retain every keyword occurrence and every member target
  occurrence through syntax, resolution, typed lowering, copying, and generic
  specialization. Resolution binds each member once to its exact boundary-trait
  symbol; duplicate authored members remain distinct explanatory occurrences
  while the semantic row remains idempotent. The normalized row is rederived as
  the parent closure of authored targets plus invocation-contributed services
  and joined exactly to typed and checked facts. A private memberless authored
  clause is therefore a published empty ceiling, distinct from omitted private
  reach inference. Callable, public-trait requirement, and recursively
  structural machine-parameter rows carry target spans—or the keyword span for
  an authored empty row—under `service_reach`. Parent closure, inference, and
  invocation-only reach receive no invented source location. Missing,
  duplicated carrier rows, stale targets, malformed spans, installation-bound
  disagreement, or semantic disagreement reject. Authored `suspends` and
  `blocks` clauses now likewise retain each exact keyword occurrence through
  syntax, resolution, typed lowering, trait-default synthesis, copying, and
  generic specialization. Callable, public-trait requirement, and recursively
  structural machine-parameter rows carry them under distinct `suspension` and
  `blocking` source roles. Projection requires authored booleans, keyword
  custody, and checked published/internal interfaces to agree exactly; omitted
  clauses and compiler inference receive no invented location. For public and
  otherwise contract-supplied machines, the checked operational summary remains
  the published may-ceiling by language design—it is not presented as a second
  observation that the retained body happened to be quiet. Current package
  review v75/canonical row v33, conflict fingerprint v16, renderer V15, and
  canonical-row recovery envelope v13 bind the appended roles; stale envelopes
  reject rather than being reinterpreted. Any later nested source carriers
  remain incremental engineering work and require deliberate retention before
  their owning frontend stage erases them, not later source-text reconstruction.
  Milestone 2026-08-27: every authored external executable leaf now retains
  the exact `via` keyword occurrence on the same conformance that owns its
  normalized binding identity. Syntax copying, resolution, and typed lowering
  preserve that custody. Projection requires exact binding/span parity and
  attaches one `external_binding` location to each public or private trait- or
  operator-based external-supply row; missing, source-free, or contradictory
  custody rejects, and no later lookup by spelling is permitted. Coordinates
  remain outside semantic row bytes, so review v74/canonical row v32 remain
  unchanged. Canonical-row recovery envelope v8, conflict fingerprint v11,
  and renderer V10 bind the appended explanatory role; stale recovery records
  reject.
  Milestone 2026-08-27: public const declarations now retain the exact parsed
  initializer-expression span before const substitution erases the value tree.
  Symbol-resolved and typed const declarations carry that occurrence, and the
  `PublicConst` row emits it under the closed `const_initializer` role beside
  the independent declaration-name anchor. Recovery envelope v9, conflict
  fingerprint v12, and renderer V11 bind the new role and exact coordinates;
  semantic package-review bytes remain v75/canonical row v33 because source
  locations are explanatory sidecars. Canaries require the two source slices
  to be exactly `LIMIT` and `4`, preserve the role through recovery and conflict
  rendering, and prove that relocating identical semantics changes coordinates
  without changing canonical review identity.
  Milestone 2026-08-27: transparent public propositions now retain the exact
  semantic-token extent of their authored formula at the proposition parser
  boundary. This occurs before typed application lowering can erase the
  enclosing expression handle and before operator-root spans can narrow the
  formula to one token. Syntax, symbol-resolved, and typed proposition
  declarations carry that custody directly; a `PublicProposition` row emits it
  under the closed `proposition_formula` role. Projection requires exactly one
  formula location for a transparent proposition and none for primitive or
  witness propositions. Recovery envelope v10, conflict fingerprint v13, and
  renderer V12 bind the role and coordinates. Semantic package-review bytes
  remain v75/canonical row v33: the normalized proposition body remains the
  compatibility identity, while the formula location is explanatory custody.
  Boolean and application-form canaries require the exact source slices and
  reject missing or contradictory custody.
  Milestone 2026-08-27: every authored proof fact now retains its full
  semantic-token extent at the common fact parser boundary. A sparse sidecar
  binds that occurrence to the existing exact fact handle through syntax,
  resolution, typed lowering, generic-instance synthesis, and checked
  monomorphization; semantic fact variants and canonical identity remain
  unchanged. Public domain predicates and public data invariants require exact
  custody for every fact. Authored callable, trait-requirement, operator, and
  recursively structural machine-parameter contracts likewise require one
  fact location per retained fact, while source-free compiler-synthesized
  contracts receive no invented coordinates. The closed `proof_fact` role
  exposes those extents beside the independent clause keyword. Recovery
  envelope v11, conflict fingerprint v14, and renderer V13 bind the role and
  coordinates; semantic package-review bytes remain v75/canonical row v33.
  Vertical canaries cover expression and membership parsing, data/domain/trait
  projection, recovery, and changed-domain conflict rendering.
  Milestone 2026-08-27: public trait rows now retain each exact machine-
  requirement declaration under `trait_requirement`, and public data rows
  retain every exact field, sum case, and sum payload-field declaration under
  `data_member`. Both use the already-retained typed declaration symbol; a
  source-backed symbol must resolve to its direct authored span, while a
  compiler-derived declaration exposes only its real derivation origin rather
  than a fictional nested anchor. Recovery envelope v12, conflict fingerprint
  v15, and renderer V14 bind the roles and coordinates without changing review
  v75/canonical row v33. Real-source canaries require exact identifier slices
  and changed-row conflict rendering for both declaration families.
  Milestone 2026-08-27: every value parameter on a reviewed package callable,
  public operator, and public trait requirement now retains its exact typed
  declaration symbol under `callable_parameter`. The same walk covers value
  parameters nested in structural static-machine contracts. Direct declarations
  expose their authored identifier span; compiler-derived declarations expose
  only their real derivation origin. Recovery envelope v13, conflict fingerprint
  v16, and renderer V15 bind the role and coordinates without changing review
  v75/canonical row v33. Vertical canaries cover callable, operator, trait,
  recovery, and changed-callable conflict rendering.
  Canonical recovery and root-project file custody are recorded below; none of
  these concerns requires nominal Chi or a new owner decision.

  Milestone 2026-08-25: review-only root policy now records one closed
  accept-candidate-change or reject-candidate-change disposition for every exact
  blocking fingerprint. Decisions can be constructed only from a conflict in
  its owning package set. The complete canonical decision set is bound to the
  candidate-closure commitment and rejects empty, incomplete, duplicate,
  stale/foreign, wrong-candidate, and non-blocking inputs. Its sole aggregate
  outcome is whether root policy permits every blocking row; it cannot mint
  evidence, authorize the wider transaction, or claim that an audit occurred.
  At that milestone, durable encoding, governance metadata, root-policy
  custody, and install/update transaction revalidation remained separate work.

  Milestone 2026-08-25: the complete review-only resolution now has one
  bounded canonical fixed-vocabulary text record. It contains only the exact
  candidate-closure digest, sorted conflict fingerprints, closed accept/reject
  dispositions, and reconstructed resolution commitment; there are no
  package-controlled strings, reviewer claims, or governance prose. Recovery
  requires exact LF framing, lowercase hex, canonical decimal counts, byte and
  decision ceilings, and a byte-identical canonical re-encoding. Every parsed
  fingerprint must match a current compiler-derived conflict and is rebuilt
  through its owning package before the complete resolution validator reruns.
  Wrong-candidate, unknown/stale, incomplete, duplicate, non-blocking,
  reordered, malformed, commitment-divergent, or trailing input rejects.
  Candidate-closure commitment v2 now binds every candidate package's target,
  compiler executable, source consumption, optional build observation, and
  whole-review commitment in addition to exact source topology and resolution,
  including packages that produce no blocking conflict. A policy record cannot
  therefore omit unchanged or recommendation-only candidate evidence.
  At that milestone, root-project file custody, optional governance metadata,
  accepted-lock reference, and install/update transaction revalidation remained
  open.

  Milestone 2026-08-26: canonical resolution records now have a bounded
  policy-directory custody layer. Trusted command orchestration supplies an
  already-open, root-owned directory capability plus one lowercase portable
  canonical filename; nested paths, absolute/empty/dot forms, separators,
  controls, overlong names, case aliases, Windows device aliases, symlink
  leaves, and existing destinations reject. Every root-policy operation is a
  direct-child operation relative to that handle. Persistence writes and
  synchronizes a private same-directory stage, rereads it, then atomically
  publishes one no-overwrite hard link. Reads retain their open file, rerun the
  exact semantic recovery, reread the bytes, and recheck the live filename.
  Directory synchronization is required rather than silently skipped; a failure
  after atomic publication reports `published but unconfirmed`, since complete
  canonical bytes may remain recoverable. Command integration must supply the
  actual root-owned directory; this library does not discover policy under
  dependencies or prescribe a final UX filename or directory.
  This detects ordinary concurrent replacement and in-place change; it does not
  claim linearizability against a hostile process already holding the root
  author's filesystem credentials. Final install/update transaction locking and
  immediate policy revalidation own that boundary.
  Directory custody proves neither an audit nor admission and does not authorize
  mutation or replace optional governance metadata, accepted-lock reference,
  and final install/update transaction revalidation.

- **DANGEROUS-AUTHORITY-CLASSIFICATION.** Classify risk from compiler-owned
  nominal metadata.

  Acceptance: filesystem, network, process, dynamic loading, signing, secrets,
  executable installation, root memory, DMA/IOMMU, interrupts, and equivalent
  authority cannot be spoofed or hidden by package-controlled names.

  Progress 2026-08-24: the existing build-host staging gate no longer admits a
  package-authored `FilesystemHost` or `Console` lookalike by spelling. Allowed
  staging services must resolve to the exact toolchain source origin and
  canonical std module path; a fail canary pins same-name spoof rejection. The
  compiler review projection now emits the first intrinsic risk row for an
  exposed canonical toolchain `FilesystemHost`, selected by exact declaration
  and toolchain-source coordinates rather than package-controlled spelling.
  The same exact provenance join now classifies canonical `MachineControl`,
  `PortIo`, `InterruptMaskControl`, `InterruptEntry`, and
  `ExtentRootProvider` as machine-control, port-I/O, interrupt-control,
  interrupt-entry, and root-memory authority. Canonical and same-named
  package-owned tests pin both sides of each join. The exact toolchain-owned
  `Console` is additionally classified as process authority because reach is
  trait-granular and that canonical trait includes `exit_process`; a
  package-owned `Console` lookalike cannot mint the class. Comparison encoding
  v41 retains these rows. `ProgramStorageEntry` is not mislabeled as executable-
  installation authority merely because it receives already-installed roots;
  that class must come from exact installation evidence. Network, dynamic
  loading, signing, secrets, executable installation, DMA/IOMMU, and sealed
  package evidence remain; no canonical surface for those classes is currently
  present, so the compiler must not infer them from suggestive package names.

- **REPRESENTATION-TCB-REVIEW.** Retain claim-free opaque boundary data as a
  distinct compiler-owned review lane.

  Acceptance: every row binds exact package/declaration, target,
  representation/ABI, mechanism or explicit unbound status, source, toolchain,
  and compiler evidence; introduction or material change produces a strong
  code/ABI audit recommendation; unchanged rows remain visible without
  recurring blanket approval; exact dangerous mechanisms may be policy-blocked;
  accepted claims, authority establishment, executable supply, and API
  compatibility remain separate rows. Package-controlled names and absence of
  current `reaches` never classify or suppress evidence.

  Progress 2026-08-24: checked package review now emits a distinct
  package-qualified representation-TCB row for every root-package
  `boundary data`, including private declarations and declarations with no
  reach or claim. The row is target-scoped by the containing projection and
  explicitly records both ABI commitment and external mechanism as `Unbound`;
  it does not fabricate layout or realization. Comparison encoding v41 retains
  the lane. Exact mechanism/ABI selection, semantic-subject and certificate
  checking, and admission-policy outcomes remain.

- **SOURCE-AND-PROVENANCE-TRIAGE.** Run automated/LLM triage for every source
  update, independently of capability equality.

  Acceptance: retained dangerous authority recommends audit; unavailable old
  source escalates to standalone candidate audit; source-lineage/provenance
  changes block as replacement; triage input contains only bounded,
  Omega-rendered evidence and no package prose. LLM output remains advisory.

  Progress 2026-08-24: review-only orchestration now derives deterministic
  per-package triage from compiler-issued closure rows rather than legacy
  manifests or reviewer prose. Initial admission blocks an exact accepted-claim
  row for root-policy resolution and recommends audit for dangerous authority
  and introduced representation-TCB rows. Newly introduced transitive packages
  follow the same rule; an unchanged accepted claim does not demand recurring
  blanket approval. Updates block
  changed capability/API bytes and source-lineage replacement, retain a
  standalone-audit recommendation when old source is unavailable, and continue
  to recommend audit for unchanged dangerous authority. A fixed-vocabulary
  renderer exposes only canonical package-key commitments and closed reason/
  disposition tokens; it rejects above a caller ceiling instead of truncating.
  A separate source-patch renderer now compares only exact-key,
  resolver-custodied immutable snapshots. It binds full immutable resolutions,
  diffs raw path-ordered files with deterministic bounded work and context,
  retains executable, directory, symlink, line-ending, and entry-kind changes,
  and byte-escapes every attacker-controlled lane. File/entry, content,
  metadata, line, diff-work, trace-memory, and rendered-output ceilings reject
  rather than truncate. Binary or non-UTF-8 changes retain size and a
  domain-separated content commitment and deterministically require standalone
  audit. Missing baseline source uses the same renderer in complete-candidate
  mode; source-lineage replacement cannot enter ordinary diff mode. Joining
  is now implemented: candidate review rows must bijectively match the complete
  resolved closure by `PackageKey` and immutable resolution; every recovered
  baseline custody must match its compiler row; absent old custody is derived
  as `BaselineSourceUnavailable` without erasing compiler evidence. Initial
  source packets are emitted only for compiler-recommended audits; every
  changed or unavailable existing update source receives an exact diff or
  standalone candidate packet, and new packages follow initial-admission risk
  policy.
  One aggregate ceiling frames compiler-only triage separately from hostile
  source lanes.

  Progress 2026-08-26: the runner-neutral advisory boundary is implemented.
  It supplies fixed system instructions separately from the bounded rendered
  evidence and response schema; the package library selects no model and
  supplies no ambient network authority. The caller sets an output ceiling and
  the runner streams response bytes into an Omega-owned bounded sink. Only the
  exact canonical result envelope with one of two tokens—`recommend_audit`
  or `no_additional_audit`—is accepted.
  Advice is monotone: it may add an audit recommendation, but cannot suppress a
  compiler recommendation or alter a blocker. It emits no prose and has no
  authority to prove an audit, resolve a conflict, admit a package or evidence,
  set policy, or mutate project state. Provider/configuration and CLI wiring,
  row-specific capability conflicts, sealed accepted-baseline loading, and
  policy application of the input-bound advisory result remain.

- **AUDIT-RESULT-STATES.** Represent at least `admitted`,
  `admitted-with-audit-recommended`,
  `blocked-capability-change`, `blocked-missing-admission-baseline`, and
  `blocked-provenance-change`. Organization-specific review completion may be
  attached as policy metadata but is not a compiler-certified result state.

  Progress 2026-08-25: deterministic compiler-review triage represents all five
  states directly. An update without normalized accepted admission evidence is
  `blocked-missing-admission-baseline`; it is distinct from unavailable old
  source (accepted rows still compare and standalone source audit is advised)
  and from initial install (an explicit complete-graph fresh admission). Its
  bounded fixed-vocabulary renderer emits no package prose, and candidate risk
  reasons remain visible without weakening the blocker. Command orchestration
  must enter this state rather than treating a missing lock as an unchanged
  update; transition to fresh admission remains part of install/update wiring.

## P6 — Commands

  Cleanup 2026-08-27: the unfinished `omega install` and `omega update`
  implementations and their temporary rejecting command gates are absent from
  the production CLI. P0, recheckable evidence, and the accepted lock remain
  prerequisites for adding the commands described below. Exact source
  filenames such as `install.omg` and `update.omg` remain ordinary compiler
  inputs.

- **OMEGA-INSTALL.** Implement
  `omega install <source> [--rev <revision>] [--as <alias>]`.

  Acceptance: fetch, declaration extraction, closure resolution, compiler
  evidence, conflict handling, triage, and required root-policy decisions all
  complete before `build.omg` or `omega.lock` changes. A failed or unresolved
  install performs no mutation.

- **OMEGA-UPDATE.** Implement
  `omega update [package-or-alias...] [--to <revision>]`.

  Acceptance: builds from the accepted lock; candidate capability/API change
  blocks; unchanged evidence still receives provenance/source triage; retained
  dangerous authority recommends audit; accepted mutation is atomic.

- **OMEGA-AUDIT-PACKAGES.** Render the accepted graph and current source state.

  Acceptance: output includes source lineage and immutable pins, dependency
  paths, declared/realized reach, authority flow, providers/trust/proofs,
  dangerous slack, build observations, review status, and first failed
  provenance edge.

## P7 — Fixtures

- [x] **MIGRATE-PACKAGE-FIXTURES.** Add `PACKAGE` declarations and canonical build
  variable names to every fixture.

  Acceptance: fixture identity comes from source, not directory names or test
  constructors, and package-aware review compilation emits every currently
  representable expected evidence row. Sealed admission is tracked by the
  admission and lock tasks rather than this mechanical migration.

  Completed 2026-08-24: all eleven local package fixtures declare `PACKAGE` and
  use the coherent `builder` parameter name. Their exact private CathedralOS
  pins were refreshed from the same local source. The optional live-network
  test compares package declarations, source content, and canonical dependency
  projections rather than assuming every fixture is dependency-free; it is the
  continuing remote-parity check and is explicitly ignored when private SSH
  access is unavailable. The ordinary local suite proves the checked-in pin and
  fixture declarations, not current remote bytes.
  A local integration canary now resolves each real package closure through
  resolver-owned immutable custody, compiles it through the package-aware
  compiler path, and asserts canonical compiler-issued review evidence for the
  package identity, public surface, reach, invocation, accepted claim, and
  capability flow represented by the fixture. `provider-switchboard` now also
  selects a real ordinary provider type from its canonical build machine and
  asserts the compiler-issued selected-provider row and its exact selection,
  schema, provider-type, and realization coordinates. The integration now uses
  production review orchestration for every package in each resolved closure;
  a tampered read-only snapshot canary rejects before compiler consumption.
  Sealed admission evidence remains gated on the final admission pipeline.

- [x] **BUILD-ROOTED-PATH-SURFACE.** Expose the executor's existing immutable
  Source grant and writable Output grant through activation-scoped facets of
  the canonical one-parameter `Build` value. Add the smallest ordinary-library
  resolver and filesystem surface needed by `generated-table`; add no grammar,
  and place no capabilities in the durable build projection.

  Resolution must bind one exact root occurrence to canonical relative
  `&[u8] in Path` bytes and reject absolute input, traversal, ambiguous roots,
  and symlink escape before provider access. Authorized path-returning
  operations preserve the same root or reject. `read_link` returns inert
  payload and following it requires checked resolution. Stable `/source/...`
  and `/output/...` renderings belong only to evidence.

  Acceptance: `generated-table` reads `inputs/table.txt`, writes a generated
  Omega file only under its fresh staging root, and introduces it through an
  explicit successful handoff. A failed build publishes nothing. Canaries pin
  source mutation, output escape, working-directory dependence, host-absolute
  path leakage, unvalidated link traversal, and output-without-handoff
  rejection. Successful evidence retains exact Source/Output occurrence plus
  canonical relative bytes.

  Progress 2026-08-25: the canonical filesystem build prelude now exposes
  ordinary zero-field `BuildSource` and `BuildOutput` facets whose exact
  toolchain `resolve` machines produce interpreter-private rooted path values.
  Rooted mode rejects bare paths, package-authored resolver lookalikes,
  absolute/traversing/ambiguous/host-specific spellings, Source-root writes,
  and the host-absolute results of `canonicalize` and
  `final_path_name_by_handle`. Existing scoped-provider custody still resolves
  symlinks and emits exact Source/Output root identities plus canonical
  relative bytes. The checker now retains exact parameter-field receiver calls
  and statement-scoped local selections needed by package admission. Full
  checker/interpreter suites and the granted-build integration pass.

  Progress 2026-08-25: exact toolchain `BuildOutput::include_source` now records
  only interpreter-retained Output-rooted paths, rejects Source/unrooted,
  duplicate, over-count, and unscoped handoffs, and publishes its sidecar only
  after successful granted evaluation. Omega matches each handoff against its
  sponsored captured tree and retains only an ordinary, non-executable `.omg`
  file's exact bytes, relative path, and digest. Missing, wrong-kind,
  wrong-extension, and uncaptured handoffs reject.

  Progress 2026-08-25: package-aware checked compilation now freezes ordinary
  source discovery, executes the selected build prepass once, appends each
  explicitly handed-off retained UTF-8 source under a compiler-owned logical
  path, and reruns the ordinary frontend/checker without re-executing build or
  expanding dependency discovery. It exactly rebinds the authored build
  declaration across passes, commits generated bytes with package source
  consumption, and verifies them from retained staged-tree custody after the
  physical review directory is gone. Staged `.omg` output without handoff,
  reserved source-discovery filenames, and invalid generated syntax reject.
  Existing review-session canaries pin failed publication cleanup.

  Progress 2026-08-25: `generated-table` now uses the canonical free
  `machine build(builder: &mut Build)`, obtains its admitted filesystem service
  from that single Build activation, reads `inputs/table.txt`, writes only
  `table.generated.omg` under Output, explicitly hands it off, and exposes the
  generated checked `table_size` callable in compiler-issued review evidence.
  Its observations are correctly Volatile and retain the six rooted filesystem
  attempts plus the replayable staged tree.

  Milestone 2026-08-27: review orchestration now retains one opaque compiler-
  issued generated-source bundle from every dependency-first checked package
  and supplies the complete set, including explicit empty bundles, to each
  later consumer. A bundle binds the exact producer package, target, source-
  path-free dependency closure, source-consumption commitment, and only that
  producer's explicit retained handoffs. Initial frontend loading gives those
  bytes the producer's exact package identity and logical
  `.omega/generated/...` path, resolves imports from retained custody without
  physical output access, commits them in the consumer's source consumption,
  and never reruns the dependency build. Missing, duplicate, foreign, root-
  self, wrong-closure, wrong-target, and review/custody substitution reject.
  Isolated compiler tests exercise a real consumer import with no physical
  generated file plus those structural tamper cases.

  The filesystem-producing `generated-table -> generated-consumer` package-
  review canary is retained but ignored at the already-open OWNER Q7 seam:
  after std relocation, the physical bundled `FilesystemHost` source is no
  longer authenticated as ordinary-package staging authority. Do not restore
  it with a name or path exception. Ratifying Q7 and switching that service to
  exact graph-role authority should make the canary executable without any
  generated-source design change.

  Completed 2026-08-26: direct Source-read and Output-create canaries place an
  intermediate symlink inside the corresponding grant and its target outside
  every permitted root. Following either link rejects as
  `OutsideGrantedRoots` before the requested open/create operation; the Output
  canary additionally proves an absent leaf was not created through the
  escaping parent. The host-specific directory-symlink constructor keeps this
  follow check active on Unix and on Windows test hosts with symlink privilege.
  Together with the existing inert `read_link`, namespace-leaf, host-absolute-
  result, source-mutation, unrooted/noncanonical, handoff, and sponsored-
  publication canaries, this closes the rooted path surface itself.

- [ ] **BLOCKED — OWNER Q3/Q8: PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION.** Route native-image
  production through the same sponsored package transaction as package review,
  without rerunning `build.omg` or reopening source discovery.

  Acceptance: the transaction lowers the exact frozen final checked program
  produced after explicit generated-source handoff, retains the unpublished
  native artifact as an exact subject, reconstructs and compares source,
  obligations, build observations, generated bytes, and native output against
  accepted evidence, and publishes only after the complete comparison succeeds.
  Standalone native compilation keeps rejecting generated-source handoff and
  cannot mint package admission or filesystem authority. This task is
  engineering-sequenced after `RECHECKABLE-PACKAGE-EVIDENCE` and
  `ACCEPTED-LOCK-SCHEMA`; final transaction identity is additionally blocked on
  the open application-root identity and requested-versus-source-target policy
  decisions.

- **SECURITY-FIXTURE-MATRIX.** Add local and remote cases for pure code,
  generated files, filesystem, network overreach, retained filesystem+network
  authority, claim-free opaque representation, dangerous-mechanism escalation,
  accepted claims, provider changes, capability flow, missing old source,
  missing lock baseline, same-name/different-lineage spoofing, transport
  normalization, and dependency-version reconciliation conflict.

  Progress 2026-08-24: real compiler evidence now covers a pure public package,
  a bodyless accepted boundary claim with a distinct blocking row and
  initial-admission source packet, exact filesystem reach/invocation,
  retained network reach without a hidden invocation, exact clock-service
  reach/invocation, package-qualified capability acquisition and return flow,
  and a two-dependency source graph. `opaque-carrier` adds an exact
  package-qualified public-data row
  with boundary-opaque supply and no authored semantic claim; its byte-identical
  private CathedralOS mirror is pinned at an exact commit. It deliberately does
  not fabricate the still-unsealed mechanism/ABI evidence. The local
  `remote-journal` fixture now adds exact retained
  filesystem+network reach and invocation through resolver-owned custody and
  compiler review evidence; its private CathedralOS mirror is pinned at the
  byte-identical source commit. `file-journal` and `remote-journal` now use the
  real canonical filesystem boundary, while the latter's network boundary
  remains intentionally package-local until a canonical toolchain network
  surface exists. Compiler review classifies only the canonical filesystem
  authority. `process-exit` adds exact canonical `Console` reach/invocation,
  compiler-owned process-authority classification, initial-install and
  unchanged-update audit recommendations, and a byte-identical private
  CathedralOS mirror at an exact commit. A package-owned `Console` lookalike is
  separately pinned as non-authoritative. `provider-switchboard` now covers
  exact build-owned provider selection and its normalized compiler review row.
  `generated-table` now executes the canonical build machine's exact toolchain-
  owned filesystem reach/invocation from immutable source custody and a
  separate writable root inside a disposable review child session, then
  compiles its explicitly handed-off generated source through the frozen final
  pass. Fixture-
  backed same-name/different-lineage coverage now keeps two byte-identical
  `shared-provider` packages as distinct exact-key graph nodes and proves the
  selected-provider review row remains bound to only the explicitly imported
  lineage. This exposed and fixed local snapshot deduplication that previously
  collapsed distinct lineages onto one physical compiler root. Missing old
  source is covered both with live review state and a reopened review-only
  baseline; missing accepted-lock state remains blocked on
  `RECHECKABLE-PACKAGE-EVIDENCE` rather than being simulated with that non-
  admitting capsule. A fixture-derived
  `provider-switchboard` update now changes only the canonical build selection
  from `MonotonicClock` to `WallClock`; compiler-issued package-qualified
  projections prove both endpoints, and reconciliation emits one changed,
  opaque-blocking selected-provider-set conflict with baseline/candidate source
  locations before triage blocks the update. Real-custody reconciliation now
  resolves two actual commits of one local Git
  repository through the hardened Git snapshot path, binds both snapshots to
  one canonical network lineage and declared package key, and proves the
  closure rejects their distinct immutable resolutions while retaining both
  exact root request ordinals and aliases. The private remote canary remains
  explicitly ignored when CathedralOS SSH credentials are unavailable; it
  rejects rather than substituting ambient or fabricated transport evidence.
  A second two-commit Git canary now upgrades `process-exit` from an inert
  `Console`-parameter API to the canonical process-termination implementation.
  Both exact commit requests resolve through hardened Git custody under one
  canonical lineage and package key. Compiler-derived comparison emits exactly
  one changed blocking callable row and one added blocking process-authority
  row with toolchain `Console` provenance; triage blocks the capability change,
  independently recommends audit for retained process authority, and emits one
  ordinary `main.omg` source patch rather than a standalone or lineage-
  replacement packet.
  Progress 2026-08-26: `generated-table` exercises canonical observation-
  summary v24 and replay-record v6 evidence and becomes `Receipted` only after
  exact no-host replay reproduces its generated file and `include_source`
  handoff. Its package-level review baseline now captures, canonically encodes,
  recovers, and rejoins that verified replay record instead of limiting capsule
  coverage to hermetic fixtures. Broader receipted operation/output grammars
  remain.
  Remote fixture infrastructure now proves without network access that every
  pinned CathedralOS SSH and HTTPS locator normalizes to one lineage. Its
  credential-gated test resolves each standalone private mirror through SSH,
  compiles the complete resolved custody through package-aware review, and
  checks that compiler-issued identity, resolution, and source consumption
  remain bound to that normalized lineage. `graph-workbench` remains a local
  workspace-closure fixture because its byte-identical standalone mirror names
  sibling Path dependencies that are intentionally unavailable outside the
  workspace. Sealed representation mechanism/ABI evidence remains.
  Successful
  portable fixture execution now exercises
  package-facing Source/Output resolution, exact rooted evidence, explicit
  generated-source publication, and the frozen build/final checked split.
  Native-image transaction integration remains under
  `PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION`.

- [x] **REMOVE-FABRICATED-MANIFEST-TESTS.** Replace integration tests that construct
  manifests from fixture intent with locally regenerated compiler evidence.

  Acceptance: only isolated data-structure unit tests may use synthetic values;
  no end-to-end admission test can pass without compiling the fixture.

  Completed 2026-08-24: the fabricated local fixture admission integration
  test was removed. The remaining isolated synthetic legacy modules and their
  tests were deleted on 2026-08-25 rather than carried beside the corrected
  model.
  The local integration canary now regenerates compiler review evidence from
  resolver-owned fixture custody; the remote suite independently proves exact
  source custody and declaration parity, not sealed package admission.

## P8 — Declaration surface, workspace, and stdlib packaging

Settled 2026-08-25. Identity and workspace declaration work landed first. The
library relocation to `source/library/` was then performed on 2026-08-26 without
a compatibility path; remaining physical-path readers are explicitly temporary
debt in this section, not a reason to preserve the old tree.

- [x] **Retire the `PACKAGE` const and its parser.** `declaration.rs` currently
      lexes `build.omg` and shape-matches a top-level literal — exactly one
      field, named `name`, string, type `Package` — behind roughly fifteen
      error variants. Replace the whole path with `builder.package("name")`
      on the ordinary build surface that already carries `depend_as`,
      `select_provider`, and `roots.bind`.

      Completed 2026-08-25: package identity is projected hermetically from
      exactly one direct root `builder.package("canonical-name")` statement
      before build execution or dependency resolution. The compiler-owned
      `Build::package` operation is an ordinary no-op build-surface method; the
      package layer rejects helpers, control flow, expression use, wrong
      receivers or arguments, duplicate/missing declarations, and authored
      `Build`/`Build::package` vocabulary. The old `Package` prelude, constant
      parser, fixtures, and documentation were removed or migrated. The
      compiler-backed fixture migration also exposed and closed source-free
      generic-instance ownership: synthesized closed data and attached machines
      now retain their exact authored generic declaration as provenance.
- [x] **Add `builder.member(path)` and a workspace root `build.omg`.** Members
      carry paths. This is the file that tells the resolver where packages live
      and makes relocation a manifest edit.

      Completed 2026-08-25: the compiler-owned ordinary build surface now
      includes `Build::member`; the hermetic declaration projector accepts one
      or more direct canonical member paths as the explicit workspace kind and
      rejects mixed, hidden, malformed, or duplicate declarations. The root
      manifest lists std, Psi, and the compiler application in authored order.
- [x] **Add `builder.application(name)`.** `source/omega/build.omg` is
      currently an application by *absence* of a package declaration, and the
      same absence is `MissingPackageDeclaration` — an error — in the package
      reader. One file shape must not mean "fine" or "broken" depending on the
      caller.

      Completed 2026-08-25: `Build::application` is compiler-owned ordinary
      vocabulary and `source/omega/build.omg` declares
      `omega-compiler`. The unified projector distinguishes package,
      application, and workspace builds without executing them. The product
      compiler checkpoint and feature/resource profile pin the expanded build
      vocabulary and application source.
- [x] **Give `source/library/std` a `build.omg`.** `builder.package(
      "omega-language-std")`. Today the standard library has no manifest of any
      kind, which is why no git URL can name it and why minimal checkout is
      impossible.

      Completed 2026-08-25: std declares `omega-language-std`; the existing Psi
      product source also declares `psi`, so every concrete root workspace
      member has an explicit kind and stable name. Repository-level tests
      project all four real declarations through the public reader.
- [x] **Migrate free project-root build files and enforce their roles in every
      reader.**
      The declaration projector and standalone compiler now reject a role-less
      selected free build root through the same shared grammar.
      Migrate actual corpus/canary/sample project roots to explicit application
      declarations, distinguish virtual build-vocabulary fragments from project
      manifests, then make dependency projection and editing consume the same
      role projection. Do not maintain two independently parsed meanings of one
      `build.omg`.

      Audit 2026-08-25: product workspace members and package-manager fixtures
      are explicit. The remaining legacy population is primarily executable
      samples, frozen bootstrap corpus inputs, and compiler canaries. Positive
      projects can migrate mechanically while also replacing academic `b`
      receiver names with `builder`; deliberately malformed build canaries need
      case-by-case expected-diagnostic preservation so a new missing-role error
      does not mask the behavior each canary exists to test.

      Milestone 2026-08-25: package-side dependency projection now returns the
      authoritative project role and direct dependency rows from one lexer/
      parser tree. Immutable package-source binding consumes that combined
      projection rather than reading `build.omg` once for identity and again for
      dependencies. Public dependency-only projection still delegates to the
      same combined reader. The editor therefore rejects role-less files and no
      longer synthesizes a role-less build machine; malformed dependency
      canaries retain their more specific diagnostics before the final role
      gate. Standalone compiler loading and the broader positive-project corpus
      migration remain. Global enforcement is additionally blocked on owner
      question Q4: standalone compilation still treats scoped `Owner::build`
      machines as project build roots while the package declaration reader
      deliberately rejects them.

      Milestone 2026-08-26: all 140 executable sample roots now declare an
      explicit kebab-case application name and use the canonical `builder`
      receiver. A repository-level package-reader canary discovers the complete
      sample population and projects every declaration through the same
      authoritative role reader. Standalone checked compilation accepts 138 of
      the migrated roots; the remaining `file-journal` and `note-vault` roots
      reach their pre-existing duplicate-trait and Exact-cast failures after
      build loading, so role migration is not masking them. Ordinary compiler
      canaries remain the next mechanical cohort; malformed build diagnostics,
      target-only vocabulary fragments, frozen bootstrap inputs, and the five
      scoped roots remain deliberately separate.

      Milestone 2026-08-26: 1,094 ordinary compiler-canary roots now likewise
      declare explicit application roles and use the canonical `builder`
      receiver. One corpus-level package-reader test pins all 1,115 canary
      build roots, the migrated declarations, and 21 explicit exceptions. The
      exceptions are 11 malformed build diagnostics, five Q4-scoped roots,
      four target-only vocabulary fragments, and the main-source `Build`
      collision canary. That last canary remains role-less because standalone
      compilation currently lets an authored main-source `Build` shadow the
      toolchain build vocabulary; source-custodied resolution must make the
      root build entry's `Build` unambiguously toolchain-owned without making
      the main-source `Build` special. Treating `application` as a magic
      evaluator no-op would mask the collision and is not an acceptable fix.

      Milestone 2026-08-26: the ten malformed-program canaries with canonical
      free build entries now also declare application roles. Their original
      duplicate/unknown root, hosted/UEFI entry-contract, and static-machine
      refinement diagnostics remain the required outcome; role projection no
      longer has to exempt them. The corpus now contains 1,104 explicit canary
      applications and only 11 named exceptions: the intrinsically wrong-arity
      root, five Q4-scoped roots, four target-only fragments, and the
      main-source `Build` collision.

      Milestone 2026-08-26: standalone project discovery now retains the exact
      selected companion `build.omg` source identity through typed lowering.
      Build prelude detection and build-machine evaluation consume that source
      identity; they no longer rescan the expanded frontier for files named
      `build.omg` or reduce authority to machine-name strings. A regression
      imports an unrelated nested `build.omg` and proves its same-shaped
      machine remains ordinary program code. Global role enforcement remains
      open: the shared declaration projector must sit below both compiler and
      package orchestration, and the main-source `Build` canary still requires
      source-specific resolution of the toolchain build vocabulary.

      Milestone 2026-08-26: `omega-build-declarations` now owns that
      compiler-neutral role grammar beneath both orchestration layers. It
      exposes validated source/syntax projections and exact build-entry handles;
      package identity converts its validated names and member paths without a
      second validation rule, while dependency projection reuses the same
      entry instead of rediscovering the root signature. Compiler enforcement
      remains deliberately pending on the Q4 compatibility lane and the
      source-specific toolchain-vocabulary resolution above.

      Milestone 2026-08-26: the exact build source now receives one explicit
      compiler-owned top-level binding from its authored `Build` spelling to
      the injected toolchain declaration. Same-spelled declarations in
      ordinary program source retain their own nominal identity; no global
      name preference or synthetic outward spelling is used. The former
      main-source `Build` collision canary now declares its application role,
      still calls its ordinary runtime `Maker::build`, and passes. The corpus
      therefore has 1,105 explicit canary applications and ten exceptions:
      the wrong-arity root, five Q4-scoped roots, and four target-only
      fragments.

      Milestone 2026-08-26: compiler loading now projects the exact retained
      bytes of the selected free `build.omg` through
      `omega-build-declarations` before injecting build vocabulary or
      executing build code. A role-less free root therefore receives the same
      missing-kind diagnostic as package orchestration; absence of a companion
      build file remains valid for focused compilation. The 70 frozen bootstrap
      roots, the wrong-arity diagnostic canary, and the four formerly
      target-only canary roots now declare explicit application roles. The
      wrong-arity root still reaches its intended two-versus-one activation
      error. Only the five scoped roots remain outside the shared role grammar,
      isolated behind the explicit Q4 compatibility lane rather than inferred
      as another manifest form.

      Completed 2026-08-26: all 1,338 tracked free build roots declare an
      explicit package, application, or workspace role. Package identity,
      dependency projection/editing, and compiler loading consume the shared
      `omega-build-declarations` grammar. The repository corpus test now also
      parses each of the five named compatibility exceptions and requires
      exactly one scoped build machine and no free root, so an exception path
      cannot silently become arbitrary role-less source.
- [ ] **BLOCKED — OWNER Q4: retire or formally admit scoped build roots.**
      Exactly five tracked canaries remain on the standalone-only
      `Owner::build` compatibility lane. Package readers reject that shape;
      compiler loading bypasses free-role projection only for it. The remaining
      work is the Q4 language/compiler-design choice, followed by mechanical
      migration and removal or shared formalization of the compatibility seam.
- [x] **Record the bundled-core decision** in
      `wiki/design_briefs/build_and_package_model.md`: core is welded to the
      compiler because it is the language, not because nobody wrote a manifest.

      Completed 2026-08-25: the governing brief now states that
      `omega::language::core` is bundled by decision, its version is the
      language version, and two versions cannot coexist in one graph.
- [ ] **BLOCKED — OWNER Q7: remove post-relocation physical std routing.** The relocation updated
      `frontend.rs`, `stages.rs`, interpreter generation, and tests to the new
      `source/library/` location so the repository remains buildable. Those
      direct reads are temporary compatibility *code*, not a compatibility
      path. Route every std module and target/provider selection through the
      exact ordinary package graph; only `omega::language::core` remains welded
      to the compiler by language-version identity.

      Acceptance: production compilation has no physical `source/library/std`
      lookup or repository-relative std fallback; filesystem and GUI provider
      injection resolves exact graph nodes; std source is classified by its
      admitted package identity rather than residence beneath a toolchain root;
      and removing the declared std dependency rejects every std selection.

      Re-audit 2026-08-27: the existing capability-conflict regression now
      demonstrates this seam directly. A source-level `FilesystemHost` reach
      loaded through the relocated physical std path produces callable changes
      but no canonical dangerous-authority-slack row. Restoring the expected
      row by adding another path exception would be the wrong fix; exact graph
      package identity must replace physical toolchain-root classification.

      Milestone 2026-08-26: a resolver/compiler vertical canary now consumes
      the repository's real `source/library/std` through an ordinary
      `Source::Path` dependency. Resolution reads std's own
      `builder.package("omega-language-std")`, derives the requester-local
      `omega_language_std` alias, snapshots it outside the live toolchain tree,
      and hands that exact graph to package-aware compilation. Importing
      `omega_language_std::wire` retains `wire.omg` and its public declaration
      as `SourceOrigin::User` under the exact std `PackageKey`; removing the
      dependency edge makes the same import fail rather than falling back to
      bundled std. This proves the ordinary package route without Q2's remote
      workspace selector or Q3's application-root identity.

      The production switch remains open. Package-aware import discovery still
      treats every `omega::...` path as toolchain-owned, filesystem-reaching
      builds seed bundled `filesystem.omg` directly, native macOS provider
      selection injects bundled `macos_gui` bytes, and source storage classifies
      both core and std below the toolchain root as toolchain source. Migrate
      std imports to the reconciled alias, load filesystem and GUI providers
      through exact graph nodes, and classify dangerous authority from the
      admitted std/provider identities. `omega::language::core` alone remains
      bundled. Do not preserve the old std namespace through a magic mount.

      Audit 2026-08-26: the remaining production seams are namespace routing in
      `frontend.rs`, direct filesystem/macOS provider injection in `stages.rs`,
      toolchain-root source classification, build-service and target-contract
      admission, dangerous-authority classification, interpreter dispatch, and
      standalone CLI entrypoints. The ordinary local std graph and import path
      are already proven. Q7 now records the actual trust-boundary blocker: an
      ordinary graph needs an explicit exact package-role binding before the
      compiler may recognize std/provider authority. Q2 separately blocks
      selecting std from this multi-package Git repository, and Q3 blocks final
      application-root CLI integration. Neither makes package-root/local-path
      migration engineering ambiguous.
- [ ] **BLOCKED — OWNER Q2: resolve git dependencies by package name.** `declaration.rs` reads
      `build.omg` at the fetched root only, so one git URL means one package at
      the repository root. Read the root manifest, consult its members, and
      select by name — the model Cargo uses, and the one that lets this
      repository publish `omega-language-std` without splitting itself apart.

      Blocked on owner question Q2: the source request has repository and
      revision but no expected package-name selector. The selected member's own
      declaration remains identity authority; the request still needs
      unambiguous selection intent for a lockless first resolution.
- [ ] **BLOCKED — OWNER Q2: `omega fetch <package>` minimal checkout.** Once members declare paths,
      partial Git object acquisition plus parent-authenticated selective
      materialization can retrieve one subtree. This must not use Git checkout,
      which the resolver contract deliberately excludes. Consuming the standard
      library currently requires authenticating and materializing roughly 9,300
      tracked files to obtain 59.

      Progress 2026-08-26: the transport floor now requests the selected
      revision at depth one with `blob:limit=<source-byte-ceiling + 1>`, disables
      lazy object fetching during authentication/materialization, and restores
      the resolver-owned canonical bare-repository configuration after Git's
      temporary promisor setup. A required individually inadmissible blob stays
      outside quarantine custody and resolution rejects. An exact object-ID pin
      re-authenticates and reuses existing cache custody without contacting the
      remote; symbolic selectors still refetch and observe movement. The task
      remains open: until Q2 supplies a selected member path, an admissible
      whole-root package still requires every blob in its authenticated tree and
      materializes the whole root.
- [x] **Stop mirroring `filesystem_host.omg` in Rust.** `FilesystemHostOperation`
      duplicates the trait's declared machines, and two `#[test]` functions in
      `psi-checked-interpreter/src/evaluator/filesystem_host_operation.rs` read
      the `.omg` source to guard the copy. Generate or consume the declaration
      instead of testing a hand-maintained mirror.

      Completed 2026-08-25: the interpreter build now consumes the canonical
      Omega declaration and generates the closed Rust operation enum,
      declaration-order transcript tags, canonical-name lookup, and exact
      operand/result schemas. Unsupported names, duplicate Rust projections,
      unknown operand/result types, or noncanonical reach clauses fail the
      build. Only evaluator policy such as host-path exposure remains
      handwritten; the two runtime source-reading parity tests and four-way
      catalog mirror are gone.

Repository relocation is complete, including the move from `omega/language/`
to `source/library/`. This section owns the remaining semantic cleanup: std must
stop being reached by physical repository path even though that path is now in
the correct ownership tree.
