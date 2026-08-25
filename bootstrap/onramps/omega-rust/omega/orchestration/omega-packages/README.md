# Omega Packages

This is the Rust on-ramp home for package source resolution, graph
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
  evaluated `PACKAGE` constant in its `build.omg`.
- `PackageName` is presentation. `PackageKey` joins the name to canonical
  source lineage. `PackageInstance` adds exact source, toolchain, and checked
  package-evidence identity.
- `build.omg` records source requests and update selectors. The dependency's
  own declaration determines its default import alias; `--as` is an
  exceptional local rename.
- Dependency-source projection is effect-free and completes before downloaded
  build code receives any host provider.
- The compiler, not the package or CLI caller, derives package capability/API
  evidence from checked source and build results.
- Ordinary admission uses a total internal projection from checked semantic
  state into versioned canonical evidence. Raw compiler IR is never a lock
  format. The projection may read each fact from the earliest coherent
  compiler-owned representation in which it is semantically settled, including
  private pre-Psi structure, and joins required checked facts only after
  successful compilation. Totality belongs to the final projection, not one
  frozen source stage. No nominal Chi stage is introduced merely to collect or
  stabilize compiler internals.
- Checked flow now owns the first package-neutral carried-semantic-dependency
  sidecar. It retains exact nominal, layout, ownership, and automatic-cleanup
  declaration symbols with private/public disposition. The compiler review
  projection qualifies those symbols by exact package ownership and emits
  blocking canonical semantic-dependency rows; checked-tree handles never
  escape into review artifacts or lock-shaped data.
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
resolutions, target and comparison commitments, and every canonical comparison
row with its exact source sidecar. Compiler-owned decode derives row kind, risk,
key, package, and target from the canonical row frame and rejects malformed or
noncanonical source coordinates; package code leaves row payloads opaque.
Recovered rows remain a distinct review-only type and cannot become newly
compiler-issued evidence. The outer decoder checks its corruption checksum
(not authenticity or proof of review), canonical re-encoding, strict
package/row order, singleton header/provider rows, graph closure and depth, and
independent resource ceilings. Recovered baselines produce the same conflicts,
triage, and source packets as live baselines, including standalone candidate
packets when old source is unavailable. This capsule is not `omega.lock`: it
cannot issue a package instance, resolve a conflict, or mutate a project.
Its aggregate bounded renderer frames compiler-only triage separately from
hostile source lanes. It does not yet invoke a model, and no advisory answer
can mint admission or prove an audit occurred.

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

The following superseded assumptions remain quarantined to isolated crate tests
and must not return to the release path:

- locks keyed by package-authored name alone;
- mandatory caller-supplied alias and package name;
- caller-constructed `PackageCapabilityManifest` values;
- standalone JSON manifests accepted as compiler evidence;
- a manifest fingerprint without the normalized accepted baseline;
- free-form reviewer/reason receipts that accept whole sections; and
- treating the legacy standalone local-Path compatibility scanner as package
  dependency authority.

The production source-custody and typed graph paths have received focused
review; the hardened native resolver boundary, sealed admission projection, and
accepted-lock path remain incomplete.

The crate now contains reviewed building blocks for immutable Git/local
snapshots, hermetic package-name extraction, and typed package/source identity.
Mutable local-package snapshots omit only `.git` metadata and the reserved
root-level `build/` compiler output; package-authored ignore files do not control
source identity, nested `build` directories remain source, and immutable Git
materialization remains an exact selected-tree check. Local directory listings
are bounded before sorting, and Git paths/symlink targets receive a
host-independent Windows portability preflight before materialization. Git
fetch requests only
the selected revision at depth one and disables automatic maintenance/GC;
strict transferred-byte and object-store quotas still require a hardened
execution backend.
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
or special kinds. The same walk applies source-scaled, absolutely capped
logical resident-byte ceilings to accepted Git entries and local publications.
Those post-helper checks can reject an oversized cache but cannot prevent
temporary disk exhaustion during an unconfined fetch. Hostile same-user racing,
Windows ownership/DACL enforcement, and native isolation remain open.
Symbolic selectors use a bounded remote advertisement only to choose the
quarantine's SHA-1/SHA-256 object format; malformed, absent, or mixed formats
reject, and the advertisement never substitutes for parent authentication.
The Git parent executable is selected from closed absolute concrete platform
paths, not ambient `PATH`; macOS excludes Apple's dispatcher. Its canonical
bytes are retained as a diagnostic observation, guarded by stable file identity
around every launch, and re-hashed when resolution returns. Git receives a
cleared, fixed environment and an explicit absolute working directory. This
closes ambient parent-executable and environment selection, but does not
certify Git or bind every helper it may
launch. SSH is noninteractive and strict about host keys, but still consumes
the user's default known-host and key files. Strict OS confinement, explicit
credential custody and during-write byte/resource enforcement remain. Ordinary
resolution is now bounded to 64 Git launches, independent of package file
count, and ten minutes, including cache-lock acquisition. Validated blobs use
one exactly framed `cat-file --batch` launch; blob payloads are shared ranges
over that bounded response and released before staged-source revalidation.
Cleanup/reaping has a separate two-second
deadline; the combined operation is therefore not a strict ten-minute
wall-clock bound.
Git, workspace-member, and external-local resolution now bind those pieces into
a `ResolvedPackageSource`: declaration and identity come from the immutable
snapshot and canonical source lineage, and canonical literal dependency rows
are projected without executing build code. Known-host lineage normalizes
GitHub and hosted GitLab HTTPS/SSH clone spellings; GitLab nested namespace
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
consulting downloaded dependency rows; legacy standalone compilation still
retains a narrow explicit `depend_as(..., Source::Path { ... })` compatibility
scanner. A package-side handoff translates only the validated custody closure
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
Observation schema v10 carries operation-attempt schema v9, retaining exact
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
lifetimes; successful opens mint monotonic IDs, duplicate and borrowed outputs
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
component bytes. Mutable byte carriers retain their complete pre/post
capacity, including unchanged tails, while mutable i64 carriers retain exact
pre/post values. Pre-state follows evaluation of every authored argument;
post-state follows provider return or halt, and input-only mutable ABI carriers
remain explicit even when unchanged. Rooted/path-alias spellings stay out of
the payload lane. A separate 256 MiB aggregate operand-evidence sponsor
reserves immutable bytes and both mutable copies before that call's provider
access; prior or nested staging effects remain cleanup-contained. The package
commitment frames exact tags, ordinals, bytes, and pre/post values without
rendering payloads as text. Path-like bytes not represented by rooted evidence,
preparation-failure operand prefixes, retained returned-path bytes, and complete
content remain absent, so this is an
incomplete trace rather than a transcript or receipt.
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
Compiler-issued package review carries this summary
outside canonical capability/API comparison bytes. It is not a receipt and
makes no replayability or source-rebuildability claim.
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
This is staged-tree replay, not operation replay, a receipt, or generated-output
handoff; hostile same-user racing remains outside this custody rung.
Checked package compilation now also retains
the exact root package and selected build-machine symbol and can emit an
in-memory authority review projection for one explicit target. That projection
is intentionally not complete source/toolchain-bound admission evidence.
Authored toolchain nominals now retain a domain-separated commitment over the
canonical toolchain-relative source path and exact bytes. Compiler-generated
symbols inherit the package/toolchain provenance of a mandatory authored
derivation origin, while truly source-free symbols remain unresolved and
the 22 exact compiler-installed builtin types use closed compiler atoms selected
by root slot and symbol kind rather than spelling. Same-named package types and
source-free generated types remain unresolved. Compiler carry aliases use
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
second API row. Quotients, data `where` facts, and proposition parameters
reject review until exact canonical rows exist. Review v43 and canonical row
v3 represent static-machine parameters directly: structural contracts retain
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
Generic selected-conformance telescopes, boundary clauses, and unsupported
expression forms fail closed until complete rows land.
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
calls, compiler intrinsics, malformed or ambiguous joins, semantic roles, and
operators remain fail-closed until exact rows land.
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
arguments in selected conformance applications, and non-public, external,
operator, or lifetime-parameterized realizations fail closed until complete
rows land, except that binder-free
generic requirements, explicit evidence binders, and non-generic selected
conformances use the canonical public-trait row. Public callable `requires`
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
only when a unique checked semantic-place row agrees. Computed members,
proposition-argument members without that join, unsupported advanced call
forms, and aggregate expressions still fail closed. Contract casts retain the
structural operand, alpha-normalized target, arithmetic policy, package-qualified
semantic domain and arguments, and value/recast form. Diagnostic spellings are excluded;
private package domains reject when exposed by a public cast.
This join does not create a nominal Chi stage.
Ordinary standalone checked compilation still takes a caller-owned writable
build root when build-host staging is possible. Package review instead supplies
a package-specific root inside its orchestration-owned disposable child
session. Resolver snapshots remain immutable and are never repurposed as output
directories.
The legacy machine-contract fingerprint no longer enters package-review bytes,
so private state shape is not public contract identity. Complete proof and
unsupported-clause rows still gate sealed admission. The compiler now provides
a version-34 length-framed binary comparison encoding over this review
projection; it is explicitly not a package certificate or accepted-lock
payload. Raw Rust/debug serialization is not an alternative. These pieces do
not become an admission path until the legacy name-keyed lock APIs are replaced
and sealed, locally regenerated compiler evidence plus the hardened resolver
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

For ordinary package claims, “produced artifact” means the complete canonical
package-admission semantic row set under an exact package, target, dependency
closure, and obligation schema. It is not native code and it is not the
compiler-issued review object. Review may carry candidate bytes in the same
vocabulary, but a consumer must regenerate the total row set from exact source
and compare it exactly before those bytes can participate in accepted evidence.
Source, certificates, proof routes, compiler observations, and local decisions
remain separately bound. The current incomplete review-v47 projection therefore
does not become a package artifact or `PackageInstance` by renaming it.

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

Ratified 2026-08-24: the compiler owns both the semantic extraction and the
canonical conflict-row boundaries. It may read different rows from different
compiler-owned representations, including private pre-Psi structural state,
and move those joins as compiler internals evolve. Checked acceptance and
effects still come from the stage that establishes them, and projection occurs
only after successful checking. Package orchestration receives only independently
framed, versioned bytes and compares them exactly; it does not parse compiler IR
or duplicate capability semantics.
This does not create a nominal Chi stage. A new stage is warranted only if
implementation discovers a genuine shared semantic invariant, not merely to
stabilize a private checker interface. The initial callable row is one complete
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

Compiler issuance now retains a separately bounded canonical row sequence.
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
closed reasons. Exact nested clause/use-site coordinates and durable root-policy
resolutions remain unfinished engineering work; none independently motivates
nominal Chi.

The former commands accepting `manifest.json`, `receipt.json`, `--package`, or
mandatory `--alias` are quarantined from the production CLI. Their manifest,
lock, review, install, update, and audit modules compile only for isolated crate
tests while the typed replacements are built; they are absent from the release
library API. Invoking the old command names fails before parsing or writing any
artifact.

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

## Expected structure

```text
omega-packages/
|-- README.md
|-- src/
|   |-- identity.rs        # Package/source lineage and instance identity.
|   |-- source.rs          # Source requests and immutable snapshots.
|   |-- package_source.rs  # Snapshot-to-declared-PackageKey custody.
|   |-- resolver.rs        # Fetch/cache boundary and transport receipts.
|   |-- declaration.rs     # Hermetic PACKAGE extraction.
|   |-- dependency_projection.rs # Hermetic literal source requests.
|   |-- dependency_edit.rs # Digest-bound conservative build.omg edit plans.
|   |-- graph.rs           # Typed pre-admission source reconciliation.
|   |-- closure_resolution.rs # Bounded recursive immutable source custody.
|   |-- source_adapter.rs  # Explicit workspace and Git closure policy.
|   |-- compiler_handoff.rs # Revalidated package-aware compiler inputs.
|   |-- source_commands.rs # Unhardened source diagnostic command surface.
|   |-- source_patch.rs    # Bounded hostile-data source review packet.
|   |-- review_evidence.rs # Private live/recovered comparison evidence seam.
|   |-- review_baseline.rs # Bounded restart-stable non-admitting capsule.
|   |-- review_closure.rs # Shared exact-key compiler-review/custody checks.
|   |-- source_review.rs   # Custody/evidence join and aggregate review input.
|   |-- source_triage.rs   # Compiler-row source/provenance triage.
|   |-- capability_conflict.rs # Bounded review-only exact row conflicts.
|   |-- evidence.rs        # Compiler-issued package admission evidence.
|   |-- lock.rs            # Accepted closure and evidence baseline.
|   |-- conflict.rs        # Future durable root-policy resolutions.
|   |-- audit.rs           # Source/provenance/capability audit rendering.
|   |-- install.rs         # Fetch, derive, admit, then edit/write.
|   |-- update.rs          # Candidate reconciliation and admission.
|   `-- commands.rs        # Thin CLI-facing orchestration.
`-- tests/
    |-- identity.rs
    |-- install.rs
    |-- update.rs
    |-- audit.rs
    `-- remote_fixtures.rs
```

Machine persistence format is an internal encoding choice. Human review and
conflict surfaces use concise canonical text and do not expose package-authored
prose to the triage model.

The checked package-review path also fails closed on contract-entailment
stand-downs. It audits the pristine typed graph (including generic templates),
retains compiler-owned machine/contract/fact coordinates and a closed reason,
and refuses review when any checked-implementation claim was left unjudged. Accepted or
opaque supply remains trust-bearing. These rows are currently in-memory review
state, not sealed lock evidence.

## Fixtures

The local fixture corpus is under `fixtures/packages/`; exact remote Git pins
are recorded in `fixtures/packages/REMOTE_PINS.md`. Every fixture declares
`PACKAGE`. The package-evidence integration canary resolves real immutable
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
rows. Remote CathedralOS fixture verification remains credential-gated and
fail-closed.
