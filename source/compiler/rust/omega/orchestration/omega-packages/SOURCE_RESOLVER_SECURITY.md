# Source resolver security boundary

Status: engineering contract, revised 2026-08-26. This document refines
`HARDEN-SOURCE-RESOLVER`; it does not define package or Omega language syntax.

## Boundary

Source resolution is a privileged supply-chain operation. Downloaded package
code receives none of the resolver's transport, cache, credential, project, or
acceptance authority. Compilation consumes a resolver-owned immutable snapshot,
never a live local tree, Git working tree, or helper-produced claim.

The production path has three custody stages:

1. A fetch helper resolves transport into a fresh quarantined object store. It
   has the selected transport authority and no access to project state or final
   snapshots.
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
- no shell interpolation, null standard input, bounded output, and a deadline;
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

Network destination authority should eventually be brokered. SSH additionally
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

The current `SourceCachePolicyRecord` remains diagnostic scaffolding. Its free
strings and mutable paths are not this receipt and cannot enter an accepted
lock. Its persistence path is nevertheless bounded and canonical: reads reject
symlink/non-regular leaves and noncanonical encodings, while writes use an
exclusive private same-directory stage, synchronized byte revalidation,
no-overwrite atomic publication, stage cleanup, and parent-directory
synchronization on Unix. The operation resolves and rechecks one canonical
parent, but
does not claim hostile same-user handle-relative custody or give the record
authority.

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
one exact client. Both transports recheck their executable identity around
every Git launch, re-hash the canonical target at completion, and retain it
separately. Drift rejects. The Git cache policy is v12, so a cache fetched
before these executable-custody floors
or under a different transport-authority profile is not silently reused.
This identifies observed parent bytes and closes ordinary cross-user path
ownership on Unix; it does not certify Git, the HTTPS helper, or SSH, bind
other executable components, inspect macOS ACL grants, establish Windows
ownership/DACL custody, protect against same-user replacement, bind TLS trust
or the effective endpoint, or prove that an observed file equals an already
loaded image.
Each launch clears the complete inherited environment, installs only the fixed
Git/protocol/locale/helper-path variables, and uses an explicit absolute cache
or repository working directory. It also receives resolver-owned stdin,
concurrent bounded stdout/stderr capture, and a deadline. Stdin is null except
for the exact object-ID request file supplied to `cat-file --batch`. Before
initializing a cache for a symbolic selector, one bounded `ls-remote` request
asks only for `HEAD` and that selector and rejects absent, malformed, or mixed
object formats. The discovered SHA-1/SHA-256 format controls quarantine setup
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
cleanup returns, including for blob reads. This process container floor is not
an OS sandbox: a hostile Unix descendant may deliberately escape into another
session. Depth-one fetch limits history amplification but does not enforce a
transferred-byte or object-store quota. The launch ceiling is not a CPU, memory,
object-store, or transfer-work budget. Fetch and
materialization still run in the parent process
without filesystem/network confinement or CPU, memory, process-count, and
transfer ceilings. A deliberately hostile same-user process can race
cooperative locks and validation, including the local before/after observation.
Git no longer stores a cache-local remote origin. Fetch receives the exact
resolver request directly, and the parent overwrites local repository config
with one byte-exact SHA-1 or SHA-256 bare-repository file. Before every use the
parent reads and compares those bytes itself; added settings, includes, remotes,
or spelling drift reject without asking Git to report its own configuration.
Git and local cache custody also receive a separately bounded 65,536-node
parent traversal before and after use. That traversal sums regular-file and
symlink logical lengths. Git entries reject above
`min(3 * source-byte-limit + 64 MiB, 1 GiB)`; local publications reject above
`min(source-byte-limit + 64 MiB, 512 MiB)`. These are post-helper acceptance
ceilings for resident cache state, not during-write disk quotas or transferred-
byte measurements: an unconfined helper may still exhaust storage before the
parent can reject its output. After acquiring every Git or local publication
lock, the resolver compares the locked handle with the current lock pathname
using device/inode identity on Unix or volume/file-index identity on Windows;
a pathname replacement while opening or waiting cannot split synchronization
across two lock objects. On Unix each cache entry and lock must be owned by the
resolver's effective user and not group- or other-writable;
canonical ancestry must be root/resolver-owned and cannot be replaceable
through a non-sticky writable directory; unsupported filesystem kinds reject.
This closes ordinary cross-user
ownership/configuration substitution on Unix. It does not prevent the owning
user from replacing a path after an observation, establish Windows
ownership/DACL policy, or
replace native isolation. Git path and symlink preflight rejects Windows
drive/alternate-stream colons, forbidden characters and controls, trailing
dots/spaces, and reserved device names independently of the host path parser.
HTTPS Git commands can select only the observed install-relative
`git-remote-https` invocation entry and its retained canonical target. Other
executable components beneath Git remain outside retained identity. SSH is
forced through its content-observed absolute client with user configuration
disabled, `BatchMode`, zero password
prompts, and strict host-key checking. It still
consults the user's default known-host and key files, so host and credential
custody remain ambient and unsuitable for strict admission. Those conditions
keep the resolver diagnostic-only until native helper confinement, hostile-
process custody, during-write resource ceilings, and opaque-receipt work land.

Public requests admit only HTTPS and SSH transports. The validated request
retains an execution profile distinct from transport-neutral hosted-repository
lineage: an HTTPS request permits only Git's `https` protocol, and either SSH
locator spelling permits only `ssh`. The cache key and exact metadata bind that
profile, so normalized HTTPS and SSH spellings cannot reuse custody established
under the other's authority. Resolved-source observations, human source-audit
output, and legacy diagnostic cache-policy schema v3 retain the selected
profile separately from normalized lineage. HTTP, unauthenticated `git://`, every unselected
protocol, and HTTP redirects remain disabled; `file` exists solely in the
explicit test-only local-repository adapter. This prevents a validated HTTPS
request from silently acquiring SSH/file authority or a redirect-selected
endpoint. It does not yet retain the effective socket endpoint, pin TLS trust,
or confine DNS and network access to the requested host, so complete endpoint
custody still belongs to the native resolver boundary and its receipt.

Parent-owned selected-object-graph authentication supplies real evidence for a
later strict receipt but does not itself make the resolver admissible. Native
isolation, hostile same-user and Windows ACL cache custody, resource ceilings,
explicit SSH trust/credential custody, and the opaque receipt remain open.
