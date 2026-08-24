# Source resolver security boundary

Status: engineering contract, 2026-08-24. This document refines
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
lock.

## Current engineering delta

Git resolution now validates exact tree/blob objects and materializes them into
a staged, read-only, atomically published snapshot without invoking checkout,
filters, hooks, submodules, or package code. It re-hashes the published source
and revalidates it before reuse. This establishes the object-to-snapshot shape,
not the complete production boundary.

Local sources now use a bounded in-memory capture, content-addressed staging,
read-only atomic publication, and revalidation before reuse. The resolver
rejects source/cache overlap and ordinary mutation observed between capture and
publication; compilation-facing diagnostics expose the published snapshot, not
the live tree. Empty directories participate in identity while directory
permissions normalize to the canonical snapshot policy. Local package capture
excludes only repository metadata and the compiler-reserved root `build/`
output directory; it does not trust package-authored ignore files. Nested
`build` directories remain ordinary source, and a symlink into excluded output
rejects. Resolver-owned materializations are checked under an exact-tree policy,
so immutable Git snapshots still preserve every selected tree entry.

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

Git subprocesses now receive null stdin, concurrent bounded stdout/stderr
capture, and a deadline. Fetch requests only the selected revision at depth one
and disables automatic maintenance and garbage collection, so selecting one
package revision does not traverse its unrelated reachable history. Each
subprocess starts in a fresh Unix process group or Windows Job Object, and every
completion or rejection path terminates that container before returning;
ordinary helper and SSH descendants therefore cannot survive the resolver
command or keep inherited capture pipes open. Overflow and timeout reject
explicitly, including for blob reads. This process container floor is not an OS
sandbox: a hostile Unix descendant may deliberately escape into another
session. Depth-one fetch limits history amplification but does not enforce a
transferred-byte or object-store quota. Fetch and materialization still run in
the parent process without filesystem/network confinement or CPU, memory,
process-count, and transfer ceilings. A deliberately hostile same-user process
can race cooperative locks and validation, including the local before/after
observation.
SSH retains an external client and credential/configuration surface. Those
conditions keep the resolver diagnostic-only until native helper confinement,
hostile-process custody, remaining resource ceilings, and opaque-receipt work
land.
