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
- retaining the exact requested lineage, selected commit/tree/content,
  immutable snapshot identity, and concrete limits that produced a successful
  source result.

Secrets never enter source declarations, `omega.lock`, resolved-source
custody, review evidence, or build inputs.

A resolver control belongs in the universal contract only when it validates
package-derived material, directly enforces a concrete resource/lifecycle
property, or prevents package-controlled input from influencing a decision
Omega makes with operator authority. A control whose only product is a
self-issued statement about the host executor does not qualify.

## Transport and credentials

Public requests admit HTTPS, SSH URLs, and SCP-like SSH locators. Host
`insteadOf` configuration may route between HTTPS and SSH because both belong
to the admitted production transport class. HTTP, unauthenticated `git://`,
`file`, `ext`, redirects outside the requested repository policy, hooks,
submodules, and credential-bearing locators reject. The test-only
local-repository adapter is not a production transport.

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

Hard-coded platform candidate lists are not a compatibility fallback.
Ownership, Unix mode, set-id state, ACL shape, and a content hash do not
establish host trust and are not source-admission rules. A managed executable
link may resolve normally; neither the link nor its ancestry establishes trust.
Ordinary existence, regular-file, and launchability checks remain
part of constructing a usable invocation. Host or CI policy owns selection and
protection of the Git installation.

The selected absolute path is an invocation fact, not execution attestation.
It does not enter immutable source identity, `PackageKey`, or lock-source
meaning. A dependency declaration, fetched repository, package source, and
`build.omg` can neither select nor alter the primary executable. Executable
content hashes, metadata drift checkpoints, and canonical executable
provenance are not source-admission inputs: the verified source result already
captures every executor difference relevant to source custody.

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

Host configuration may rewrite an authored HTTPS locator to SSH or an authored
SSH locator to HTTPS. That changes the host route, not the package key. The
authored canonical lineage remains the identity input: known GitHub/GitLab
adapters may normalize proven repository namespaces, while generic Git lineage
retains transport, user, host, port, path case, and suffix distinctions until
an adapter establishes equivalence. Effective routing is never used to merge
otherwise distinct lineages.

This delegation is limited to the transport and credential chain. It does
not reopen package-controlled execution: the closed protocol surface,
noninteractive invocation, disabled redirects, hooks, replacements, filters,
and submodules, command construction, captured-output and lifetime bounds,
platform-owned process-container cleanup, verified object graph, quarantine
publication, and immutable snapshot remain compiler owned. Uniform native limits may apply
to the complete child tree without knowing descendant identities. A control
that requires an allowlist of host transport/helper identities or writable
state locations is not part of the universal network path.

Repository initialization and inspection use the same universal execution
boundary. Omega does not impose executable or filesystem sandbox policy on the
operator's Git. Operators that require stronger isolation provide it through
their host, VM, container, or CI policy; Omega does not attest that external
policy.

## Successful source custody

A successful Git resolution retains the facts downstream consumers actually
use: authored canonical lineage and requested revision; selected object format,
commit and root tree; materialized content identity and workspace-member
projection; published immutable snapshot identity; entry, byte, and depth
measurements; and the compiler-owned limits applied to them.

Those facts belong directly to the resolved source and eventual
`PackageInstance`. There is no canonical `GitSourceReceipt` over the process
that fetched them. Selected executables, prepared commands, completion order,
platform-hardening dispositions, and operational telemetry cannot mint source
custody or change source identity. Persisted bytes cannot mint a live resolver
result or bypass source, object, snapshot, command-success, and concrete-bound
revalidation.

## Git acquisition

The resolver uses a fresh quarantine repository, verifies the selected commit
and tree through Git's content-addressed object graph, and materializes source
without checkout filters, hooks, submodules, or package execution. Repository
inspection reconstructs modes, names, object IDs, and payloads from the
selected tree. Workspace selection reads only compiler-authorized declaration
paths and publishes only the verified selected member.

“Verified” here means that the materialized bytes and graph rejoin the resolved
Git object identities. It does not authenticate a repository owner or package
publisher, and it adds no endpoint claim beyond the invoking host's ordinary
Git/HTTPS/SSH policy.

The trusted parent independently revalidates command success, object
identities, the materialized tree, cache custody, and source limits before
publishing `ResolvedGitSource`. Helper output alone never issues a result.

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
deadlines and platform-appropriate process-container cleanup. Native backends
may additionally enforce honest CPU, memory, file-size, descriptor, or
process-count limits. These controls are execution mechanisms, not canonical
evidence rows. Universal networked resolution claims neither aggregate
transport-byte accounting nor direct endpoint confinement.

These controls bound retained source inputs and the concrete resources enforced
by the selected backend, but an ordinary user-mode package manager cannot
promise that the host filesystem will never report disk exhaustion. A
quota-backed cache or stronger host sandbox may add that property without
changing source identity. Disk-full, process-launch, network, and host
credential failures remain ordinary resolution failures.

## Platform execution mechanisms

Platform mechanisms remain only where they directly implement a concrete
resource or lifecycle property. Windows Job Objects may enforce limits and
kill-on-close process-tree cleanup; Unix resource limits may enforce their
actual kernel bounds. Those mechanisms publish no execution-trust or
confinement guarantee.

Seatbelt/Landlock executable and filesystem policies, closed native-guarantee
matrices, and canonical platform-hardening observations are not part of the
universal resolver. They cannot close hostile same-user replacement and may
override valid operator-selected Git behavior. Stronger host isolation remains
an optional deployment concern outside source admission. The universal
implementation likewise contains no forced CONNECT route, preselected
transport/helper identity graph, or endpoint/transfer receipt fields.

## Package and build separation

A dependency declaration chooses source location and revision only. It cannot
embed secrets or choose which secret store, key, agent, or credential broker the
host uses. `build.omg` receives compiler-owned build facets, not resolver
environment, credentials, transport handles, or an ambient filesystem. Package
review and lock admission remain separate decisions over the exact resolved
source custody.
