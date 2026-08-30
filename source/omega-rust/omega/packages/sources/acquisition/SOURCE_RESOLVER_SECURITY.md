# Source resolver security boundary

Status: engineering contract, revised 2026-08-29. This document describes the
security properties that Omega's package-source resolver actually enforces. It
does not define package or language syntax.

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
- bounding captured helper output, brokered network transfer, command duration,
  command count, and accepted source/cache size; and
- recording the endpoint, transport, executable identities, command outcomes,
  source identities, limits, and observed resource use that produced a
  successful result.

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
closed, hooks, checkout filters, and submodules are disabled, and the selected
transport executable is invoked explicitly. User and system Git/SSH
configuration and the invoking environment remain host inputs so ordinary
credential helpers, agents, identity files, known-host policy, and proxies work
normally. These inputs never become package identity or package authority.
Omega does not override the host's `StrictHostKeyChecking` policy. Because
package resolution is noninteractive, a host policy that requires a confirmation
prompt fails normally unless that host is already known or host configuration
selects a noninteractive acceptance policy.
Platform or CI integrations may provide stronger credential isolation, but
that is optional host policy rather than a prerequisite for resolving a
package.

## Successful receipt

Every successful Git resolution issues one opaque `GitSourceReceipt`. It has no
public constructor or decoder. Its canonical identity binds:

- the requested and normalized locator, transport, and requested revision;
- the selected object format, commit, root tree, materialized tree, and
  workspace-member projection;
- the published snapshot path, content identity, entry count, logical bytes,
  depth, and compiler-owned limits;
- the selected Git and transport executable observations;
- every prepared-policy and completed-command observation;
- the requested endpoint route and observed connection outcomes;
- captured-output and network-transfer ceilings and observed counts; and
- the retained cache-storage measurement accepted before publication.

The receipt records what happened. It does not assert that host credentials,
same-user authority, or every operating-system capability was excluded.
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

## Local sources

Local sources skip transport but still pass through bounded snapshot staging.
The resolver captures the requested tree, validates entries and links, publishes
a read-only snapshot under resolver-owned storage, and rechecks the live tree
and snapshot before returning. Ordinary concurrent drift rejects. The snapshot
does not claim protection from a process that already possesses the same user
authority after resolution completes.

## Resource handling

The resolver enforces compiler-owned ceilings on source entries, source bytes,
depth, command count, captured output, brokered transfer, and retained cache
state. Commands have deadlines and platform-appropriate process cleanup. Native
backends may additionally provide filesystem, network, executable, descendant,
CPU, memory, file-size, descriptor, or process-count controls; their exact
dispositions are recorded rather than promoted into universal package
semantics.

These controls substantially bound hostile input, but an ordinary user-mode
package manager cannot promise that the host filesystem will never report disk
exhaustion. A quota-backed cache or stronger host sandbox may add that property
without changing source identity. Disk-full, process-launch, network, and host
credential failures remain ordinary resolution failures.

## Platform hardening

The existing macOS Seatbelt, Linux Landlock/resource-limit, Windows Job Object,
endpoint-broker, executable-custody, and process-lifecycle code is defense in
depth. It narrows what trusted helpers can do and records the controls that were
active. Cross-platform parity is not required before a successful source
receipt may issue, and those mechanisms do not claim to solve hostile same-user
replacement or remove ambient host authority.

## Package and build separation

A dependency declaration chooses source location and revision only. It cannot
embed secrets or choose which secret store, key, agent, or credential broker the
host uses. `build.omg` receives compiler-owned build facets, not resolver
environment, credentials, transport handles, or an ambient filesystem. Package
review and lock admission remain separate decisions over the exact resolved
source and receipt.
