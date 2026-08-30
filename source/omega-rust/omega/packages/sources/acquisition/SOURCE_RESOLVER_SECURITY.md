# Source resolver security boundary

Status: engineering contract, revised 2026-08-30. This document defines the
security properties that Omega's package-source resolver must enforce;
remaining implementation work is tracked in `TASKS_PACKAGE_MANAGER.md`. It does
not define package or language syntax.

## Trust boundary

The resolver is trusted application code running with the invoking user's host
authority. Git, HTTPS, and SSH authentication use the host tooling and
credentials selected for that user. Omega does not claim to remove ambient
authority from an ordinary desktop operating system, protect the user from
another hostile process running as that user, or repair a compromised Git, SSH,
kernel, credential store, or operating system.

Downloaded source is outside the trust boundary. It receives none of the
resolver's process, transport, cache, environment, credential, project, review,
or lock authority. Resolution executes no code from the fetched repository.
Compilation consumes a resolver-published snapshot rather than a live working
tree or helper-produced claim.

## Threats Omega owns

The resolver is responsible for:

- validating source locators and rejecting embedded credentials and secrets;
- preventing source-controlled protocol, redirect, hook, filter, submodule,
  and repository-configuration substitution during a fetch;
- binding the requested locator and revision to the resolved commit, root tree,
  selected workspace member, and materialized content;
- rejecting malformed Git objects, path escapes, unsafe links, unsupported
  filesystem objects, and source trees outside compiler-owned entry, byte, and
  depth ceilings;
- preventing fetched source and `build.omg` from receiving resolver
  credentials, environment, or transport handles;
- bounding captured helper output, command duration, command count, accepted
  source/cache size, and the uniform process resources the selected host
  backend can honestly constrain; and
- recording the primary Git invocation, command outcomes, source identities,
  limits, and observed resource use that produced a successful result.

Secrets never enter source declarations, `omega.lock`, source receipts, review
evidence, or build inputs.

## Transport and credentials

Public requests admit HTTPS, SSH URLs, and SCP-like SSH locators. HTTP,
unauthenticated `git://`, redirects, unselected protocols, hooks, submodules,
and credential-bearing locators reject. The test-only local-repository adapter
is not a production transport.

HTTPS and SSH use the invoking environment's system authentication facilities.
That is an operational input, not package identity and not authority granted by
the dependency. An SSH server receives the ordinary SSH proof of possession;
the private key is not serialized into Omega evidence or exposed to package
source. A missing, rejected, or unusable credential produces an ordinary fetch
failure.

Omega constrains the command surface it owns: protocols and redirects are
closed, while hooks, checkout filters, and submodules are disabled. User and
system Git/SSH configuration and the invoking environment remain host inputs so
ordinary credential helpers, agents, identity files, known-host policy,
transport programs, and proxies work normally. These inputs never become
package identity or package authority.
Omega does not override the host's `StrictHostKeyChecking` policy. Because
package resolution is noninteractive, a host policy that requires a confirmation
prompt fails normally unless that host is already known or host configuration
selects a noninteractive acceptance policy.
Platform or CI integrations may provide stronger credential isolation, but
that is optional host policy rather than a prerequisite for resolving a
package.

## Primary Git selection and consistency

Primary Git selection is an operator input, not package authority. The host
may supply one explicit absolute executable path. Otherwise the resolver
snapshots the invoking environment's `PATH` before it reads package-controlled
input and resolves one executable from that snapshot. The selected path is
made absolute once and every primary Git launch in that resolution uses it;
the resolver never performs a later bare-name lookup from a repository,
workspace, build-output, quarantine, or cache working directory.

Automatic lookup considers only absolute `PATH` directory entries. Empty or
relative entries, implicit current-directory search, and candidates inside a
package-controlled workspace, source, build-output, quarantine, or resolver
cache root reject as selections. On Windows the automatic form selects a
directly executable `git.exe`. A `.bat` or `.cmd` wrapper has nonstandard
argument decoding while package-controlled locator bytes reach Git's argument
vector, so such a wrapper requires a separately specified safe invocation
contract rather than ordinary automatic discovery. An explicit operator path
does not make a package-controlled wrapper safe.

The resolver records the selected absolute path, a bounded content identity,
and checkpointed metadata. Metadata is compared around launches; executable
content is rehashed at defined acquisition and publication checkpoints. A
detected mismatch rejects that resolution as internally inconsistent. These
checks establish neither continuous file immutability nor trust in Git, its
helpers, the invoking user, or the operating system.

Hard-coded platform candidate lists are not a compatibility fallback.
Ownership, Unix mode, set-id state, ACL shape, and a content hash do not
establish host trust and are not source-admission rules. A managed executable
link may resolve normally; neither the link nor its ancestry establishes trust.
Ordinary existence, regular-file, bounded-read, and launchability checks remain
part of constructing a usable invocation. Host or CI policy owns selection and
protection of the Git installation.

The selected path and content identity remain operational provenance in the
receipt for that particular run. They do not enter immutable source identity,
`PackageKey`, or the semantic identity of a lock source. Different selected Git
installations may therefore yield different resolution receipts while
producing the same authenticated source identity. A dependency declaration,
fetched repository, package source, and `build.omg` can neither select nor
alter the primary executable.

The retained source-resolver storage session owns this frozen selection. Its
operator constructors accept an explicit executable and controlled-root
exclusions; the automatic constructor snapshots `PATH` once. Git source and
workspace-member lanes inherit that same selection, so acquisition never
re-reads ambient executable search state after package declarations begin.

## Host-routed network transport

Networked discovery and fetch invoke the selected system Git under the
invoking user's ordinary descendant-execution, filesystem, credential, and
network authority. The universal path does not force an Omega proxy or SSH
command, pre-enumerate the host-selected transport/helper executable graph, or
confine helper-specific state locations. Git configuration may contain
includes, shell commands, helper protocols, platform services, and further
host-selected programs; partially reproducing or brokering that ecosystem
would override host behavior without establishing a stronger operating-system
trust boundary.

This delegation is limited to the transport and authentication chain. It does
not reopen package-controlled execution: the closed protocol surface,
noninteractive invocation, disabled redirects, hooks, replacements, filters,
and submodules, command construction, captured-output and lifetime bounds,
whole-process-tree cleanup, authenticated object graph, quarantine publication,
and immutable snapshot remain compiler owned. Uniform native limits may apply
to the complete child tree without knowing descendant identities. A control
that requires an allowlist of host transport/helper identities or writable
state locations is not part of the universal network path.

Repository initialization and inspection have no ambient transport-helper
requirement and may retain their closed execution and write policy. Operators
that require stronger isolation provide it through their host, VM, container,
or CI policy; Omega does not attest that external policy.

## Successful receipt

Every successful Git resolution issues one opaque `GitSourceReceipt`. It has no
public constructor or decoder. Its canonical identity binds:

- the requested and normalized locator, transport, and requested revision;
- the selected object format, commit, root tree, materialized tree, and
  workspace-member projection;
- the published snapshot path, content identity, entry count, logical bytes,
  depth, and compiler-owned limits;
- the selected primary Git executable and exact compiler-owned invocation;
- every applicable lifecycle/resource policy and completed-command
  observation;
- captured-output ceilings and observed counts; and
- the retained cache-storage measurement accepted before publication.

The receipt records what happened. Its executable coordinate is per-run
provenance rather than immutable source or package identity. It does not assert
that the selected executable, host credentials, same-user authority, or every
operating-system capability was trustworthy or excluded.
Platform hardening rows remain part of the observation so consumers can see
which controls were enforced, unavailable, or inapplicable; an unavailable
optional hardening row does not invalidate an otherwise successful resolution.

Changing a receipt input changes its identity. Persisted bytes cannot mint a
live resolver result or bypass source, object, snapshot, and command
revalidation.

## Git acquisition

The resolver uses a fresh quarantine repository, authenticates the selected
commit and tree through Git's object graph, and materializes source without
checkout filters, hooks, submodules, or package execution. Repository
inspection reconstructs modes, names, object IDs, and payloads from the
selected tree. Workspace selection reads only compiler-authorized declaration
paths and publishes only the authenticated selected member.

The trusted parent independently revalidates executable observations, command
outcomes, object identities, the materialized tree, cache custody, and source
limits before publishing `ResolvedGitSource`. Helper output alone never issues
a result.

Pipeline consistency is compiler-owned even though hostile same-user isolation
is not. Resolution binds one exact commit and tree, validates a private
materialization, and publishes one immutable snapshot. Later phases consume
that snapshot and its retained content identity rather than rereading a mutable
repository. Detected drift or a mismatch between retained and consumed bytes
rejects; the resolver never combines observations from different source
moments. Atomic publication and rechecks protect correctness, reproducibility,
and ordinary concurrent-edit races without claiming that they can defeat a
process already holding the invoking user's authority.

## Local sources

Local sources skip transport but still pass through bounded snapshot staging.
The resolver captures the requested tree, validates entries and links, publishes
a read-only snapshot under resolver-owned storage, and rechecks the live tree
and snapshot before returning. Ordinary concurrent drift rejects. The snapshot
does not claim protection from a process that already possesses the same user
authority after resolution completes.

## Resource handling

The resolver enforces compiler-owned ceilings on source entries, source bytes,
depth, command count, captured output, and retained cache state. Commands have
deadlines and platform-appropriate process cleanup. Native backends may
additionally provide phase-applicable filesystem, descendant, CPU, memory,
file-size, descriptor, or process-count controls; their exact dispositions are
recorded rather than promoted into universal package semantics. Universal
networked resolution claims neither aggregate transport-byte accounting nor
direct endpoint confinement.

These controls substantially bound hostile input, but an ordinary user-mode
package manager cannot promise that the host filesystem will never report disk
exhaustion. A quota-backed cache or stronger host sandbox may add that property
without changing source identity. Disk-full, process-launch, network, and host
credential failures remain ordinary resolution failures.

## Platform hardening

The macOS Seatbelt, Linux Landlock/resource-limit, Windows Job Object, and
process-lifecycle mechanisms are defense in depth when they preserve the
selected phase's contract. Uniform lifecycle and resource controls may narrow
what the complete trusted child tree can do. Network-phase executable or write
allowlists that prevent a host-selected transport helper from running are
behavior overrides and are excluded from the universal path. Cross-platform
parity is not required before a successful source receipt may issue, and these
mechanisms do not claim to solve hostile same-user replacement or remove
ambient host authority.

The operator-selected host Git route likewise does not apply a local-phase
Seatbelt profile when that profile would forbid Git's ordinary launcher or
descendant behavior. The semantic phase, lifecycle, resource ceilings, and
command observation remain intact, while Seatbelt-specific guarantees are
recorded as unavailable.

The universal implementation contains no forced CONNECT route, preselected
transport/helper identity graph, or endpoint/transfer receipt fields. Those
former behavior overrides were deleted rather than retained as empty or
zero-valued observations.

## Package and build separation

A dependency declaration chooses source location and revision only. It cannot
embed secrets or choose which secret store, key, agent, or credential broker the
host uses. `build.omg` receives compiler-owned build facets, not resolver
environment, credentials, transport handles, or an ambient filesystem. Package
review and lock admission remain separate decisions over the exact resolved
source and receipt.
