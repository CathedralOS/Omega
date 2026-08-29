# Omega Packages

This is the Rust product implementation's home for package source resolution, graph
reconciliation, admission, audit, and install/update orchestration.

The package manager is Cargo-like in workflow, not in registry model. It
resolves Git, URL, and local sources supplied by the project; it does not host
packages or trust repository names.

The security and custody rules are durable; the exact build-library vocabulary
is intentionally discovery-driven. Prefer existing Omega data, machines,
arithmetic, and provider mechanisms, and add a new public boundary only when a
real package fixture demonstrates an irreducible external contract.

## Governing model

- Every fetched package declares its own human name through the hermetically
  projected ordinary `builder.package("name")` call in its `build.omg`.
- Every build file declares exactly one role on that same ordinary surface:
  one `builder.package("name")`, one `builder.application("name")`, or one or
  more `builder.member("relative/path")` rows. Absence never infers a role.
  Identity projection accepts only direct root statements on the canonical
  `builder: &mut Build` parameter and runs before dependency resolution or
  package-controlled build execution. The compiler-neutral
  `omega-build-declarations` crate owns this grammar; package identity and
  dependency projection consume its validated results rather than maintaining
  another build-entry parser or name/path validator.
- This repository's root workspace currently names `source/library/std` and
  `source/omega`; target-neutral Psi phases are nested within the latter. Each member owns
  its declaration; paths locate members but do not name them.
- The real `omega-language-std` package has a resolver/compiler vertical
  canary: an ordinary `Source::Path` row derives the
  `omega_language_std` alias, snapshots std under resolver custody, and retains
  imported declarations as exact user-package ownership. The same import with
  no dependency edge rejects, so package-aware compilation has no implicit
  bundled-std fallback for the ordinary alias. Production still has explicit
  bundled filesystem, macOS GUI, `omega::...`, and source-classification seams;
  only `omega::language::core` may remain toolchain-bundled when those migrate.
- All executable roots under `samples/` declare explicit application roles and
  use the canonical `builder` receiver. A repository canary discovers the full
  sample population and projects each role through this package reader.
- The ordinary compiler-canary and frozen bootstrap corpora declare explicit
  application roles. Compiler loading projects the exact retained bytes of a
  selected free `build.omg` through the same declaration crate before prelude
  injection or execution, so a role-less free root cannot bypass package-side
  interpretation. The exact selected source also resolves compiler-owned
  `Build` vocabulary through a source-scoped binding, leaving an ordinary
  same-spelled program declaration nominally independent. Only the five scoped
  build roots remain in the explicit Q4 compatibility lane; they are not
  accepted as package declarations.
- `PackageName` is presentation. `PackageKey` joins the name to canonical
  source lineage. `PackageInstance` eventually adds exact source and produced-
  artifact subjects, per-subject obligation semantics, locally re-derived
  results, and disclosed open obligations. Compiler/toolchain identity remains
  separately labeled review and reproduction metadata, not a package seal.
- `build.omg` records source requests and update selectors. The dependency's
  own declaration determines its default import alias; `--as` is an
  exceptional local rename.
- Dependency-source projection is effect-free and completes before downloaded
  build code receives any host provider.
- The compiler, not the package or CLI caller, derives package capability/API
  evidence from checked source and build results.
- Ordinary admission uses a total internal projection from compiler-owned
  semantic state after successful checking into versioned canonical evidence.
  Raw compiler IR is never a lock format. The projection may read each fact
  from the earliest coherent compiler-owned representation in which it is
  semantically settled, including private pre-Psi structure, and joins required
  checked facts only after successful compilation. Totality belongs to the
  final projection, not one frozen source stage. No nominal Chi stage is
  introduced merely to collect or stabilize compiler internals.
- Checked flow now owns the first package-neutral carried-semantic-dependency
  sidecar. It retains exact nominal, layout, ownership, and automatic-cleanup
  declaration symbols with private/public disposition. The compiler review
  projection qualifies those symbols by exact package ownership and emits
  blocking canonical semantic-dependency rows; checked-tree handles never
  escape into review artifacts or lock-shaped data. Before projection, the
  compiler rederives the complete canonical sidecar from final typed and
  checked state and requires exact ordered equality, so missing, duplicated,
  reordered, or altered retained rows fail closed.
- Authored declaration identity separately rejects every exact selection of an
  owner-attached `T::drop` hook, both before checked evaluation and after late
  call finalization. Automatic cleanup is compiler-carried semantics rather
  than an authored selection; ordinary same-spelled machines do not match by
  name alone.
- Proposition/named-evidence projection joins structural typed applications to
  checked acceptance and witness disposition. Diagnostic renderings are never
  package identity; missing structural coordinates are retained in their
  existing typed or checked owner rather than motivating a report-only stage.
- Terminal evidence is separate and required only for final-realization claims
  or hardened profiles, not as a blanket package-admission gate.
- `omega.lock` records the exact reconciled closure and normalized accepted
  evidence baseline. It should normally be committed.
- The compiler's older standalone trust receipt is not package admission.
  Domain and arbitrary-string root grants reject and produce no report row or
  receipt; exact selected-provider grants remain, while exact accepted-machine
  grants are temporary standalone compatibility only. Package-aware compilation
  rejects the latter because one selector cannot admit the package's complete
  accepted-claim inventory.
- Every update receives source/provenance triage. Blocking capability/API
  changes produce exact conflicts; retained dangerous authority always
  recommends code audit.
- Dangerous-authority review classes come from compiler-owned metadata joined
  to exact reached/invoked service identities. Implemented rows mark canonical
  toolchain filesystem, machine-control, port-I/O, interrupt-control,
  interrupt-entry, and root-memory services; package-authored lookalikes and
  similarly named local traits do not acquire a class.
- Claim-free opaque boundary data remains visible as package-qualified
  representation-TCB evidence. Introduction or material change recommends
  code/ABI audit without becoming a trust claim unless exact mechanism,
  authority, executable, claim, or compatibility policy independently blocks.
  Current compiler review emits this lane for public and private opaque data
  with ABI and mechanism explicitly `Unbound`; sealed realization provenance
  remains future admission work.
- Canonical review rows carry explanatory source coordinates outside semantic
  identity. Public-trait rows now pair the outer declaration with every exact
  typed parent-identifier span under `trait_parent`. Direct authored machine,
  trait-requirement, and operator contract keyword anchors similarly travel
  under `contract_clause`; structural static-machine parameter contracts are
  walked recursively for every projected declaration family, and accepted
  claims reuse their callable locations. Checked-flow call coordinates are
  joined to exact typed statement, expression, and named-transition call sites
  under `body_call` during checked lowering, before provider settlement can
  rewrite typed call identity. Source calls must retain exactly one valid
  private authored call-selection occurrence; checked target/receiver identity
  and operational acknowledgement must agree at capture. Legitimate late-bound
  targets do not invalidate location custody, which proves authorship location
  rather than target finalization. Generated calls retain neither occurrence
  nor location. External executable-supply rows additionally retain their exact
  authored `via` occurrence under `external_binding`. Public const rows retain
  their parsed initializer under `const_initializer`, distinct from the
  declaration-name anchor. Transparent public propositions retain the full
  semantic-token extent of their authored formula under
  `proposition_formula`; primitive and witness propositions retain no invented
  formula location. Formula custody is captured at the parser boundary before
  application lowering can erase its enclosing handle or an operator-root span
  can narrow it to one token. Every authored proof fact likewise retains its
  full semantic-token extent under `proof_fact`, separately from its clause
  keyword; public domain/data facts and authored public contracts reject if
  that custody is missing. Source-free compiler synthesis receives no invented
  fact location. Public trait requirements and public data fields/cases/payloads
  retain exact declaration coordinates under `trait_requirement` and
  `data_member`; callable, public-operator, and public-trait-requirement value
  parameters use `callable_parameter`, including parameters nested in
  structural static-machine contracts. Generated declarations expose their
  real derivation origin.
  Recovery envelope v13, conflict fingerprint v16, and renderer V15 retain what
  review displays. Later nested carriers must come
  from existing compiler owners rather than package-layer source parsing.
- Missing old source escalates code review but does not prevent comparison
  against the lock baseline. Missing lock evidence causes fresh graph
  admission.
- Package review takes a caller-supplied workspace. Orchestration creates a
  fresh disposable child session beneath it, leaves resolver snapshots
  immutable, and publishes results only after successful session cleanup.
  Ordinary standalone compiler build roots remain caller-owned.

The review-only source-triage layer consumes compiler-issued closure rows
directly. Initial dangerous authority or representation-TCB exposure recommends
audit; update capability/API drift and source-lineage replacement block;
unavailable old source and retained dangerous authority recommend audit even
when canonical capability bytes are unchanged. Its bounded model-facing form
contains only fixed reason/disposition vocabulary plus canonical package-key
commitments and rejects rather than truncates. A separate bounded source packet
compares exact-key resolver snapshots directly and binds both immutable
resolutions. It renders deterministic line hunks plus directory, executable,
symlink, and entry-kind changes under independent source, metadata, line,
diff-work, trace-memory, and output ceilings. Paths and source lines are
byte-escaped into fixed lanes but remain hostile code data; binary/non-UTF-8
changes retain size and content commitments and require standalone audit. The
review-input join requires a complete candidate closure matching every
compiler-issued key and immutable resolution, validates each recovered baseline
custody against its compiler row, and derives missing-old-source state itself.
One shared review-only validator also rejects duplicate compiler rows,
package/projection identity mismatch, mixed deployment targets, and mixed
compiler-executable commitments before capability comparison or source-packet
assembly. These checks establish review custody only; they do not issue an
accepted lock or settle complete toolchain provenance.
Review baselines can now cross a process restart without old source custody. A
bounded binary capsule retains the complete typed source graph, immutable
resolutions, target and comparison commitments, every canonical comparison
row with its exact source sidecar, and any compiler-verified bounded source-read
replay record. Compiler-owned decode derives row kind, risk,
key, package, and target from the canonical row frame and rejects malformed or
noncanonical source coordinates; package code leaves row payloads opaque.
Recovered rows remain a distinct review-only type and cannot become newly
compiler-issued evidence. The outer decoder checks its corruption checksum
(not authenticity or proof of review), canonical re-encoding, strict
package/row order, singleton header/provider rows, graph closure and depth, and
independent resource ceilings. Replay-record recovery requires the exact
semantic schemas and complete operation-specific lane relationships; capsule
v2 binds the opaque record to its parent build observation and charges all
records to one aggregate byte budget. That association is consistency checking,
not authenticity or admission. Recovered baselines produce the same conflicts,
triage, and source packets as live baselines, including standalone candidate
packets when old source is unavailable. This capsule is not `omega.lock`: it
cannot issue a package instance, resolve a conflict, or mutate a project.
Its aggregate bounded renderer frames compiler-only triage separately from
hostile source lanes. The runner-neutral advisory boundary passes fixed system
instructions separately from that bounded evidence; the package library chooses
no model and supplies no ambient network authority. A caller-supplied output
ceiling is enforced by an Omega-owned sink into which the runner streams. Only
the exact canonical result envelope with one of two tokens—`recommend_audit` or
`no_additional_audit`—is accepted, with no prose. Advice may add an
audit recommendation but cannot suppress compiler recommendations, alter
blockers, prove an audit, resolve conflicts, admit a package or evidence, set
policy, or mutate project state. The outcome carries a commitment to the exact
rendered input. Concrete provider/configuration and CLI wiring remain.

The complete design is in:

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`
- `SOURCE_RESOLVER_SECURITY.md` for the resolver helper, snapshot, sandbox, and
  receipt boundary.

## Trust status

The crate is not yet an accepted end-to-end package-admission implementation,
but its release surface contains reviewed corrected-model source, identity,
closure, compiler-review, conflict, baseline, and triage building blocks. No
manifest-file, receipt-file, lock-assembly, or plan CLI is a production trust
boundary.

The removed prototype encoded the following superseded assumptions. They must
not return to the release path or its tests:

- locks keyed by package-authored name alone;
- mandatory caller-supplied alias and package name;
- caller-constructed `PackageCapabilityManifest` values;
- standalone JSON manifests accepted as compiler evidence;
- a manifest fingerprint without the normalized accepted baseline;
- free-form reviewer/reason receipts that accept whole sections; and
- reintroducing standalone dependency scanning or identity-free package roots.

The production source-custody and typed graph paths have received focused
review; the hardened native resolver boundary, sealed admission projection, and
accepted-lock path remain incomplete.

The sibling `omega-resolver-execution` crate now supplies the concrete macOS
engineering floor: closed discovery/initialization/fetch/inspection phases,
Seatbelt write/network/exec canaries, inherited Unix rlimits, and opaque bounded
policy observations issued from the same inputs as each native command.
Successful Git resolutions retain every configured-command row, including the
generated policy hash, exact numeric ceilings, normalized executable path set,
discovery/inspection content-read roots when applicable, and mutable root. Every macOS
phase uses a host-profile-free default-deny policy with exact selected
executables and write-data to `/dev/null`. SSH discovery/fetch retain broad
reads. Initialization, inspection, and HTTPS discovery/fetch confine metadata
and content to their exact phase root plus exact executable/runtime paths and
required literal ancestors, and therefore mark their filesystem-read rows
enforced. HTTPS discovery/fetch also admit metadata-only lookup within their
compiler-selected Git helper directory and fixed `/etc/ssl` alias path, and read
the fixed `/private/etc/ssl` system TLS configuration root; this does not
establish TLS trust or custody.
Initialization and fetch additionally confine writes to the exact mutable
quarantine root. Discovery and fetch require one closed authority matching the
already-validated HTTPS or SSH transport and admit outbound network. Only SSH
receives exact OpenDirectory libinfo, hostname, and Rust runtime page-size
reads for the pinned client and connector; HTTPS receives none. Both route
through an exact compiler-owned loopback broker. On macOS direct child egress
outside that broker port is denied; Linux and Windows retain unavailable
endpoint-confinement rows. Initialization and
inspection deny network, reject transport authority, and omit process-fork
authority, so even allowlisted executables cannot become descendants in those
phases. The applicable write/network/exec and nonnetwork descendant rows are
enforced; strict checking still rejects the remaining
unavailable guarantees.
Before returning a successful resolution, the package layer requires one observation
per bounded launch and exact equality between every observed allowlist path and
the verified content identities for Git, the selected transport, and fixed
platform helpers; all helper identities remain in the result. Each completed
command also retains a domain-separated digest of
its actual program, ordered arguments, sealed environment, cwd, and stdin class,
then exact exit/signal and bounded stdout/stderr length/digest results joined to
the corresponding native policy digest. Both streams and every command charge
one overflow-safe cumulative captured-output counter under
`min(source-byte ceiling + 64 MiB, 576 MiB)`. Counts must match before success,
and the counter must exactly equal the sum of all retained stream lengths.
Every discovery/fetch route also shares one separately derived bidirectional
broker-transfer ceiling with the same formula. Endpoint events retain uploaded
and downloaded bytes accepted for relay; CONNECT framing and DNS are excluded.
Any ceiling event rejects, and successful issuance requires the event sum to
equal the live whole-resolution counter.
Only after cache custody, executable content, those command rows, authenticated
Git objects, and a final physical re-read of the immutable snapshot all
reconcile under the retained cache lock does the resolver seal its private
pending result and issue one compact canonical final-result observation. The
public result exposes read-only accessors rather than mutable evidence fields.
The observation also binds the source ceilings, request/selector, object format
and identities, snapshot subject, tool identities, cumulative captured-output,
and directional broker-transfer ceiling and counts. The fixed outcome is
explicitly `resolved-non-admitting`:
unavailable native guarantees remain unavailable. Linux/Windows strict
backends, TLS/SSH credential evidence, direct-egress prevention outside macOS,
object-store and non-Windows descendant aggregate-resource accounting, and the
complete source receipt remain open, so this does not promote diagnostic source
commands into admission.

The crate now contains reviewed building blocks for immutable Git/local
snapshots, hermetic package-name extraction, and typed package/source identity.
The obsolete JSON source-cache policy record and its CLI persistence command
are deleted rather than retained beside the sealed in-memory resolver
observation. `omega audit source` remains a non-admitting live diagnostic; no
caller-readable intermediate record can be promoted into future receipt or
lock authority. Git reports expose the compiler-selected broker-transfer
ceiling and observed upload/download counts; local reports omit those
inapplicable rows.
Ordinary command paths now create and retain one versioned private per-user
source root beneath the platform cache location. Compile dependency closure,
source inspection, and source audit use that capability; they no longer choose
a project-local cache, accept an ambient cache override, or fall back to the
host temporary directory. Every manager-owned root component is private. Git,
workspace-member, and external-local lanes are retained separately and
reconciled around ordinary closure operations. The public source-audit path now
requires storage; its caller-path implementation is internal-only. Other
explicit-path resolver harnesses remain while their tests and deeper
acquisition APIs are migrated to carry retained lane handles directly.
Mutable local-package snapshots omit only `.git` metadata and the reserved
root-level `build/` compiler output; package-authored ignore files do not control
source identity, nested `build` directories remain source, and immutable Git
materialization remains an exact selected-tree check. Local directory listings
are bounded before sorting, and Git paths/symlink targets receive a
host-independent Windows portability preflight before materialization. Git
fetch requests only the selected revision at depth one, disables automatic
maintenance/GC, and asks the remote to omit every individual blob larger than
the accepted source-byte ceiling. Lazy object fetching is disabled while the
parent authenticates and materializes the selected graph. Git's temporary
promisor configuration is replaced with the resolver-owned canonical bare
configuration before further processing. Restoration publishes exact
synchronized bytes from an open handle through a handle-relative atomic rename
and confirms the resulting pathname identity; it no longer deletes `config`
before recreating it. A required omitted blob therefore rejects without
entering resolver custody. Exact object-ID pins re-authenticate and reuse an
existing cache entry without transport; symbolic selectors still refetch. This
is a transport floor, not selective package checkout: until the resolver has an
exact selected member path, every admissible blob in the whole authenticated
root is still required. Broker-routed bytes are now strictly bounded, but a
universal transferred-byte guarantee still requires endpoint confinement on
every execution backend; object-store quotas remain separate work.
Selected Git objects are not trusted merely because Git named them. Before any
snapshot stage exists, an exact requested object ID must equal the selected
commit. The parent recomputes SHA-256 commit and blob IDs, computes SHA-1 with
collision detection, checks the commit's root-tree edge, retains and verifies
every explicit child-tree edge, reconstructs the canonical recursive tree
including empty subtrees, compares its Merkle root, and preflights every
materialization destination. Empty subtrees remain explicit directories in the
immutable snapshot. Collision detection keeps legacy SHA-1 repositories usable;
it does not make SHA-1 collision resistant, and SHA-256 remains preferred. This
strengthens source custody; it does not turn the still-unisolated resolver into
an admission boundary. Reuse re-authenticates the selected Git graph and compares
the published snapshot directly with the resulting source identity. Rewritable
snapshot metadata is checked for consistency but never acts as the content
baseline.
Git cache repositories no longer persist a remote origin. Fetch receives the
exact resolver request directly, and the parent writes and byte-compares one
canonical SHA-1 or SHA-256 bare-repository configuration without asking Git to
describe it. Git and local cache trees receive a bounded parent-owned custody
walk before and after use. Unix nodes and locks must belong to the effective
user and reject group/other write authority, replaceable non-sticky ancestry,
or special kinds. On macOS every ancestor, cache, publication, staging, and
lock node additionally rejects any native extended ACL allow entry; deny-only
ACLs do not broaden custody, unreadable ACLs fail closed, and symlinks are
inspected without following them. The same walk applies source-scaled,
absolutely capped
logical resident-byte ceilings to accepted Git entries and local publications.
On Windows, every cache root, retained directory, regular file, reparse point,
and lock is inspected through its no-follow retained handle. Cache objects must
be owned by the current process user; ancestry may instead be owned by that
user, LocalSystem, or BUILTIN Administrators. A null DACL, an unknown granting
ACE form, or mutation authority granted to any other SID rejects without
resolving principal names. Inherit-only ACEs are ignored for the current
object, while inheritable ACEs that also apply to it are checked normally.
Git lock waiting consumes its ten-minute whole-resolution budget; local
snapshot publication lock waiting has a separate compiler-owned two-minute
deadline and rejects explicitly instead of blocking indefinitely. Those
post-helper checks can reject an oversized cache but cannot prevent
temporary disk exhaustion during an unconfined fetch. Hostile trusted-principal
racing and complete native isolation remain open.
Symbolic selectors use a bounded remote advertisement only to choose the
quarantine's SHA-1/SHA-256 object format; malformed, absent, or mixed formats
reject, and the advertisement never substitutes for parent authentication.
The Git parent executable is selected from closed absolute concrete platform
paths, not ambient `PATH`; macOS excludes Apple's dispatcher. Its canonical
bytes are retained as a diagnostic observation, guarded by stable file identity
around every launch, and re-hashed when resolution returns. Git receives a
cleared, fixed environment and an explicit absolute working directory. The
executor grants exactly one Git protocol per validated request (`https` or
`ssh`); the test-only local-repository adapter alone grants `file`. The cache
key and exact metadata bind that execution profile even when hosted-repository
lineage normalizes HTTPS and SSH clone spellings together. Resolved-source
diagnostics retain the profile separately. This closes ambient parent-executable,
environment, and cross-protocol selection. HTTPS additionally resolves
`git-remote-https` from a closed install-relative candidate set, retains its
invocation entry and canonical target, and exposes only that observed helper
directory through `GIT_EXEC_PATH` and `PATH`. SSH retains and rechecks the
selected client's exact path and content. Both use the same Unix custody floor
as the parent Git executable and are rechecked around every launch and at
completion. On macOS that floor also reads native extended ACLs for invocation
entries and canonical targets; any allow entry rejects even
when ordinary mode bits appear safe. Deny-only entries do not broaden custody,
and an unreadable ACL fails closed. Windows applies the equivalent closed
owner/DACL mutation-authority policy through retained handles to invocation
entries and canonical targets. Cache policy v28 separates entries
predating this transport-executable, cumulative-output, endpoint-brokerage,
network-transfer, nonnetwork descendant-denial, content-read, and nonnetwork-
metadata/HTTPS-network-metadata/deadline/Windows-Job/Windows-custody floor. HTTPS receives
an exact command-scoped proxy. SSH uses the separately
custodied `omega-resolver-connect` companion as a fixed ProxyCommand; compiler-
authored environment fields carry the broker and normalized target without
placing locator text in shell syntax. This does not certify any executable or
establish TLS/SSH host trust. SSH is noninteractive and strict about host keys,
but still consumes
the user's default known-host and key files. Strict OS confinement, explicit
credential custody and during-write byte/resource enforcement remain. Ordinary
resolution is now bounded to 64 Git launches, independent of package file
count, ten minutes including cache-lock acquisition, cumulative parent-captured
output, and broker-routed bidirectional bytes. Each byte ceiling is separately
derived as `min(source-byte ceiling + 64 MiB, 576 MiB)`. Neither is an
object-store or universal descendant aggregate-resource quota, and the transfer
ceiling does not prevent direct egress on an unconfining backend. On Windows,
the resolver-owned Job Object separately enforces 16 active processes, 2 GiB
committed memory per process, 4 GiB aggregate committed memory, and 120 aggregate
user-CPU seconds; filesystem, executable, and endpoint confinement remain
unavailable there. Here executable confinement means constraining which images
descendants may load; selected resolver executable owner/DACL custody is
enforced separately.
Validated blobs use
one exactly framed `cat-file --batch` launch; blob payloads are shared ranges
over that bounded response and released before staged-source revalidation.
Each command reserves cleanup/reaping within its existing deadline: at most two
seconds for ordinary budgets and one quarter of a smaller budget. Cleanup
therefore no longer receives a compiler-authored extension beyond either the
command or whole-resolution interval. Host scheduling and uninterruptible
kernel work are still not a hard wall-clock guarantee.
Git, workspace-member, and external-local resolution now bind those pieces into
a `ResolvedPackageSource`: declaration and identity come from the immutable
snapshot and canonical source lineage, and canonical literal dependency rows
are projected without executing build code. Role and dependency rows come from
one parsed `build.omg` projection; absence is not an implicit project kind, and
the dependency editor cannot synthesize a role-less build machine. Known-host
lineage normalizes GitHub and hosted GitLab HTTPS/SSH clone spellings; GitLab nested namespace
paths remain exact and case-sensitive, while self-hosted and unknown hosts
retain transport distinctions. Workspace-member resolution binds
the workspace root lineage to a normalized member-relative path, verifies the
live member is the matching strict canonical descendant, and snapshots only
that member. An explicit external-local closure adapter instead binds every
relative or absolute local Path request to the same supplied consuming context,
retains each canonical absolute lineage, and snapshots each package without
ambient workspace/lock discovery. A contextual workspace entrypoint may route
an escaping live-workspace Path row into this lane; the strict entrypoint and
all fetched Git snapshots remain confined. A transport-neutral recursive
resolver
accepts only erased custody derived from these resolved sources, delegates each
request to an adapter, and
returns the complete validated `ResolvedPackageClosure` together with every
exact immutable custody root. It derives ordinary aliases from fetched package
declarations, preserves explicit aliases, reuses identical custody, and reports
all requesting paths when one package key resolves inconsistently. Package,
dependency-request, and depth ceilings bound hostile closure traversal.
The first concrete adapter roots traversal in an explicitly supplied workspace
member, resolves requester-relative Path rows only within a registered
workspace, and resolves Git rows through immutable Git custody. A fetched Git
snapshot becomes a separate registered workspace for its own nested Path rows;
the adapter never searches parents or guesses an external protocol.
Toolchain/compiler evidence is intentionally absent from source resolution,
and this closure has no persistence, lock, or admission API. `PackageKey` also
derives the opaque stable identity carrier used by package-aware compiler
inputs. The compiler's separate native and checked package entrypoints consume
a closed requester-local alias graph and canonical source roots without
consulting downloaded dependency rows. Standalone compilation never interprets
dependency declarations and resolves only ordinary root-relative and toolchain
imports. A package-side handoff translates only the validated custody closure
into compiler inputs, whose constructor independently canonicalizes and checks
every root and edge again. Review orchestration compiles every package in
deterministic dependency-first order, temporarily re-rooting each package over
only its transitive dependencies. The caller supplies a workspace;
orchestration creates one fresh disposable child session beneath it and assigns
package-and-source-specific writable roots inside that session. Resolver
snapshots remain immutable. Orchestration retains results privately, cleans up
the complete child session, and publishes the review rows only after cleanup
succeeds; cleanup failure rejects the review. It returns non-constructible
review rows carrying the
selected `PackageKey`, immutable resolution, compiler projection, and canonical
comparison bytes. Every transitive snapshot is re-hashed under its original
resolver limits before and after each compilation. Package-aware checked
compilation emits a separate source-consumption commitment over the canonical
root/package/alias graph and every exact loaded package/toolchain source path
and byte sequence; absolute cache paths and load order are excluded. The
compiler re-reads those physical paths before returning, and orchestration does
so again after its whole-snapshot post-check. This remains review-only custody
association: hostile same-user racing, whole-compiler/toolchain commitment,
and sealed completeness still gate any accepted instance or lock payload.
Dependency-first review also retains one opaque compiler-issued generated-
source bundle per checked package. Every consumer receives the complete bundle
set for its transitive dependencies, including explicit empty bundles. The
compiler validates producer identity, target, dependency closure, and same-run
source-consumption association, then loads retained generated bytes under the
producer's package identity before the consumer frontend. Generated imports do
not read Output or rerun a dependency build, and the consumer source commitment
includes the injected bytes. The bundle has no public constructor or recovery
format and is not admission evidence. The real `generated-table` consumer
canary is presently ignored at OWNER Q7 because relocated std filesystem
authority lacks its exact ordinary-package role; adding a path/name exception
would be a security regression.
Review orchestration also commits to the exact bytes readable at the current
producer executable path before reviewing a closure and checks the observation
again afterward. Every row in that review set retains the same verified
compiler-executable commitment, separately from canonical capability/API bytes.
This identifies an observed producer artifact only: it does not certify the
compiler, bind its complete source/toolchain closure, establish a reproducible
build, or prove that the file equals the process image already loaded by the
operating system.
Selected build-machine execution also retains a separate versioned observation
summary. The compiler derives its static ceiling from exact reachable canonical
toolchain filesystem service identity and records whether that host family was
actually invoked. The current scoped real provider has no replay transcript, so
filesystem use is `Volatile`; pure, console-only, and declared-but-unreachable
filesystem rows remain `Hermetic`. Console-only execution is not supplied real
filesystem authority. Filesystem dispatch also requires an exact canonical
toolchain requirement symbol in statement and value position; package-authored
lookalikes cannot consume the provider merely because granted mode was selected.
The exact canonical signature then selects a closed, explicitly tagged
operation identity shared exhaustively by both providers; aliases remain
distinct. Future rooted transcripts must handle potentially absolute
`read_link` output and necessarily absolute `canonicalize` and
`final_path_name_by_handle` output.
Observation-summary schema v22 carries operation-attempt schema v18, retaining exact
providers, operation tags, normalized results, post-error state, and every direct
scoped path authorization in successful-run call-start order. Authorized paths
use closed Source/Output identities and canonical slash-separated relative
UTF-8 bytes, never physical compiler/cache roots; operand ordinal and access
remain explicit. Ambiguous/unresolved roots, unrepresentable rooted paths, and
the 16 MiB retained-path ceiling reject before host access; ceiling exhaustion
is a non-catchable evaluator resource halt. Grant denials remain distinct,
while a host error retains a prior authorization without fabricating a refusal.
Granted evaluator failures retain partial typed outcomes; worker
failures mark evidence unavailable, and Omega emits no package review row.
Descriptor, native, and find inputs retain Resolved/Null/Unknown logical
lifetimes immediately after successful typing. A later preparation failure
keeps the completed prefix, while a fully prepared call must reproduce the
exact logical-handle plan. Successful opens mint monotonic IDs, duplicate and borrowed outputs
bind their source, and successful closes retain every invalidation. Failed
closes retire nothing and reused provider tokens receive fresh identities. A
token live in another logical domain rejects before provider access; provider
acceptance of an otherwise Unknown token traps. Virtual duplicates share
their source cursor. Real descriptors retain rooted write authority through
duplicate and borrowed views; content, extent, metadata, ownership, and
host-lock mutation deny before sponsor or host access when the descriptor came
from source-read authority alone. `open_at`/`unlink_at` accept one portable
relative component; real-provider path outputs are lossless or reject.
Successful handle-valued operations retain only their logical result identity;
provider token integers never enter compiler/package evidence. Non-handle
results and failed handle-result sentinels remain exact scalars, and package
commitments type-tag both lanes. Fully prepared calls whose evidence
reservation succeeds retain ordinal-ordered non-handle I32/U32/I64/U64 scalars,
exact authored immutable write/FILETIME payloads, and validated at-family
component bytes. Mutable byte and i64 carriers retain a distinct complete
resolution-time snapshot as their operands are evaluated, so later preparation
failure keeps the prefix. Mutable byte carriers separately retain complete
provider pre/post capacity, including unchanged tails, while mutable i64
carriers retain exact provider pre/post values. Provider pre-state follows all
authored argument evaluation because a later argument may alias an earlier
carrier; the snapshots need not match. Post-state follows provider return or
halt, and input-only mutable ABI carriers remain explicit even when unchanged.
Rooted/path-alias spellings stay out of the payload lane. A separate 256 MiB
aggregate operand-evidence sponsor covers immutable bytes, exact path-like and
rooted-resolution bytes, exact returned-path prefixes, one mutable resolution
copy, and both provider copies;
prior or nested staging effects remain cleanup-
contained. Directory-entry names, symlink targets, find patterns, and other
non-rooted path-like operands occupy a distinct ordinal-tagged lane, are
retained incrementally across later preparation failure, and are bound by the
package commitment without rendering bytes as text. Rooted-path preparation
prefixes occupy a separate lane with exact ordinal, closed
Source/Output identity, and canonical relative bytes captured before physical
provider-path lowering. They survive later preparation failure and a fully
prepared call must reproduce the complete compiler-private semantic sidecar.
This is input resolution, not authorization; later grant resolution separately
retains access and may select a different canonical rooted location. Successful
provider write branches retain exact meaningful `read_link`, `canonicalize`,
and final-path bytes without NUL terminators or stale tails, plus output ordinal,
closed kind, and Complete/LimitReached disposition. Exact provider target length
distinguishes an exact-fit `read_link` from truncation; failure and insufficient-
capacity returns carry no output row. Package-rooted builds reject the two
always-absolute operations, while `read_link` remains inert payload. Successful
`read`/`read_at` observations designate the exact prefix of the
already-custodied mutable post-carrier with a closed sequential/positioned kind,
output ordinal, and returned length. EOF retains a zero-length row; failure
retains none. These zero-copy rows add no byte-sponsor charge. Package
commitments bind their kind, coordinates, and referenced mutable post-state.
`read_dir` similarly designates exact `DirectoryRecords`; `find_first` and
entry-producing `find_next` designate their complete 320-byte `FindEntry`
record. Directory EOF and no-entry find returns retain empty rows, while failed
enumeration emits none. Successful path, descriptor, and no-follow metadata
operations retain one target-neutral canonical row containing all 14
`StatRecord` fields. Omega extracts and validates the selected target's checked
`StatLayout<StatRecord>` from private typed/layout state and gives only that
closed descriptor to the Psi evaluator. The evaluator zeroes and fills the
complete authored ABI carrier (whose API minimum is 144 bytes) through the
descriptor and cross-checks it against the semantic row. Package commitments
bind both representations. This creates neither a public internal-IR contract
nor nominal Chi. Complete replay remains absent, so this is still an incomplete
trace rather than a transcript or receipt.
One or more complete, non-interleaved Source-rooted source-read chains receive
bounded compiler replay with no filesystem provider. Each chain contains one
flags-zero `open`, one or more `read`/`read_at` calls on its distinct created
descriptor, and its exact retiring `close`. Sequential reads advance that
chain's implicit zero-based cursor by their successful result; positioned reads
bind an exact nonnegative offset and do not advance it. Ordered operation kinds,
counts, offsets, results, mutable carriers, and observed regions determine each
cursor without a separately trusted field. Replay requires exact event order,
inputs, outputs, exhaustion, and final result; zero reads, failed reads,
descriptor reuse, cross-chain operations, interleaving, and incomplete chains
reject. The package commitment binds this partial replay fact. Summary v22 and
compiler replay-record v4 retain the exact verified chains, while review-
baseline capsule v2 preserves
it across restart as opaque, bounded, non-admitting bytes associated with the
parent observation. Checked compilation can strictly rehydrate those bytes into
the PSI executor's exact typed source-read chains and reevaluate the
selected build machine without a host filesystem provider. Recorded source
bytes are used even after the corresponding host file changes, while changed
authored paths, counts, positioned offsets, operation or region kinds, and
event structure reject. This creates no host authority,
authenticity claim, admission, public IR contract, or nominal Chi. It does not
make the build `Receipted`; full operation coverage, output mutation and staged-
tree reproduction, package-command integration, and a complete replay verdict
remain open.

Summary v23 and replay-record v5 make the same artifact an ordered source-input
stream. Successful Source-rooted `read_metadata` and
`read_symlink_metadata` events can surround closed read chains while retaining
the authored rooted input separately from the authorized target, the exact
follow/no-follow semantic row, all 14 metadata fields, and the complete
selected-target carrier. Canonical recovery validates the event shape and both
canonical relative paths. Provider-free replay reconstructs the checked
`StatLayout` carrier and compares every field, zero padding, and tail byte
before requiring exact stream exhaustion. Failed and descriptor-backed
metadata remain outside this rung. Review-baseline capsule v2 needs no framing
change because the embedded record is already versioned, bounded, and opaque.

Summary v24 and replay-record v6 add the first complete replay verdict without
claiming broad filesystem coverage. The admitted grammar is one or more of
those Source-input events followed by one Output-rooted direct-child ordinary
file: exact `create(438)`, one full immutable `write`, exact retiring `close`,
and one matching generated-source handoff. Replay serves Source observations
from the retained record but executes Output mutation in a fresh virtual
namespace, requires exact event/evidence/result equality and namespace
quiescence, and reconstructs the canonical one-file tree. Initial package
review issues `Receipted` only after that independently reproduced tree equals
the separately sponsored physical staged tree. Unsponsored output cannot mint
the record. Reopened custody repeats the no-host execution and restores the
generated source without reading drifted host Source or Output bytes. Static
reach still has a `Volatile` ceiling; broader operations and output shapes
remain realized `Volatile`. Operation schema v18 and baseline capsule v2 do not
change. A separate 16 MiB aggregate retained-evidence ceiling rejects before
replay cloning; validated attempts are shared across evaluator handoff.

Summary v26 and replay-record v8 additionally admit one exact successful
Source-rooted flags-zero `open` -> `read_file_metadata` -> `close` event. Its
descriptor is created and retired inside that indivisible event, and replay
retains the exact `OpenDescriptor` row, mutable carrier states, descriptor
lineage, results, and post-error states. The event composes with the ordered
Source stream and existing Output grammar, but arbitrary descriptor-metadata/
read interleaving remains outside the rung. Recovery rejects changed kind,
ordinal, lineage, carrier shape, operation order, missing close, and descriptor
reuse. Filesystem-attempt v19 and baseline capsule v2 remain unchanged.

Raw byte-valued inputs are evaluated once by the shared preparer and reject
above the current 16 MiB evaluator sponsor ceiling before provider cloning/
allocation. Read/count capacities use one checked conversion and reject
negative, wrapped, or above-ceiling values. Before either provider receives a
call, one shared closed preparer checks exact arity, evaluates all authored
operands left-to-right exactly once, rejects wrong scalar/byte shapes, and
retains validated mutable cells and capacities, including the fixed Win32
`OVERLAPPED` input. This includes otherwise-unused
ABI operands; the canonical trait test pins all 50 operand schemas and result
widths. Preparation traps remain on the outer attempt and occur before grant or
provider access. Canonicalize enforces its declared 1024-byte `PATH_MAX`
carrier at that gate; process memory and CPU ceilings remain open.
Scoped hard links require write authority on both names, preventing a read-only
source inode from being aliased into writable staging. Namespace mutations
authorize the canonical parent and actual leaf rather than following an
existing leaf symlink, preventing a target inside staging from lending write
authority to an outside name. Target-following operations still authorize the
resolved target. `ReviewBuildSession`
owns one object/namespace sponsor shared by every package compile in the
closure. Package output roots consume the same account. The compiler permits
4,096 entries, 256 MiB total logical bytes, and 256 MiB for any one object
extent; hard links do not double-count bytes, symlink spellings do count, and
open-but-unlinked files remain charged through their final descriptor. Every
mutation reserves its candidate accounting state before host mutation and
commits only after success. Ceiling refusal is resource exhaustion rather than
a host errno or generic evaluator trap. Path-summing and per-package limits are
intentionally rejected designs.
Compiler-issued package review carries these summary fields outside canonical
capability/API comparison bytes. The summary fields alone are not a receipt and
make no replayability or source-rebuildability claim; only the exact v24/v6
grammar above may combine them with verified operation replay and reproduced
tree equality to issue `Receipted`.
After a successful sponsored package build has released its filesystem
provider and descriptors, the compiler also commits the complete fresh Output
tree before orchestration deletes the disposable review session. A successful
empty tree has its own commitment. Canonical entries use sorted Output-relative
UTF-8 slash paths, explicit directory/file/executable/symlink modes, file length
and content digest, and exact validated relative symlink spelling. Empty
directories participate; timestamps, ownership, ACLs, ambient permission bits,
host roots, inode numbers, and hard-link topology do not. Sponsor namespace
groups and extents are cross-checked, byte-identical content is counted once
without retaining hard-link topology, and
unknown kinds, external symlinks, non-portable paths, open descriptors, prepared
transactions, custody mismatches, or ceiling excess reject review. The initial
ceilings are 4,096 entries, 256 MiB unique file content, and 16 MiB aggregate
path/target bytes. Ordinary caller-owned build roots are not claimed as package
output trees. The package observation commitment binds the tree digest and
counts, so a change recommends review through the existing build-observation
triage lane. Review now retains that canonical content behind a compiler-owned
carrier after the physical session is deleted. It can materialize into an
existing empty concrete directory and independently re-inspect every path,
kind, mode, symlink target, and file byte before returning the same commitment.
In isolation this is staged-tree replay, not operation replay, a receipt, or
generated-output handoff. The exact v24/v6 grammar above separately joins this
custody to verified operation replay. Hostile same-user racing remains outside
this custody rung.
Checked package compilation now also retains
the exact root package and selected build-machine symbol and can emit an
in-memory authority review projection for one explicit target. That projection
is intentionally not complete source/toolchain-bound admission evidence.
Authored toolchain nominals now retain a domain-separated commitment over the
canonical toolchain-relative source path and exact bytes. Compiler-generated
symbols inherit the package/toolchain provenance of a mandatory authored
derivation origin, while unsupported source-free symbols remain unresolved.
The 22 exact compiler-installed builtin types and 71 compiler-installed builtin
functions use closed compiler atoms selected by root slot and symbol kind rather
than spelling. Same-named package declarations and source-free generated
symbols do not classify as those atoms. Compiler carry aliases use
closed `CarryPermission` atoms in a distinct non-nominal lane; other compiler-
semantic atoms remain unresolved until they gain closed carriers. Arithmetic
domains and aggregate carry policy are already closed enums. Typed domain
constraints now distinguish declared, carry, value-domain, and `OmegaLayout`
subjects; layout retains a closed grammar and exact structural schema argument.
Review rejects legacy/unclassified layout forms, malformed compiler subjects,
residual const calls, and unsupported or unselected index expressions. The
v41 projection also decodes source-unspellable canonical-const transport atoms
into closed type/value terms, excludes their diagnostic display text, and
encodes decimal const values numerically. Const values are accepted only in an
exact declared const slot; const binders must reconcile uniquely to the exact
alpha-normalized telescope, while residual const declarations and unresolved
source spellings reject. The projection uses that same exact source commitment
inside every structural type identity. The frontend `SourceId` used to join a
nominal to its source never enters the
canonical bytes, and a missing join rejects review rather than substituting the
weaker generic toolchain marker. The projection includes selected provider
mechanisms, and provider plans/trust rows retain
exact package owners for the realizing machine, provider type, service schema,
and requirement owner. Review v41 additionally binds the exact schema,
provider-type, requirement, and realizing-machine declarations as canonical
nominal identities, so readable plan/overload strings are never declaration
identity. Projection also verifies each declaration owner against the selected
plan: an owned declaration must match its exact `PackageKeyIdentity`, while a
package-less plan declaration must have an exact authored toolchain-source
identity. Package-less user source, unresolved/source-free ownership, and every
owner mismatch reject rather than entering review evidence.
The selected-provider source sidecar independently anchors every exact
requirement declaration and realizing machine under distinct fixed roles; a
conflict therefore binds both sides of each realization row to the source shown
to the reviewer.
Checked-adapter bindings resolve by canonical overload plus exact package owner
without a short-name fallback. Authored provider choices retain two structural
type paths, resolve to exact typed trait/data symbols, and match plans only by
package plus canonical path. The selected plans remain intact through cycle,
ABI, and checked-fact construction; package-distinct same-spelled slots and
providers do not collapse. Remaining grant joins, whole-compiler and source-free
compiler-intrinsic toolchain identity, and the remaining
trust/proof/reproducibility joins are incomplete. Build-bound
progress obligations retain and match package ownership for both service and
requirement, and retained selected-provider facts expose no name-only plan
lookup. Installation-bound reach, termination, mutation, crash, and permission
frontier rows now use normalized package-owned semantic paths, and crash
predicates retain their existing source-independent canonical identity. Review
identity retains the exact deployment profile rather than collapsing profiles
that happen to share a native ABI. Capability-flow states, including propagated
`via` states, are package-qualified. Ordinary public-machine visibility now
survives checked compilation; public omission enforces empty reach, invocation,
suspension, blocking, and crash ceilings. Exact source-body presence survives
resolved and typed copies. Review v41 uses inferred transitive reach only for
actual checked bodies and records bodyless supply explicitly instead of copying
its published ceiling into a false realization. Its concrete row retains the
preselection body base rather than reconstructing it from the current callable;
that base is not final selected-provider evidence. Dangerous declared-but-unused
authority on checked bodies emits exact callable-and-service slack rows with an
audit-recommended risk; bodyless supply and package-authored lookalikes do not.
The review includes public and
boundary callables plus the selected build machine, excludes private machines,
and projects invocation targets as exact parameter ordinals or package-qualified
service identities. Package-qualified type identity gives every non-binder
nominal an exact package or authored toolchain-source owner while preserving
owner-free alpha-normalized binders. Review v42 and canonical row v2 also encode
concrete proposition type arguments through that structural lane, so compiler builtins use closed
atoms rather than declaration-shaped placeholders. Unresolved nominal ownership
rejects exact type projection and canonical encoding instead of becoming
serializable review evidence. Package-owned public data is now projected
with supply, generic shape, properties, stable field/variant identities,
retired identities, relevance, lifetime arity, and exact lifetime-sensitive
field/payload types. Numbered ordinary data is the wire contract; the retired
standalone `wire data` form is not a
second API row. Public data default-domain `where` facts now give fields and
static parameters exact local declaration identities during symbol resolution.
For this data-invariant row family, the compiler rederives the complete evidence
graph from final typed Psi, currently the earliest coherent owner of all of its
ownership records, semantic rows, references, contexts, symbol indexes, and
structural places. That is not a requirement for unrelated package evidence to
wait for final typed Psi. The compiler requires the graph to equal the retained
checked graph, projects the facts through the same canonical contract vocabulary
as public domains, and includes them in public-data identity in review
v59/canonical row v17. Missing, duplicate, altered, malformed-span, or
path-spoofed custody rejects; unsupported source fact forms remain fail-closed.
Review v60/canonical row v18 adds a closed ordinary/quotient data discriminant.
Public quotient rows bind the exact carrier-family type and package-qualified
public relation declaration only after the compiler independently reruns the
complete formation judgment. The relation's public-proposition row binds its
body and evidence interface. The selected equivalence conformance remains
private package-admission custody and is deliberately absent from quotient API
identity. Data declarations do not admit proposition parameters. Review
v61/canonical row v19 adds exact raw byte-sequence literals to public contract
expressions. It encodes the decoded octets directly, with no text
interpretation, so equivalent literal spellings share identity and any changed
byte changes the reviewed row. Unsupported aggregate and advanced call forms
still fail closed. Review v62/canonical row v20 retains fully substituted,
lifetime-sensitive declaring-trait arguments for inherited requirements of a
lifetime-generic public conformance. Alpha-renaming and private realization
bodies remain irrelevant; changing the selected lifetime ordinal changes the
row. A conformance targeting a lifetime-parameterized trait remains fail-closed
pending its declaration-site application rule. Review v43 and canonical row v3
represent static-machine parameters directly: structural contracts retain
their complete alpha-normalized nested signature and operational envelope,
while nominal contracts retain exact public trait and requirement identities.
Nested structural binders have exact checked proof/crash custody; missing rows,
excessive depth, and private nominal requirements reject. Public traits now
retain exact package identity, boundary status, alpha-normalized
lifetime/type/const binders, package-qualified parent applications with exact
lifetime-binder arguments, and ordered machine/operator requirement signatures
with exact service reach, installation-bound status,
synchronous invocations as exact non-`self` parameter ordinals or
package-qualified services, suspension, blocking, and termination. Progress
premises retain package-qualified public profile identity, receiver/non-`self`
parameter roots, and package-qualified field projections. Generic conformance
requirements retain optional alpha-normalized evidence-
binder ordinals, exact subject ordinals, public trait identity, and structural
arguments. Binder-free requirements do not fabricate evidence. Non-generic
selected conformances retain exact package-qualified conformance, carrier, and
underlying public-trait identities plus carrier/trait applications. Their
semantic declarations retain exact carrier/trait symbols. Public trait
requirements retain named and unnamed `requires` and `ensures` through the same
closed structural fact/expression and evidence vocabulary as public callables
and join every fact to its exact checked state-signature owner. Named inputs
retain ordered proposition and evidence-interface identity while treating their
source aliases as local; named outputs additionally retain their public selector
identity. Abstract trait-requirement crash
ceilings retain canonical cause-and-guard routes from exactly one checked
trait/requirement capsule without inventing realized body sites or calls.
Selected generic-conformance applications retain their exact declaration,
complete alpha-normalized lifetime/static telescope, instantiated subject, and
underlying public-trait application. Proposition/evidence application
arguments, boundary clauses, and unsupported expression forms remain
fail-closed.
Review v64/canonical row v22 also retains proof-only `zero_value<T>()` contract
expressions by exact package-qualified, alpha-normalized target type.
Proposition-local type references are symbol-resolved before projection;
renaming a binder is stable, while changing the observed type changes review
identity. Quotient targets remain forbidden by the compiler's representation-
observer fence.
Review v65/canonical row v23 retains outcome-specific `ensures` with the exact
package-qualified result-data/result-case guard, public selector when named,
checked evidence-lane position, and canonical fact. Projection must rejoin one
exact producer-side checked guarded-guarantee carrier; missing, duplicate, or
mismatched carriers reject. Group and row ordering are stable, while moving a
fact between cases or renaming its public selector changes review identity.
Review v66/canonical row v24 retains public-operator crash ceilings. Checked
lowering creates one exact operator-symbol-keyed row for every root and domain-
homed operator, including a crash-free row; review requires the complete table
to equal compiler rederivation. Cause buckets retain truth or exact structural
guard expressions with package-qualified call/member/declared-overload
identity. Route ordering and duplicates are stable, while changed causes or
guards change canonical evidence.
Review v67/canonical row v25 retains ordered array literals in public contract
expressions. Elements recursively use the same structural vocabulary and
encoding limits, so nested arrays remain exact. Reordering or changing an
element changes review identity; unsupported nested forms reject the complete
row.
Review v68/canonical row v26 retains nominal record and sum-case constructors
by exact package-qualified data, optional case, and field identities plus
recursive field values. Fields canonicalize by exact identity instead of
authored order. Changed cases/fields/values change review identity; unresolved,
mismatched, or private public-interface selections reject.
Review v69/canonical row v27 retains indexed and ranged contract expressions.
The authored `[` token must finalize through one exact checked public-interface
`Index` or `Range` selection. Rows preserve builtin or declared meaning,
collection, scalar index or optional range endpoints, and inclusivity. Changing
an index, endpoint, or inclusive end changes review identity; missing,
ambiguous, or mismatched custody rejects. Indexed children compose recursively
inside arrays.
The v69/row-v27 wire shape is unchanged while public operator contracts now
require checked declaration custody. Every non-crash `requires`/`ensures` fact
must have one exact `OperatorDeclaration` owner row keyed by its declaration;
missing, duplicate, or mismatched owner/kind/fact rows reject. No named
operator-contract syntax or evidence lane is introduced.
Trait `invariant` clauses are retired rather than awaiting a package row.
Requirements also retain whether their checked declaration
supplies a default realization; implementation bodies remain checked source
subject to universal update triage rather than compiler-private IR in package evidence. Public
domains with representable shape now retain exact package identity,
alpha-normalized generic carrier/index shape, package-qualified carrier/index
types, closed compiler-owned classifications, and authorized establishment
routes with exact package-qualified trait/requirement identities. Transparent
aliases recursively flatten to canonical package-qualified atoms; compiler
carry atoms remain explicitly toolchain-unbound. Predicate-body presence and
the representable structural expression/membership subset retain exact
domain-carrier, member, and referenced-domain identity through typed facts,
checked owner rows, and fact-keyed dependency places. Proposition applications
use their exact checked rows. A simple total, pure call retains its optional
receiver, exact checked package-qualified entry target, and ordinary arguments
after joining one public-interface declaration-selection row. The separate
whole-source commitment pins the helper body; a callable signature is not body
identity. Review v44 and canonical row v4 extend that call row with static-
machine arguments, retaining either the exact caller machine-binder ordinal or
the exact concrete machine entry identity. Review v45 and canonical row v5
rejoin each contract call to exactly one selected callee static telescope and
preserve supported argument categories as direct concrete type identities,
parser-canonical integer const literals, caller machine-binder ordinals, or
exact concrete machine entry identities. Nested static applications, forwarded
or symbolic type/const binders, proposition/evidence static arguments, quotient
calls, compiler intrinsics, and malformed or ambiguous joins remain fail-closed
until exact rows land.
Review v46 and canonical row v6 add bounded recursive generic data-type static
arguments in contract calls. Each application base rejoins exactly one checked
data declaration, whose telescope is recursively classified; changing a nested
type changes canonical evidence. This rung admits zero-lifetime generic data
applications only. Lifetime-bearing applications, generic machine/conformance
applications, unresolved forwarded type/const binders, proposition/evidence
static arguments, quotient calls, and compiler intrinsics remain fail-closed.
Review v47 and canonical row v7 admit lifetime-bearing recursive generic data
static arguments in contract calls after an exact data-declaration lifetime-
arity join. Lifetime arguments retain alpha-normalized caller lifetime-binder
ordinals: renames are stable, while selecting a different lifetime changes
canonical evidence. Generic machine/conformance applications, unresolved
forwarded type/const binders, proposition/evidence static arguments, quotient
calls, and compiler intrinsics remain fail-closed.
Review v48 and canonical row v8 admit contract-call forwarding of caller type
and const binders. Each argument is validated against the exact caller and
selected-callee telescope categories and encoded by its alpha-normalized caller
static-telescope ordinal: binder renames are stable, while selecting a different
binder changes canonical evidence. The frontend now resolves const-parameter
carrier types on machines and traits. Symbolic const declarations or
expressions, proposition/evidence static arguments, true nested
machine/conformance applications, quotient calls, and compiler intrinsics
remain fail-closed.
Review v49 and canonical row v9 admit public-trait proposition-family
parameters with their mandatory declaration-site value signature. Each retains
the ordered, package-qualified and alpha-normalized value-parameter types.
Trait, proposition, and value-parameter binder renames are stable, while
changing a signature type changes canonical evidence. Non-default
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
interface; proposition-valued contract-call static arguments remain a separate
incomplete form. Generic proposition law conformance now compares the exact
normalized proposition declaration and structural application; rendered labels
are diagnostic only, and a same-spelled foreign endpoint cannot discharge the
selected law. This compiler result still does not become standalone package
proof until it is carried by the total recheckable package evidence artifact.
Review v51 and canonical row v11 admit the four compiler-owned byte-sequence
predicate calls in public contract facts. The checked authored-selection row
now retains the exact closed predicate instead of one undifferentiated
intrinsic tag; projection cross-checks that identity against an unresolved,
receiver-free call before encoding it. Changing the predicate changes
canonical evidence, while a package declaration with the same spelling remains
an ordinary package-qualified callable. Other compiler intrinsics remain
fail-closed.
Review v77 and canonical row v35 admit exact compiler-installed builtin-function
calls in public contract facts. Projection requires the retained checked call
selection to identify the same exact fixed root slot and symbol kind, then
encodes the stable closed builtin ordinal. The call spelling is diagnostic only;
late root, nested, package-authored, and generated same-spelled symbols do not
classify. Static arguments and target-symbol custody disagreement reject.
Review v78 and canonical row v36 retain that same closed builtin identity for
builtin-backed boundary-operator provider execution. The compiler-owned sidecar
is row-aligned and separate from the authored realization machine; projection
rederives and cross-checks it from checked overload identity. Missing,
mismatched, spoofed, and still-unclosed primitive-expression children reject.
Review v79 and canonical row v37 extend the sidecar with exact named-float
negation atoms for `f32` and `f64`. Compiler dispatch selects the atom from the
checked overload and external realization join rather than from the authored
machine name; the package-qualified realization nominal remains independently
reviewed. Projection rederives the atom and rejects missing, cross-format,
non-intrinsic, or spoofed state.
Review v80 and canonical row v38 add named-float conversion atoms. Each commits
the exact checked numeric source type, numeric target type, and arithmetic
domain together, so width, signedness, and integer overflow policy cannot drift
independently. Projection rederives every coordinate from the exact checked
overload and rejects missing, mismatched, or spoofed state.
Authored unary `!` and `~` operators retain their exact operator-token spans,
including when nested in a public proposition or contract. Checked lowering
must finalize that exact occurrence as the closed builtin-operator meaning,
and review rejoins it before projecting the structural unary discriminant.
That custody-only change did not alter the then-current v76/canonical row v34
bytes; it prevents a canonical unary expression from bypassing its authored
public-interface custody.
Review v52 and canonical row v12 add blocking standalone public-proposition
rows. The compiler retains `pub` through checked proposition vocabulary,
rejects public-interface selection of a private proposition, and records every
package-owned public declaration even when unused. Primitive publication is a
name/signature API row, never a fact or admission; witness interfaces and
transparent expansions are structural compatibility content. A published
transparent alias remains source API even though normalized proposition
applications expand through it.
Review v53 and canonical row v13 add one blocking `PublicConst` row for every
package-owned public const, whether used or unused. The row contains the exact
package-qualified declaration identity, exact typed declared-type identity,
and canonical structural value encoding. It never substitutes source
initializer text, display text, or runtime storage identity. Public consts
whose declared type exposes private data, or whose value cannot yet be given
that exact semantic identity, reject rather than weakening the row. A type or
value change consequently enters source-backed conflict review as
`public_const`; private const-v0 declarations remain outside this API surface.
Ordinary `pub operator` visibility now survives checked compilation and owns
its visibility independently of any carrier-qualified path. Exact authored
source provenance keeps package ownership recoverable, and late proof-static
operator selections finalize only from exact typed operands before the
visibility gate is repeated. Cross-package private selection rejects while an
owner's private implementation use remains legal. Review v54 and canonical row
v14 add the blocking standalone `PublicOperator` row. Its key combines exact
package-qualified declaration identity with the compiler's canonical operand
and result-dispatch identities; its value retains boundary status, fixed
spelling, complete signature shape, and contracts projected directly from the
declaration even when unused. Public contract binaries now encode either one
exact declared overload coordinate or explicit builtin meaning. Unresolved
proof-static selections reject closed.
Complete name-first conformances now retain their own ordinary `pub` bit
through syntax, source profiling, resolved/typed/checked trees, and all stage
snapshots. Exact authored-selection gates reject private cross-package
selection and public-interface citation, and a public conformance cannot hide a
private carrier or trait. Private realization machines remain implementation;
their visibility is not promoted. Lexical conformance-binder requirements
inherit the enclosing declaration's visibility and do not become package
declarations. Explicit conformance-row machine references retain their authored
span through exact row normalization and obey ordinary package visibility.
Review v55 and canonical row v15 add a blocking `PublicConformance` lane keyed
only by exact package-qualified conformance identity. Its value retains the
alpha-normalized lifetime/static telescope, subject, exact trait application,
and complete normalized inherited requirement interface. Requirement
overloads use the compiler's canonical callable identities. Closed and
attached-machine realization forms encode identically; realization names,
bodies, and physical code do not enter public compatibility. Every realization
signature and substituted trait law is checked before projection, while the
referenced `PublicTrait` row owns the law text. Canonical recovery and
fixed-vocabulary conflict rendering recognize `public_conformance`. Unsupported
lifetime-parameterized target traits, inherited lifetime substitutions, and
proof-static trait parameters reject review rather than yielding partial rows.
Review v56 and canonical row v16 additionally admit a trait's
requirement-identity machine parameter as a closed, payload-free contract kind.
It is distinct from structural callable contracts and nominal trait-requirement
pairs, and therefore cannot be reinterpreted as either during compatibility
review.
Public domain semantic roles now project from the exact typed declaration as a
closed compiler-owned tag set. Every retained contribution must point to that
declaration's own semantic identity; canonical review records the package-
qualified domain and role, not a compiler-private semantic ID or a role guessed
from source spelling. The separately encoded exact `PublicOperator` rows remain
the operator contract surface.
In particular, true nested machine static applications such as
`consumer<family<Selected>>()` now reject during compiler validation, before
checked lowering. Treating the argument as the uninstantiated `family`
declaration checked the wrong callable shape; monomorphization also has no
closed recursive application identity in conflict equality, specialization
keys/fingerprints, or retained specialization evidence. Supporting this form
requires recursive specialization plus exact declaration-telescope, lifetime,
and static-argument identity throughout those paths. This is distinct from
already coherent bare generic-machine selection and call-target use such as
`Schema<Selected>(...)`.
Reviewed boundary/public
machines and the selected build machine retain exact canonical entry
signatures and checked-body/boundary/accepted supply tiers. Bodyless boundary
guarantees remain explicit trust-bearing accepted claims; claim-free boundary
symbols do not become claims. Each accepted callable additionally emits a
separate blocking canonical row with the complete callable envelope and exact
declaration source. Initial admission or a newly introduced package must
resolve that trust row; an unchanged accepted baseline does not recur as a
blanket prompt. Signatures retain lifetime arity,
alpha-normalized type/const/static-machine parameters, ordered
parameter names/modes, package-qualified lifetime-sensitive parameter types,
and result type. Checked realizations of public, ordinary, lifetime-free traits
retain exact package-qualified trait/requirement identities, alpha-normalized
arguments, and aliases. Callable conformance bounds, static-machine/proposition
arguments in selected conformance applications and non-public or lifetime-
parameterized trait realizations fail closed until complete rows land, except
that binder-free generic requirements, explicit evidence binders, and non-
generic selected conformances use the canonical public-trait row. Review
v76/canonical row v34 additionally retains an exact public ordinary,
nongeneric, lifetime-free operator coordinate plus optional conformance alias
for each checked-body realization,
whether or not the declaration owns a fixed token,
after checked lowering retains and projection exactly rederives the selected
machine/operator symbols, conformance/admission form, normalized overload
shape plus exact lifetime-bearing type nodes, both full canonical contract sets, and exact typed semantic snapshots of
their contract graphs. Projection then reruns exact selection and the equality/
`&&` `requires`/`ensures` operator-contract judgment. Operators with outcome-
specific or crash contracts, and providers with any nonempty checked crash
behavior, reject until their refinement rules land. The
association is a retained compiler-private checked baseline, not a hash,
persisted package format, or defense against a trusted component rewriting both
typed state and checked facts. Private, generic/lifetime-parameterized, and
bodyless checked realizations still fail closed. Operator-bound external
supply uses the tagged v72/canonical-row-v30 trust association below. A fixed-
token checked realization uses the same declaration coordinate as its named
call surface; the public-operator row owns the closed compiler spelling, so the
callable edge does not repeat it or create another identity form. Checked-body boundary
operators use the same satisfaction edge; the selected-provider set separately
identifies the active plan and rejoins its exact requirement and realizing
machine. Projection repeats the exact symbol, slot, checked-adapter binding,
package, and machine join. A named-boundary canary covers unique selection.
Fixed-token boundary operators remain fail-closed until checked-adapter token
dispatch exists. Authored same-path overload-family override remains OWNER Q10.
Public callable `requires`
and `ensures` retain exact structural rows for the closed
boolean/integer expression subset over parameter ordinals, `result`, generic
binders, and package-qualified nominals. Domain-membership rows retain the exact
value and package-qualified public domain; private package domains reject.
Proposition rows retain an exact package-qualified primitive endpoint,
alpha-normalized declaration binders and parameter types, structural
binder/value arguments, and fact-only or witness classification. Transparent
aliases expand without identity. Witness interfaces retain exact root arguments
and complete direct/inherited requirement surfaces. Named contracts join their
checked evidence term and positional lane. A `requires` binding spelling is a
local alias and is excluded from canonical identity; an `ensures` selector is
public and remains. Checked string renderings are diagnostic only and changing
them does not change review bytes. A proof-static `evidence.member` binder
argument retains its named-`requires` lane, package-qualified declaring trait,
structural requirement-argument template, and exact requirement. The source
lane binds the template to the proposition application's concrete arguments;
the local evidence alias is not identity. Matching checked evidence-term,
interface, and projection facts are mandatory. Direct parameter-rooted member
paths retain the receiver ordinal and exact package-qualified case/field chain
only when a unique checked semantic-place row and exactly one finalized public-
interface member-token selection agree on the field. Missing, duplicate, or
mismatched token custody rejects without changing canonical row bytes. Computed members,
proposition-argument members without that join, unsupported advanced call
forms, and aggregate expressions still fail closed. Contract casts retain the
structural operand, alpha-normalized target, arithmetic policy, package-qualified
semantic domain and arguments, and value/recast form. Diagnostic spellings are excluded;
private package domains reject when exposed by a public cast. The semantic-domain
path retains one exact authored-selection occurrence on the cast expression;
typed domain resolution must finalize that same occurrence before checked
visibility, direct-dependency admission, or review projection can accept it.
The expression also retains its exact authored public/private position, which
governs nominal selections in cast targets, cast domain indices, and
`zero_value<T>()`; proposition and machine casts resolve those targets through
the same symbol path. A public contract cannot smuggle a private or transitive-
only type through one of these expression-owned positions.
This join does not create a nominal Chi stage.
Ordinary standalone checked compilation still takes a caller-owned writable
build root when build-host staging is possible. Package review instead supplies
a package-specific root inside its orchestration-owned disposable child
session. Resolver snapshots remain immutable and are never repurposed as output
directories.
The legacy machine-contract fingerprint no longer enters package-review bytes,
so private state shape is not public contract identity. Complete proof and
unsupported-clause rows still gate sealed admission. The compiler now provides
a version-52 length-framed binary comparison encoding over this review
projection; it is explicitly not a package certificate or accepted-lock
payload. Raw Rust/debug serialization is not an alternative. These pieces do
not become an admission path until an accepted typed lock is implemented and
sealed, locally regenerated compiler evidence plus the hardened resolver
receipt are wired through end to end. The earlier public
`PackageInstance` constructor was removed: the real type must not exist as a
caller-constructible tuple of arbitrary toolchain and evidence fingerprints.

Terminal replay now has one complete verifier-owned obligation set covering
operation sites, call requirements, nominal cleanup requirements, and contract
guarantees. Rows retain exact semantic owners, assumptions, reconstructed
axioms, and obligation class. The verifier consumes and retains this exact set.
Its canonical ledger encoding binds the Terminal-Psi subject and source-backed
verifier trust graph but deliberately excludes the selected proof route; a
consumer must reconstruct and compare the ledger locally after decoding it.
Terminal artifact manifests retain its fingerprint independently from semantic
and proof identity, and the replay lowering path accepts semantic, ledger, and
proof bytes only after exact local ledger comparison. This is not a package-
evidence or lock-promotion API. Ordinary package
capability/API obligations, source-to-artifact binding, transitive open rows,
and schema-delta composition remain to be built before `PackageInstance` can
exist honestly.

`ReviewOnlyBaselineCapsule` now has explicit capability-rooted file custody for
restart state. Trusted command orchestration supplies an already-open
project-owned directory and one bounded lowercase portable direct-child name.
Recovery never follows a leaf symlink, remains byte-bounded through canonical
decode, and then rereads the retained handle and rechecks its live pathname.
Publication uses a synchronized private same-directory stage, Unix mode `0600`,
and atomic no-overwrite installation. This cannot write `omega.lock`, authorize
a conflict, or promote the review-only capsule into package evidence.

For ordinary package claims, “produced artifact” means the complete canonical
package-admission semantic row set under an exact package, target, dependency
closure, and obligation schema. It is not native code and it is not the
compiler-issued review object. Review may carry candidate bytes in the same
vocabulary, but a consumer must regenerate the total row set from exact source
and compare it exactly before those bytes can participate in accepted evidence.
Source, certificates, proof routes, compiler observations, and local decisions
remain separately bound. The current incomplete review projection therefore does
not become a package artifact or `PackageInstance` by renaming it.

The current ordinary obligation ledger binds the exact path-free dependency
closure consumed by the compiler alongside package, target, and canonical rows.
That closure contains every reachable package identity and requester-local alias
edge, but excludes copied package display-name strings, source roots, immutable
resolutions, and source bytes. Each opaque package identity still binds its
declared name and source lineage. Recovered row envelopes must be joined to a
locally reconstructed closure; renaming an unused alias or adding/removing an
unused reachable dependency invalidates equality, while relocating the same
graph does not. The ledger now names its obligation-semantics schema separately
from its outer codec and review-row vocabulary. Its bounded canonical whole-
ledger frame carries the exact package, target, closure, aliases, and row bytes;
strict decode revalidates canonical graph closure and row framing, and a domain-
separated fingerprint identifies the complete framed replay question. Row
payload meaning remains opaque until exact local reconstruction. Compiler-
issued closure review retains that locally reconstructed ledger under one
overflow-safe 64 MiB aggregate retained-ledger ceiling per review session.
Decode and fingerprint remain inert until exact local reconstruction succeeds.
This closes the schema-bound replay-question coordinate, not transitive
certificate/open-obligation composition or lock admission.

The resolved source closure now retains the exact validated root request beside
its normalized lineage and immutable resolution. Its read-only request-set view
joins that root and every dependency request occurrence to the exact selected
package key and resolution without copying dependency locator strings. The
dependency join uses requester plus authored ordinal, preserving multiple
different requests that resolve to one package in a diamond; no primary request
is inferred. Repository-root Git closure resolution uses the same custody and
retains the requested locator and `HEAD` selector separately from resolved
commit/tree/content and transport provenance. A mismatched root request rejects.
This is bounded resolver state, not compiler evidence, obligation-ledger data,
accepted-lock encoding, admission, or a `PackageInstance` constructor.

`CanonicalSourceClosureSubject` is the first bounded persistence-neutral form
of that exact source-selection question. It stores the root request and every
requester/ordinal dependency occurrence beside its resolved alias and exact
selected package key, immutable resolution, and content identity. Strict decode
reconstructs the closed graph and canonical ordering, but neither decode nor
its domain-separated fingerprint grants authority. A consumer must resolve and
snapshot the requested closure again, reconstruct the complete subject, and
require exact equality. Cache/snapshot paths, source bytes, transport execution
observations, compiler source-consumption and build observations, artifacts,
certificates, decisions, and open obligations remain separate. The type has no
accepted-lock or `PackageInstance` promotion API.

`CanonicalPackageReconstructionQuestion` joins those two replay coordinates
without promoting either. It retains the complete source-subject frame and one
complete canonical ordinary-ledger frame per package in strict full-
`PackageKey` source order. Every ledger must match its package identity, the
aggregate target, and that package's independently derived transitive package
and requester-local alias closure. Missing, foreign, swapped, identity-
colliding, mixed-target, or graph-drifted associations reject. Strict recovery
is bounded and canonical; fresh matching rebuilds the whole question from
current resolver custody and a newly compiler-issued review set. Its
fingerprint identifies only the question. The type contains no admission,
certificate, result, artifact, accepted-lock, or `PackageInstance` path.

## Target command surface

```text
omega install <source> [--rev <revision>] [--as <alias>]
omega update [package-or-alias...] [--to <revision>]
omega audit packages
```

Install fetches the source before learning its package name. Update builds from
the accepted lock and never silently re-resolves mutable source selectors.
Conflict resolution is row-specific and bound to the exact candidate; there is
no blanket approval switch.

Ratified 2026-08-26: the compiler owns both the semantic extraction and the
canonical conflict-row boundaries. It may read different rows from different
compiler-owned representations, including private pre-Psi structural state,
and move those joins as compiler internals evolve. Checked acceptance and
effects still come from the stage that establishes them, and projection occurs
only after successful checking. Package orchestration receives only independently
framed, versioned bytes and compares them exactly; it does not parse compiler IR
or duplicate capability semantics.
This does not create a nominal Chi stage. A new stage is warranted only if
implementation discovers a genuine shared semantic invariant, not merely to
stabilize a private checker interface. Discovery may instead consolidate facts
in an existing coherent representation such as `Exact` when that removes
machinery without erasing meaning. The initial callable row is one complete
envelope, and the selected-provider set deliberately remains one opaque,
blocking row even with sealed provider identity; finer explanation does not
change that ownership boundary.

The concrete carried-type slice follows that rule. Checked flow joins machine
heads, exact checked call targets, ownership places, and compiler-selected
cleanup into one package-neutral sidecar after checking succeeds. The sidecar
uses exact declaration handles internally and promotes any public-interface
occurrence; those handles are not lock data. The package-review projector now
maps each reviewed-package consumer and dependency to package-qualified nominal
identity, emits nominal/layout/ownership/automatic-cleanup rows with
private/public disposition, and anchors each row to both declarations. These
rows are versioned comparison evidence, not an accepted-lock payload. A total
coverage audit and accepted admission/lock issuance remain open.

Authored conformance authority follows the same compiler-owned join. Explicit
static conformance arguments are retained at resolution; checked trait
operators retain the conformance selected for the operator token; and generic
specialization retains the exact unique conformance inferred for an unbound
`satisfies` requirement. The latter is package-qualified in specialization
identity and attached to the authored call token. Explicit and inferred
selections remain separate rows, and each selected declaration's owner must be
self or a direct dependency before package code is admitted.

Source-authored Unit/discarding statement calls use the same ledger. Resolution
retains exact targets and explicit static conformances before statement-table
rebuilding; checked flow finalizes late targets and inferred conformances.
Compiler-owned build markers and lowered assembly operations have closed
intrinsic identities, while every ordinary statement target remains subject to
the direct-dependency gate.

Every source-backed static argument path on an expression or statement call is
also an authored declaration selection. Explicit conformance paths retain the
dedicated conformance kind; other type, static-machine, and forwarded-binder
paths share one static-argument kind and retain the exact selected symbol.
Nested declaration applications recurse. Integer literals select no
declaration, and named const reduction separately preserves the exact const
declaration that supplied the value. Any unresolved static path remains visible
and fails package admission.

Declaration-owned expressions retain the declaration's exposure. Public
machine contracts and ranking expressions, public data/domain predicates, and
public trait contracts enter public compatibility custody; executable states
and bodies remain private implementation. Proof-membership facts retain the
exact selected domain path instead of treating only their value operand as an
expression. Lexical parameters and locals are places, not declaration rows.
Compiler-recognized byte-sequence predicate calls are closed intrinsics; a
resolved declaration with the same spelling wins and retains its package owner.
Visibility inheritance for nested declaration families remains governed by the
owner visibility decision.

Generic conformance bounds retain only their authored declaration selections:
the subject and evidence binder are lexical, an ordinary right-hand trait is a
trait selection, and `Subject satisfies Carrier::Evidence` selects both the
carrier and the named conformance. Machine and trait bounds inherit their
enclosing declaration's exposure. This direct-authority custody is independent
of the still-open rule for publishing declaration families.

Trait composition follows that same authority rule. Header parents and body
`requires` clauses normalize to one edge, but each authored edge retains its
exact resolved trait as a type-reference selection with the enclosing trait's
public/private exposure. A parent available only through a transitive package
therefore rejects; the existing `trait_parent` source location is explanatory
review provenance, not package admission.

An attached declaration head such as `machine Data::operation` also selects the
exact `Data` declaration. That carrier coordinate is retained as a type-
reference row under the machine's interface exposure, including exported
boundary supply even when the boundary declaration is not spelled `pub`.
Qualification does not relabel or implicitly admit a transitive carrier.

Quotient formation retains each authored declaration coordinate separately:
the carrier type, right-hand relation path, repeated static-`where` relation
subject, sealed `Equivalence` trait and its arguments, and named proof
conformance. Relation and trait coordinates inherit the quotient data
declaration's exposure. The selected conformance remains private formation
custody and stays outside quotient API identity, though ordinary package
visibility and direct-dependency admission still govern selecting it.

An exact `machine ... satisfies Namespace::requirement` edge retains the
declarations selected at both parts of the path. Trait edges retain the exact
trait and result-dispatch-selected requirement; operator edges retain the exact
signature-selected overload. These rows follow the realizing machine's actual
interface exposure, including boundary and accepted supply. The compiler
settles them from the complete typed declaration graph, then validation,
progress, checked operator facts, provider planning, and review cross-check the
same symbol. Supply-mode policy is separate: external supply cannot substitute
a boundary declaration for the ordinary operator the source actually named.

Domain `established by Trait::requirement` paths use the same two exact row
kinds at the earlier signature-free normalization point that already proves
uniqueness and subject authorization. Their exposure comes from the domain,
not the selected trait. Normalized establishment alternatives may deduplicate,
but authored source occurrences do not disappear from direct-authority
custody.

Nominal callable machine-parameter contracts retain their complete authored
`Trait::requirement` path after signature-free resolution. Typed lowering emits
the exact trait-path and requirement-token rows with the enclosing
declaration's exposure, recursively for nested machine contracts. Generic
nesting therefore cannot hide a transitive-only or private requirement from
package admission.

Compiler issuance now retains a separately bounded canonical row sequence.
Before fresh closure review publishes those rows, it strips explanatory source
coordinates into separate provenance, forms a source-handle-free
`OrdinaryPackageObligationLedger`, reconstructs the complete current row set a
second time from checked compiler semantics, and requires exact equality.
The ledger also binds the path-free dependency closure projected from validated
compiler inputs: every reachable package identity and requester-local alias
edge, but no source root, resolution, separately copied package display name, or
source byte. Each opaque package identity still binds its declared name and
source lineage. Individually recovered row envelopes establish framing only and
must be joined to that separately reconstructed closure; missing, reordered, stale,
mixed-package, mixed-target, renamed-alias, or changed-closure ledgers reject
under local comparison, while relocation alone is irrelevant. This is a replay
gate for the current review vocabulary. It also retains an explicit current
obligation-semantics identity and a bounded canonical whole-ledger frame over
the package, target, complete path-free closure, aliases, and row bytes. Decode
rechecks schema and row-vocabulary versions, closure reachability/cycles/order,
row framing, resource ceilings, and canonical re-encoding; row payload semantics
remain subject to exact local reconstruction. The ledger fingerprint is only
identity of that framed question, and retained ledgers share one 64 MiB session
ceiling. This is not accepted package evidence or a lock-promotion path:
produced-artifact completion, certificate results, transitive open obligations,
checked schema deltas, transitive dependency-evidence composition, and root
admissions remain absent.
The package layer now also retains one canonical reconstruction question over
the complete resolved source subject and every per-package ledger, rechecking
their exact full-key order, common target, and independently derived transitive
closures. This aggregate remains a replay question only and has no promotion
route.
Review-only update comparison joins candidate rows to exact resolver custody,
matches rows linearly by compiler-owned `(kind, key)` coordinates, and retains
complete old/new bytes without decoding them. Conflict fingerprints bind both
source revisions, compiler/source-consumption evidence, the displayed shortest
dependency path, and a canonical commitment to the entire candidate closure.
The ordinary model-facing view remains compact and fixed-vocabulary: it shows
row coordinates, lengths, and commitments while the separately framed source
patch provides readable code. Representation-TCB-only changes recommend audit;
blocking and opaque-blocking changes still reject. Compiler-issued rows now
carry explanatory package-relative UTF-8 paths and exact byte spans separately
from semantic bytes. Ordinary declaration rows point to their declaration;
dangerous-authority rows point to both the canonical toolchain declaration and
the package callables exposing it. Generated symbols follow authored derivation
provenance. Changed-row fingerprints bind the exact old/new coordinates shown
by the escaped fixed-vocabulary renderer, but source movement alone is not a
capability change. Ordinary semantic rows carry their exact declaration symbol
through canonical sorting; dangerous-authority rows retain their exact service
declaration and exact exposing callables during derivation. Source issuance no
longer rescans typed trees by reduced nominal identity. Provider candidates now
carry compiler-internal provenance
beside their semantic plans: exact schema and optional nominal-provider symbols,
plus the exact requirement and realizing machine for every external or
checked-adapter row.
Selection and sorting preserve the pair and add exact authored build/target-
default call sites or a closed reason for an implicit unique choice. The single
selected-provider row may therefore contain both authored coordinates and
compiler-derived reasons; free external providers and empty sets also have
closed reasons. Exact `invokes`, `reaches`, `suspends`, and `blocks` occurrences
now survive their owning frontend stages and appear under distinct source
roles on callable, public-trait requirement, and recursively structural
machine-parameter rows. The compiler joins those occurrences to the exact
resolved target or operational interface and rejects missing, stale, malformed,
or contradictory custody. It does not parse source text in package
orchestration or invent locations for inference and closure. Public operational
facts remain may-ceilings; the review does not mislabel a permissive public
contract as an observation that its current body exercised the permission.
Public const initializer, transparent public-proposition formula, and authored
proof-fact coordinates also survive their owning frontend stages; none is
reconstructed from source text after value substitution or expression lowering.
Other nested clause/use-site coordinates remain unfinished engineering work; none
independently motivates nominal Chi. Checked invocation facts retain exact
symbolic published and inferred targets before provider settlement, and package
review consumes those facts rather than re-inferring from transformed typed
calls. Package review v79/row v37,
canonical recovery v13, conflict fingerprint v16, and conflict renderer V15 bind
the current source-role vocabulary. The package layer
does now validate a complete in-memory
root-policy disposition for every exact blocking fingerprint. It canonicalizes
and candidate-binds the decision set, rejects non-blocking or stale decisions,
and reports only whether root policy permits all blocking rows. It does not
prove review or authorize admission. The complete result also has a bounded,
fixed-vocabulary canonical text record containing only candidate, fingerprint,
closed disposition, and resolution-commitment fields. Recovery strictly
validates framing and resource ceilings, maps every fingerprint back to the
current compiler-derived conflict and owning package, reruns complete
resolution, and requires byte-identical canonical re-encoding. The record alone
is restart-stable policy state, not policy-origin custody, governance evidence,
accepted package evidence, or transaction authorization.
Candidate-closure commitment v2 binds source topology and immutable resolution
plus every candidate package's target, compiler executable, source consumption,
optional build observation, and whole-review commitment, even when that package
has no blocking row.
The record now also has an explicit policy-directory file-custody layer. Trusted
command orchestration supplies an already-open root-owned directory capability
and one bounded lowercase portable canonical filename; nested paths are not
representable and dependency source is never searched for policy. Every
operation is a direct-child operation relative to that handle. Symlink or
non-regular leaves, case aliases, and existing destinations reject. Reads retain
the opened file through semantic recovery, reread and compare its bytes, and
then recheck its live filename. New files use a private same-directory stage:
write, file synchronization, reread, and identity verification all precede
atomic no-overwrite hard-link publication. Directory synchronization follows.
A failure after publication reports `published but unconfirmed`, because the
complete canonical file may remain recoverable. Command integration must supply
the actual root-owned directory. This custody neither proves an audit nor
authorizes the wider transaction, and it deliberately leaves the eventual
command directory and filename open.
The reread and identity checks detect ordinary concurrent change, not a hostile
process already holding the root author's filesystem credentials and deliberately
alternating valid states between observations. Final command transaction locking
and immediate revalidation own that boundary.

The former commands accepting `manifest.json`, `receipt.json`, `--package`, or
mandatory `--alias` were removed from the production CLI. Their name-keyed
manifest, lock, review, install, update, audit, diff, and command modules have
now also been deleted rather than retained as a parallel test-only model.
Direct `omega install` and `omega update` invocations, along with the old
review/plan/lock command names, fail at dispatch before compiler parsing,
resolution, or project writes. Ordinary source files named `install.omg` or
`update.omg` remain compiler inputs.

## Responsibilities

- Normalize transport-independent source lineage where equivalence can be
  established safely.
- Resolve source requests to immutable commit/tree/content identity in an
  isolated cache.
- Extract package declaration and dependency-source projection without
  build-host authority.
- Reconcile one immutable instance per `PackageKey` in the initial model.
- Invoke compiler package-admission mode and locally regenerate evidence bound
  to source, evidence schema, and compiler/toolchain provenance. This excludes
  dependency-authored manifests; it does not certify the selected compiler.
- Persist the complete accepted baseline and exact closure in `omega.lock`.
- Render compact capability conflicts and hostile-input-safe LLM triage packets.
- Leave audit quality, reviewer/quorum requirements, and merge authorization to
  root-project policy; no receipt or status is presented as proof of audit.
- Perform conservative `build.omg` edits only after admission.

## Non-responsibilities

- Hosting packages or providing a registry namespace.
- Trusting a package name, URL spelling, repository name, or human prose as
  identity/evidence.
- Solving semantic-version ranges in the first implementation.
- Defining language semantics for reach, authority, proofs, providers, or build
  observations.
- Giving downloaded code resolver, root-package, or acceptance authority.

## Source structure

The parent [`packages/README.md`](../README.md) is the subsystem entrance and
explains the three crate boundaries. Inside this workflow crate, folders name
responsibilities; the crate root only explains and reexports them.

```text
omega-packages/
|-- README.md
|-- src/
|   |-- lib.rs                    # Responsibility map and compatibility exports.
|   |-- declarations/             # Read and conservatively edit build.omg.
|   |-- resolution/
|   |   |-- identity.rs           # Package/source lineage and immutable identity.
|   |   |-- graph.rs              # Typed pre-admission reconciliation.
|   |   |-- closure_resolution.rs # Bounded recursive source custody.
|   |   |-- package_source.rs     # Snapshot-to-declared-PackageKey custody.
|   |   |-- source_adapter.rs     # Explicit workspace and Git closure policy.
|   |   |-- source_closure_subject.rs # Canonical complete-closure subject.
|   |   |-- source_commands.rs    # Read-only diagnostic command surface.
|   |   `-- source/               # Local/Git acquisition and host custody.
|   |-- review/                   # Compiler review, comparison, triage, and policy.
|   |-- storage/record_file.rs    # Internal bounded rooted persistence.
|   `-- bin/omega-source-snapshot.rs
`-- tests/
    |-- capability_conflicts/    # Transaction, public-API, and operational deltas.
    `-- responsibility-specific integration fixtures
```

Future accepted evidence, lock, audit, install, and update owners belong under
`review/` or a later transaction folder once their boundaries are implemented.
They should not return as speculative flat files in the crate root.

Machine persistence format is an internal encoding choice. Human review and
conflict surfaces use concise canonical text and do not expose package-authored
prose to the triage model.

The checked package-review path also fails closed on contract-entailment
stand-downs. It audits the pristine typed graph (including generic templates),
retains compiler-owned machine/contract/fact coordinates and a closed reason,
and refuses review when any checked-implementation claim was left unjudged. Accepted or
opaque supply remains trust-bearing. These rows are currently in-memory review
state, not sealed lock evidence.

External executable supply is a separate blocking trust row. Review v70/row
v28 binds every package-owned external leaf, including private implementation
leaves, to an exact package-qualified callable/conformance application and a
closed compiler-owned import, syscall, intrinsic, vtable, or table-function
identity. External leaves must be bodyless with exactly one conformance;
inconsistent or malformed supply-mode, conformance-binding, mechanism,
binding-table, payload, or attached-table state rejects. The callable and
complete conformance application form the stable key, while the structural
binding is value, so a binding-only update renders exactly one
`external_executable_supply` conflict with `opaque_blocking` risk and leaves
callable API bytes unchanged. Private leaves do not become public callable
rows. The compiler projects this only after successful checking, joining facts
from their earliest coherent private representations. Only the canonical row
crosses this crate boundary; it makes no Terminal or audit claim and introduces
no nominal Chi stage or public package IR.

Review v72/row v30 keeps the same row kind and risk while tagging the exact
requirement as either a trait conformance or an operator overload coordinate.
The first operator lane accepts a bodyless external leaf for a public, named,
nongeneric boundary operator. Public leaves retain that coordinate in callable
API; private leaves do not become public. The selected-provider row remains the
only selection claim and must exactly rejoin the operator, realization, package,
normalized machine identity, and binding. Compiler-known intrinsics are the
first executable mechanism. Ordinary or private operators, aliases, generic/
lifetime applications, and fixed-token operators remain fail closed.

External binding selection now has exact source custody. The parser retains
the authored `via` keyword on the same conformance that owns the normalized
binding identity, and resolution plus typed lowering preserve it without a
name-based rejoin. Every public or private trait- or operator-based external
supply row carries one `external_binding` location in addition to its machine
declaration. Binding/span disagreement, missing custody, and invalid source
coordinates reject. These locations remain outside semantic review bytes, so
review v74/row v32 are unchanged; recovery envelope v8, conflict fingerprint
v11, and renderer V10 bind the explanatory vocabulary.

## Fixtures

The local fixture corpus is under `tests/fixtures/packages/`; exact remote Git pins
are recorded in `tests/fixtures/packages/REMOTE_PINS.md`. Every fixture declares its
name through `builder.package`. The package-evidence integration canary resolves real immutable
source custody, performs the package-aware compile, and checks the compiler's
canonical review projection. It deliberately stops before sealed admission and
lock mutation; tests that fabricate manifests from fixture intent have been
removed from integration coverage. The `process-exit` fixture exercises exact
toolchain `Console` provenance, compiler-owned process classification, and the
audit recommendation retained on both initial admission and unchanged update.
A separate production-path canary resolves two byte-identical packages with
the same declared name and provider symbols from distinct lineages. Local
snapshot custody keeps separate physical compiler roots, and selected-provider
evidence remains bound to the explicitly imported package identity.
A fixture-derived `provider-switchboard` update also changes the selected type
under separate immutable baseline/candidate custody. The compiler-owned row is
reported as one opaque-blocking conflict with exact authored provenance, and
update triage blocks it.
A real-Git reconciliation canary resolves two commits of one declared package
into separate immutable snapshots, assigns the same canonical package key, and
proves closure reconciliation rejects while retaining both exact requester
rows. A second exact-revision Git canary upgrades `process-exit` from an inert
`Console` parameter to effective process termination. Compiler comparison
reports one changed blocking callable and one added blocking process-authority
row, triage retains the independent process-audit recommendation, and source
review receives the exact ordinary update patch. Remote CathedralOS fixture
verification remains credential-gated and fail-closed. The ordinary pin test
still proves every SSH/HTTPS pair normalizes to one lineage. With credentials,
each standalone private mirror is resolved into full custody and compiled
through package-aware review; the resulting package key, projection identity,
immutable resolution, and source-consumption commitment must remain bound to
that lineage. The local `generated-table` fixture also round-trips its verified
v26/v8 filesystem replay record through canonical review-baseline recovery.
