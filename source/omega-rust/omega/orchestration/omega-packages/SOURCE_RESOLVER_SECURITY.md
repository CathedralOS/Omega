# Source resolver security boundary

Status: engineering contract, revised 2026-08-28. This document refines
`HARDEN-SOURCE-RESOLVER`; it does not define package or Omega language syntax.

## Boundary

Source resolution is a privileged supply-chain operation. Downloaded package
code receives none of the resolver's transport, cache, credential, project, or
acceptance authority. Compilation consumes a resolver-owned immutable snapshot,
never a live local tree, Git working tree, or helper-produced claim.

The intended strict production path has three custody stages. The current
macOS floor now confines repository-initialization, inspection, and HTTPS
discovery/fetch file content but does not yet enforce read separation for SSH
or metadata:

1. A fetch helper resolves transport into a fresh quarantined object store. In
   the strict boundary it has the selected transport authority and no access to
   project state or final snapshots.
2. A no-network materializer reads only validated objects and writes ordinary
   files and symlinks into a fresh snapshot stage. It never runs checkout
   filters, hooks, submodules, or package executables.
3. The trusted parent independently validates the resolved object identities,
   hashes the materialized tree, atomically publishes it, and issues the source
   receipt. Helper output is observation, not evidence.

Local sources skip transport but still pass through snapshot staging. A hash of
a live local directory is diagnostic only.

## Portable executor floor

Every helper launch must use:

- an absolute pre-resolved executable with retained content identity;
- no package-controlled shell interpolation, null standard input, bounded
  output, and a deadline;
- `env_clear` followed by an explicit adapter-specific environment;
- an explicit working directory and exclusive staging roots;
- adapter-specific protocols with no ambient protocol helper;
- disabled prompts, hooks, filters, submodules, replacement objects, and
  user/system configuration; and
- file, byte, depth, object, process-output, and elapsed-time ceilings.

These controls do not constitute an OS sandbox. Strict admission also requires
a platform backend that can confine descendant processes, filesystem access,
network destinations, executable selection, and CPU/memory/process resources.
Linux may use namespaces/Landlock/seccomp/cgroups, macOS an available Seatbelt
launcher plus resource/process controls, and Windows a restricted token or
AppContainer plus a kill-on-close Job Object. If the selected backend cannot
establish a required guarantee, strict resolution rejects; it never degrades to
"best effort."

The current macOS engineering floor now selects a fixed Seatbelt launcher and
closed resolver phase; it is described below. Every phase uses a self-contained
compiler-generated policy with no host-profile import and confines writes and
executable paths; the nonnetwork phases also deny network. SSH discovery/fetch
reads remain broad. Initialization, inspection, and HTTPS discovery/fetch
confine file-content reads to their exact working, mutable-quarantine, or
retained-repository root and fixed runtime files while retaining broad metadata
reads. HTTPS discovery/fetch also admit the fixed `/private/etc/ssl` system TLS
configuration root. Each network phase confines its child to one compiler-owned
loopback broker port. The broker accepts only the normalized host and port derived from the
validated locator and records the effective connected peer. Linux and Windows
route selected helpers through the same broker but do not yet deny direct
egress, so their endpoint-confinement row remains unavailable.

SSH additionally
requires a pinned client, explicit known-host evidence, empty user
configuration, and an explicit credential-provider class. Ambient agent use is
a distinct trust class, not the default.

## Resolver-owned receipt

The future `SourceResolutionReceiptV1` is an opaque, canonical value issued by
the trusted resolver path. Parsing a persisted receipt never mints authority.
It binds:

- schema, canonical encoding, resolver, adapter, and helper identities;
- canonical source lineage, sanitized locator identity, selector, and transport
  profile;
- isolation backend and the required/enforced guarantee set;
- filesystem, executable, environment, endpoint, deadline, and resource policy;
- requested/effective endpoint, redirect policy, TLS or SSH host trust,
  credential-provider class, and transferred-byte observations;
- object format, commit, tree, object-integrity, submodule, and gitlink verdicts;
- snapshot policy, content identity, entry counts/bytes/depth, mode/symlink
  policy, and immutable publication identity; and
- phase results, resource observations, bounded diagnostic digests, truncation
  flags, and one closed accepted/rejected reason code.

Containment facts are closed rows such as
`FilesystemDeniedOutsideScopes`, `NetworkDeniedOutsideEndpoints`,
`DescendantsContained`, `ExecDeniedOutsideToolSet`,
`AmbientConfigurationDenied`, `ResourceCeilingsEnforced`, and
`ImmutableSnapshotPublished`. Each is either backed by exact enforcement
evidence or marked unavailable. An accepted strict receipt has no unavailable
required row.

The former JSON `SourceCachePolicyRecord` diagnostic and its CLI persistence
surface are deleted. They duplicated a subset of live resolver state through
free strings and mutable paths but could never become this receipt. `omega
audit source` prints a bounded human diagnostic from a fresh live resolution;
it cannot be recovered or promoted into an accepted lock. Authoritative source
persistence must begin with the future opaque receipt rather than a caller-
readable intermediate record.

The native execution crate now returns a narrower opaque policy observation
with each command it constructs. It binds the verified backend, closed phase,
generated policy hash, numeric compiler ceilings, primary executable path,
normalized bounded descendant-executable path set, mutable root, sealed
endpoint route where applicable, exact discovery/inspection content-read roots
where applicable, and a complete ordered guarantee vocabulary.
Required guarantees are either `Enforced` or
`Unavailable`; phase-inapplicable rows are `NotRequired`. There is no public
constructor or decoder, path and helper counts are bounded, and
`require_strict` rejects any unavailable row. Git resolution retains one value
for every configured command only inside a resolution that ultimately
succeeds. This prevents a phase-only summary from hiding different executable
or filesystem scopes, but it is configuration provenance—not proof the command
ran and not a `SourceResolutionReceiptV1`. The complete receipt must join the
package layer's exact executable content observations, environment/protocol
sealing, endpoint and credential trust, bounded command result, object
authentication, snapshot identity, and final publication verdict.

Every macOS phase uses a compiler-generated default-deny profile with no import.
All grant exact selected executables and write-data to `/dev/null`. SSH
discovery/fetch grant broad reads. Initialization, inspection, and HTTPS
discovery/fetch grant broad metadata reads but file-content reads only beneath
the exact working, mutable-quarantine, or retained-bare-repository root, from
the exact executable set, `/dev/null`, and the literal filesystem-root directory
entry required by the native process runtime. HTTPS discovery/fetch additionally admit `/private/etc/ssl`;
that fixed path is not evidence that TLS trust or custody was established.
Initialization and fetch additionally grant writes only beneath the exact
mutable quarantine root. Discovery and fetch require the already-validated
closed HTTPS or SSH transport authority and grant outbound network. Only SSH
receives the exact OpenDirectory libinfo lookup, `kern.hostname`, and
`hw.pagesize_compat` reads needed by the pinned client and compiler-owned Rust
connector; HTTPS receives none of them. Each network child may connect only to
its exact loopback broker port, so macOS endpoint confinement is `Enforced`.
Initialization and inspection deny network and reject a
transport authority. They also omit process-fork authority entirely, so an
allowlisted executable still cannot become a descendant in either nonnetwork
phase. Filesystem-write and executable-path rows are
`Enforced` for every phase, network denial is `Enforced` where applicable, and
descendant containment is `Enforced` for initialization and inspection. The
exact compiler-owned rlimit rows are `Enforced` throughout macOS.
Because metadata remains broad and SSH discovery/fetch retain broad content
reads, `FilesystemReadsConfined` remains `Unavailable`.
Before a successful Git result is issued, the package layer also requires the
number of retained policy observations to equal the bounded launch count and
requires every observation's executable path set to equal the paths backed by
the still-verified Git, selected transport, and fixed platform-helper content
identities for that phase. The result now retains those fixed helper identities
instead of dropping them. This closes configuration-to-content association for
successful resolution; it still is not an execution-result receipt.

Each completed Git command now also contributes one bounded package-layer
execution observation after output capture, executable/backend revalidation,
and budget reconciliation succeed. A domain-separated command commitment binds
the closed phase, actual native program and ordered arguments, complete explicit
environment, working directory, and either null stdin or the exact object-batch
stdin length and digest. The outcome binds exit code or Unix signal plus exact
bounded stdout/stderr lengths and digests, and joins positionally to the digest
of its native policy observation. Network commands additionally retain the
sealed route, bounded CONNECT outcomes, and effective peers; successful remote
issuance requires at least one connected event and exact route-policy equality.
Successful resolution requires outcome,
policy, and launch counts to agree. Both capture threads and every command also
charge one overflow-safe whole-resolution counter. Its compiler-owned ceiling
is `min(source-byte ceiling + 64 MiB, 576 MiB)`. Exhaustion terminates and reaps
the command container with a distinct error. Successful issuance requires the
counter to equal the sum of every retained stdout/stderr length and binds both
ceiling and observed count into the final observation. Output text and package-
controlled arguments are not rendered into this fixed record. This establishes
completed-command provenance and cumulative parent-captured-output accounting
for successful resolution; it is not network-transfer, object-store, or
descendant aggregate-resource accounting.

After the outer resolver has additionally reconciled cache namespace/custody
and final executable content, it physically reopens and re-hashes the published
snapshot under the original exact-tree policy while the cache lock remains
held. Only then is a private pending result converted into the sealed public
`ResolvedGitSource` and one compact `GitSourceResolutionObservation` issued.
The public result fields are not mutable. The observation's canonical identity
binds the exact source policy ceilings, request and normalized locator,
transport and object format, selected commit/tree, immutable snapshot
path/content/counts, Git and helper content identities, every native policy row,
every completed-command row, and the cumulative captured-output ceiling and
observed count.
The observation has no public constructor or decoder and is issued with the
fixed outcome `resolved-non-admitting`; changing even a source ceiling changes
its identity. This closes the successful-result join that the narrower rows
could not express. It deliberately remains below `SourceResolutionReceiptV1`:
unavailable containment rows remain unavailable, and Linux/Windows endpoint
confinement, TLS/SSH trust, credential custody, transferred bytes, object-store/during-write
quotas, descendant aggregate resources, and strict acceptance are still absent.

Resolved package custody now also projects a bounded
`CanonicalSourceClosureSubject`: the exact root request and every authored
dependency request occurrence joined to its alias, selected package key, and
immutable resolution/content identity. This is a canonical question for later
independent reconstruction, not the resolver receipt described above. It omits
cache/snapshot paths, execution-transport observations, isolation claims,
compiler consumption, and phase verdicts. Decode and fingerprinting grant no
authority; a consumer must independently resolve and snapshot the source and
require complete subject equality.

## Current engineering delta

Git resolution now treats helper-reported IDs, listings, and framing only as
inputs to parent-owned verification. An exact requested object ID must equal the
selected commit. The parent hashes the raw selected commit, parses and compares
its root-tree edge, collision-checks every SHA-1 object hash, hashes every
returned blob, retains every explicit child-tree edge, reconstructs each
canonical recursive Git tree object including empty trees, and compares the
resulting Merkle root with the selected tree. SHA-1 compatibility rejects known
collision attacks but does not restore theoretical SHA-1 collision resistance;
SHA-256 uses its ordinary full digest and remains preferred. Fixed object vectors,
real repositories, and mismatch tests cover object bytes, exact-pin binding,
commit identity, commit-to-tree and tree-to-child edges, empty trees, canonical
Git ordering, and destination containment. All authentication and destination
preflight completes before a snapshot staging path can be created. The
authenticated bytes and explicit directories are then materialized into a
staged, read-only, atomically published snapshot without invoking checkout,
filters, hooks, submodules, or package code; the published source is re-hashed
and compared directly with the identity derived from freshly authenticated Git
entries before every reuse. Snapshot metadata must agree too, but is descriptive
and cannot authorize replacement bytes even when rewritten to match them. This
establishes the selected-object-graph to snapshot shape, not the complete
production boundary.

Local sources now use a bounded in-memory capture, content-addressed staging,
read-only atomic publication, and revalidation before reuse. Physical
publication is additionally namespaced by the canonical live-source lineage,
so byte-identical packages from distinct paths retain distinct compiler custody
roots while keeping the same content identity. The resolver
rejects source/cache overlap and ordinary mutation observed between capture and
publication; compilation-facing diagnostics expose the published snapshot, not
the live tree. Empty directories participate in identity while directory
permissions normalize to the canonical snapshot policy. Local package capture
excludes only repository metadata and the compiler-reserved root `build/`
output directory; it does not trust package-authored ignore files. Nested
`build` directories remain ordinary source, and a symlink into excluded output
rejects. Each directory listing is bounded before it is retained and sorted by
the remaining source-entry allowance plus only the toolchain-reserved names
that may be excluded at that level. Resolver-owned materializations are checked
under an exact-tree policy, so immutable Git snapshots still preserve every
selected tree entry.

Capture acquires the canonical source root by walking from its filesystem
anchor and opening every directory component no-follow, then traverses relative
to retained directory capabilities. Child directories and regular files open
without following their final path component, and file bytes are read
immediately from the retained handle rather than from a pathname saved during
classification.
Replacing a classified leaf with a symlink therefore cannot redirect capture;
replacing the root pathname after it is opened does not redirect the open
session either. Symlink spelling and target validation remain rooted in the
same capability. Absolute local-link spellings reject because copying them
would preserve authority to the mutable live pathname rather than the published
snapshot. This narrows pathname substitution but does not make a
multi-file capture atomic or defeat a hostile same-user process that mutates
ordinary files and directories through its own credentials.

Cache-tree custody is now walked from a retained canonical-root capability.
Root acquisition opens each absolute path component no-follow; directory
enumeration and metadata classification remain relative to retained parents;
and every queued child directory is reacquired component-by-component from the
retained root, opened no-follow, and checked for stable file identity before
descent. The queue retains paths and classified metadata rather than one open
descriptor per sibling; a fixed 260-level ceiling bounds reacquisition work.
The same bounded walk serves Git cache entries and local snapshot publications.
This prevents a directory leaf reclassified as a symlink or different concrete
directory from redirecting the later traversal and keeps a walk bound to an
already-opened root if its pathname is replaced. Git cache-entry, Git snapshot,
and local snapshot publication additionally open the canonical publication
parent component-by-component no-follow, reduce both operands to validated
direct-child names, and rename through that retained directory capability.
The parent then confirms the published directory has the staged directory's
file identity and synchronizes the retained parent handle. A replaced parent
pathname therefore cannot redirect the publication operation. Git and local
publication locks likewise open a validated direct-child name no-follow through
a retained parent capability. Lock acquisition confirms leaf identity relative
to that parent after waiting and requires the canonical parent pathname still
to identify the retained directory; the parent capability remains alive for
the lock lifetime. Git invalidation opens the entry as a stable direct child
through the retained cache parent and removes resolver metadata relative to the
retained entry directory; a substituted entry symlink cannot redirect that
deletion. Resolver-owned control records—Git configuration, Git source identity,
and Git/local snapshot identity—are read through retained no-follow directory
and file capabilities with per-record byte ceilings, repeated handle reads, and
leaf/handle identity confirmation. Immutable Git-tree and local-source snapshot
stages are also rooted in retained snapshots-parent and exact stage-directory
capabilities. Directory, create-new file, symlink, identity-record, permission,
recapture, publication, and failure-cleanup operations remain beneath those
handles. Source identity is recaptured again after read-only finalization, whose
child opens must preserve their classified identities. Publication rejects if
the stage name no longer identifies the retained stage, and cleanup begins from
the retained stage handle. The host
cleanup primitive is not atomic against concurrent rename and is pathname-
based on Windows, so cleanup remains best-effort under a hostile same-user
process. Mutable Git object-cache staging now retains the lock-acquired cache
parent and exact stage directory across creation, resolver-metadata emission,
publication, invalidation, and cleanup. The stage has exact Unix mode `0700`,
its resolver metadata has exact mode `0600`, setup-time native Git calls are
bracketed by parent/stage pathname-identity checks on success and failure,
publication requires the retained stage identity, and invalidation plus parent
synchronization remain relative to that same parent. Namespace and
invalidation failures outrank ordinary operation errors. A provisional
parent-relative guard covers the interval before the new stage is retained;
after retention, cleanup starts from the open stage and does not delete a
replacement at its former name. Native Git still consumes the stage
and repository through pathnames; a rename race during a launch and strict
mutation confinement therefore remain native-isolation work. Native Git
repository verification now retains the exact cache entry, Omega-created bare
repository, and object-store directories for the complete resolution.
Control-record reads/restoration, recursive repository shape, and
forbidden-indirection absence checks derive from those capabilities; only exact
`NotFound` proves absence. The Omega-owned repository policy rejects all
symlinks and, on Unix, multiply-linked regular files while allowing ordinary
fetch products such as shallow state, refs, loose objects, and packs. Every
repository-bound Git launch, including the bespoke blob batch, reconciles the
retained entry/repository/object identities after success or failure, and full
static shape is checked after fetch and before acceptance. This is not a claim
that arbitrary bare Git repositories have that stricter shape. The blob-batch
request is now an exact-mode `0600`, create-new file beneath the retained cache
entry. Its handle/name identity is checked around use, explicit cleanup is
parent-relative and synchronized, and an already observed replacement name is
preserved. The final check/unlink pair is not atomic against an active same-user
rename race. The Git snapshot collection is classified through that same
retained entry; only exact `NotFound` permits private `0700` creation, and later
publication lookup and materialization staging use the retained collection. Git still receives the
repository as a pathname, so launch-race confinement remains open. Published
Git/local snapshot mode and shape verification opens the publication and Source
roots no-follow, traverses retained child handles with
identity checks, and captures content from that same open Source root.
Authenticated Git paths, kinds, and
executable bits are compared with the captured tree rather than ambient
metadata. On macOS, retained cache directories, regular files, and locks are
queried for extended ACL allow entries through their descriptors. Concrete
selected executables and their ancestry are descriptor-queried as well. Cache
root/ancestry ACL facts likewise follow no-follow directory acquisition and
identity reconciliation; only symlink ACL observations remain path-based.
Destination exclusion is cooperative rather than an atomic hostile-same-user
no-replace primitive, so this still does not claim exclusion of an actively
hostile same-user process.

Compilation handoff now captures and revalidates the root package of each
package compilation again, then asks the compiler to independently recapture
that same root into one closed canonical Source-metadata index bound to its
compiler-owned full-content commitment.
Indexes for reachable dependencies are not retained in that compilation; when
review re-roots to a dependency, it receives its own freshly derived root
index. The index
contains raw root-relative paths, directory/file/symlink kind, executable bits,
ordinary-file lengths, symlink-target spelling lengths, and an implicit root
directory. It is bounded to 65,537 rows including the root and 16 MiB of
aggregate path bytes. Build-time `stat`, `lstat`, and `fstat` on Source obtain
mode, length, and a fixed timestamp from this index; followed `stat` selects the
resolved target row, `lstat` selects the authored leaf, and an open handle keeps
its selected row for `fstat`. A path absent from the complete index rejects
rather than falling back to physical checkout metadata. Source directories are
`040555`, files are `100444` or `100555`, symlinks are `120777`, directory size
is zero, and the canonical timestamp is exactly 1,000,000,000 seconds after the
Unix epoch. The writable Output grant continues to expose physical staging
metadata. Package-aware filesystem execution rejects an absent root index and
anchors Source to the exact package root, not the selected entry file's parent.
Callers cannot supply package index rows or their commitment. The compiler
captures canonical shape, mode, length, and full file/symlink content itself
under a 512 MiB aggregate-content ceiling; input construction independently
recaptures the complete physical root and requires exact index equality. The
compiler repeats the content check at compilation entry and immediately before
returning checked evidence or publishing a legacy result.
Non-root dependency indexes reject rather than accumulating across the closure.
The package layer still performs its stronger resolver-policy revalidation and
source-resolution binding immediately before compiler capture. Replay records
bind the metadata policy version and compiler-owned full-content commitment,
and package-aware no-host replay rejects a mismatch with the current root
index. The provider also checks that the host object kind agrees with the
indexed kind;
this detects ordinary drift but does not defeat the same-user race described
above. The index is compiler sponsorship, not a source-resolution receipt and
does not add authority to persisted metadata.

Transport erasure now retains the original file, byte, and depth limits beside
each package snapshot. Review orchestration checks canonical read-only modes
and re-hashes every transitive snapshot under those limits immediately before
and after each package compilation. Any mismatch rejects the review set. This
is joined to a compiler-issued, domain-separated commitment over the canonical
reconciled input graph and every exact source path and byte sequence retained by
the frontend. The compiler re-reads each physical source before returning;
orchestration repeats that comparison after its whole-snapshot post-check.
Together these bind review rows to compiler-consumed bytes and detect ordinary
custody drift, but they are not an atomic filesystem snapshot: a hostile
same-user process may still race every observation.

Review-time source patching reuses that custody boundary. It captures the
verified bytes once under the exact-materialized policy and diffs that private
capture; it never reopens a live checkout, asks Git for a patch, or reapplies
mutable-local `.git`/root-`build/` exclusions to a published snapshot. A second
whole-snapshot verification after rendering detects ordinary intervening
mutation under the same documented same-user race limitation.

Git subprocesses now select the parent Git binary only from a closed platform
list of absolute concrete paths; they never search ambient `PATH`. macOS does
not select Apple's `/usr/bin/git` dispatcher. The resolver canonicalizes the
selected regular file and, on Unix, requires root/resolver ownership,
non-writable/non-set-id executable mode, and root/resolver-owned ancestry whose
externally writable directories have sticky-entry protection. Those custody
conditions are rechecked around every launch. The resolver hashes the file
under a 256 MiB ceiling, retains that observation on `ResolvedGitSource`, checks
stable file identity before and after every launch, and re-hashes the bytes when
the complete resolution returns. HTTPS requests select `git-remote-https` from
a closed install-relative candidate set, retain its invocation entry and
canonical target, and apply the same observation and Unix custody checks to
both identities. `GIT_EXEC_PATH` and `PATH` expose only that observed helper
directory. SSH requests apply the same observation and Unix custody checks to
one exact client. On macOS the fixed shell executables required to realize the
sealed SSH command receive the same identity, hash, custody, and ACL treatment;
they do not grant execution of any unlisted descendant. Both transports recheck
their executable identity around every Git launch, re-hash the canonical target
at completion, and retain it separately. Drift rejects. The Git cache policy is
v16, so a cache fetched before these executable-custody, cumulative-output,
transport-authority, endpoint-brokerage, and nonnetwork descendant-denial floors
is not silently reused.
This identifies observed parent bytes and closes ordinary cross-user path
ownership on Unix; it does not certify Git, the HTTPS helper, or SSH, bind
other executable components, establish Windows ownership/DACL custody, protect
against same-user replacement, bind TLS or SSH host trust, or
prove that an observed file equals an already loaded image. On macOS, every
concrete selected executable, transport invocation entry, canonical transport
target, and executable ancestor is opened no-follow, required to preserve its
classified file identity, and read through the descriptor form of the native
extended-ACL surface. Symlink invocation entries use the path-oriented native
link ACL interface. The narrow platform wrapper classifies only native
allow/deny tags and does not resolve ACL principals through ambient identity
services. Any allow entry rejects; deny-only entries cannot broaden custody.
Failure to inspect an ACL rejects rather than degrading to mode-only checks.
These ACL checks run at the same repeated custody points as owner, mode,
ancestry, and executable identity checks. They close ordinary extended-ACL
grant substitution, not hostile same-user replacement or loaded-image
identity.
Each launch clears the complete inherited environment, installs only the fixed
Git/protocol/locale/helper-path variables, and uses an explicit absolute cache
or repository working directory. It also receives resolver-owned stdin,
concurrent bounded stdout/stderr capture, and a deadline. Stdin is null except
for the exact object-ID request file supplied to `cat-file --batch`. Per-command
stream ceilings remain independent of the whole-resolution captured-output
ceiling described above.

`omega-resolver-execution` derives native policy from one of four closed phases:
transport discovery, repository initialization, fetch, or repository
inspection. On macOS it verifies `/usr/bin/sandbox-exec` as a root-owned,
non-writable, non-set-id executable beneath root-owned ancestry, rejects native
extended-ACL allow entries, binds its content hash and file identity, and
rechecks that identity before constructing each command. Compiler-fixed policy
adds outbound network only during discovery and fetch, quarantine mutation
during initialization and fetch, and exact process-exec paths for the verified
  Git/helper chain. No phase imports a host profile; initialization and fetch
  confine mutation to the exact quarantine root while discovery and inspection
  admit write-data only to `/dev/null`. Real Git resolution and native canaries
  exercise those policies. Network phases permit only the exact loopback broker
  port; the parent broker resolves and admits only the validated requested host
  and port and records the actual connected peer.
`/usr/bin/sandbox-exec` is deprecated, so this is a concrete current-host floor,
not a durable macOS backend promise. Failure to establish or revalidate the
launcher rejects on macOS.

Every Unix resolver child inherits at most 120 CPU seconds, a zero core-file
limit, a 1 GiB file-size ceiling, and at most 256 descriptors. Linux/Android
also receive an 8 GiB address-space limit; Darwin does not expose a usable
equivalent through this rlimit path. Each compiler ceiling intersects the inherited soft
and hard limits and therefore never loosens a stricter host limit. These limits
are inherited per process, not an
aggregate descendant/process-count/object-store/transfer budget. Linux still
lacks filesystem, executable, and network confinement; Windows still has only
the existing kill-on-close process container. Native canaries exercise denied
writes, denied unlisted descendant execution, denied inspection networking,
and admitted discovery networking. Hermetic loopback canaries also exercise the
selected production HTTPS helper and fixed shell/SSH executable chains through
the same allowlist. The full Git source suite runs through this boundary.

Before initializing a cache for a symbolic selector, one bounded `ls-remote`
request asks only for `HEAD` and that selector and rejects absent, malformed,
or mixed object formats. The discovered SHA-1/SHA-256 format controls quarantine setup
but is not evidence; parent-owned object authentication still decides whether
the selected graph is coherent. Fetch requests only the selected revision at
depth one, disables automatic maintenance and garbage collection, and requests
`blob:limit=<source-byte-ceiling + 1>`. Lazy object fetching is disabled during
all later Git commands, and the parent restores its byte-exact canonical bare
configuration after a successful filtered fetch before authenticating objects.
A required individual blob above the accepted source ceiling therefore remains
absent and causes fail-closed tree authentication. Exact object-ID pins reuse
and re-authenticate existing cache custody without transport; symbolic selectors
still refetch. This bounds unrelated history and individually impossible blobs,
not the admissible bytes of a still-whole selected root. Selective subtree
acquisition remains dependent on an exact member-path selector. A whole-
resolution budget now caps
launches at 64, independent of package file count, and limits ordinary elapsed execution to ten
minutes; each command receives only the smaller remaining interval. One exactly
framed `cat-file --batch` launch reads all validated blobs in tree order. Each
subprocess starts in a fresh Unix process group or
Windows Job Object. Completion and rejection paths attempt to terminate that
container before returning; ordinary helper and SSH descendants therefore do
not survive or hold capture pipes open in the tested cases. Cleanup/reaping has
a separate two-second deadline and fails closed if portable process APIs do not
finish within it. A descendant escaping its Unix session remains outside this
portable guarantee, and the cleanup allowance means the per-command deadline
is not a strict wall-clock guarantee. Overflow and timeout reject explicitly once
cleanup returns, including for blob reads. On Linux this process container
floor is not an OS sandbox: a hostile Unix descendant may deliberately escape
into another session. The macOS Seatbelt floor adds phase-specific native
confinement but not endpoint, read-scope, or aggregate-resource custody.
Depth-one fetch limits history amplification but does not enforce a transferred-
byte or object-store quota. The launch ceiling and inherited rlimits are not an
aggregate CPU, memory, process-count, object-store, or transfer-work budget.
Materialization remains trusted parent code rooted in retained filesystem
capabilities rather than a separate sandboxed helper. A deliberately hostile
same-user process can race
cooperative locks and validation, including the local before/after observation.
Git no longer stores a cache-local remote origin. Fetch receives the exact
resolver request directly, and the parent overwrites local repository config
with one byte-exact SHA-1 or SHA-256 bare-repository file. Replacement uses a
synchronized stage held open across a handle-relative atomic rename, then
checks the published pathname, file identity, exact bytes, and parent-directory
synchronization; the repository record root itself is acquired
component-by-component no-follow, and there is no delete/recreate gap. Before every use the parent
reads and compares those bytes itself; added settings, includes, remotes, or
spelling drift reject without asking Git to report its own configuration.
Git and local cache custody also receive a separately bounded 65,536-node
parent traversal before and after use. That traversal sums regular-file and
symlink logical lengths. Git entries reject above
`min(3 * source-byte-limit + 64 MiB, 1 GiB)`; local publications reject above
`min(source-byte-limit + 64 MiB, 512 MiB)`. These are post-helper acceptance
ceilings for resident cache state, not during-write disk quotas or transferred-
byte measurements: a helper without an aggregate disk quota may still exhaust
storage before the parent can reject its output. Every Git or local publication lock opens
no-follow relative to a retained canonical-parent capability. After waiting,
the resolver compares the locked handle with that parent's current leaf and
compares the retained parent with the current canonical parent pathname using
platform file identity. A symlink leaf or replaced parent therefore rejects
rather than selecting a different cooperative lock namespace. Git waits
consume the whole-resolution deadline, while
local publication waits use a separate two-minute compiler-owned deadline and
reject explicitly on expiry. On Unix each cache entry and lock must be owned by
the resolver's effective user and not group- or other-writable;
canonical ancestry must be root/resolver-owned and cannot be replaceable
through a non-sticky writable directory; unsupported filesystem kinds reject.
On macOS the same custody walk inspects native extended ACLs on every ancestor,
cache, publication, staging, and lock node. Each already-open cache directory,
regular file, and lock is queried through its descriptor; regular files first
open no-follow beneath their retained parent and preserve classified identity.
Any allow entry rejects even when mode bits appear private; deny-only ACLs do
not broaden custody, and an unreadable ACL fails closed. Root and ancestry
owner/mode classification remains path-based, but each ACL fact follows a
no-follow directory open and identity reconciliation. Symlinks are inspected
as links rather than following their targets because the native link ACL
interface is path-oriented.
This closes ordinary cross-user
ownership/configuration substitution on Unix. It does not prevent the owning
user from replacing a path after an observation, establish Windows
ownership/DACL policy, or establish strict native isolation on every platform.
Git path and symlink preflight rejects Windows
drive/alternate-stream colons, forbidden characters and controls, trailing
dots/spaces, and reserved device names independently of the host path parser.
HTTPS Git commands can select only the observed install-relative
`git-remote-https` invocation entry and its retained canonical target. Other
executable components beneath Git remain outside retained identity and the
macOS backend's execution allowlist. SSH is forced through its content-observed
absolute client and the separately custodied `omega-resolver-connect`
companion. The fixed ProxyCommand receives the broker and target only through
compiler-authored environment fields; no locator string enters shell syntax.
User configuration is disabled with `-F none`, with `BatchMode`, zero password
prompts, and strict host-key checking. It still
consults the user's default known-host and key files, so host and credential
custody remain ambient and unsuitable for strict admission. Those conditions
keep the resolver diagnostic-only until strict native confinement on every
supported platform, hostile-process custody, during-write resource ceilings,
and opaque-receipt work land.

Public requests admit only HTTPS and SSH transports. The validated request
retains an execution profile distinct from transport-neutral hosted-repository
lineage: an HTTPS request permits only Git's `https` protocol, and either SSH
locator spelling permits only `ssh`. The cache key and exact metadata bind that
profile, so normalized HTTPS and SSH spellings cannot reuse custody established
under the other's authority. Resolved-source observations and human source-audit
output retain the selected
profile separately from normalized lineage. HTTP, unauthenticated `git://`, every unselected
protocol, and HTTP redirects remain disabled; `file` exists solely in the
explicit test-only local-repository adapter. This prevents a validated HTTPS
request from silently acquiring SSH/file authority or a redirect-selected
endpoint. The broker observation retains the requested endpoint, every bounded
CONNECT outcome, and each actual connected peer. On macOS the child cannot
bypass that route; Linux and Windows still can until their native backends deny
direct egress. None of this pins TLS or SSH host trust.

Parent-owned selected-object-graph authentication and the current macOS native
enforcement supply real evidence for a later strict receipt but do not by
themselves make the resolver admissible. Linux/Windows strict isolation,
hostile same-user and Windows ACL cache custody, aggregate/during-write resource
ceilings, cross-platform endpoint confinement, explicit SSH trust/credential
custody (OWNER Q16),
and the opaque receipt remain open.
