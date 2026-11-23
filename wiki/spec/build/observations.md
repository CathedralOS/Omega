# Build inputs, observations, and outputs

[Execution](execution.md) owns build admission and generated-source handoff;
[package acceptance](../packages/acceptance.md) owns the decisions retained in
`omega.lock`. Observations describe one execution, not a second admission policy.

## One admitted execution

A build uses the dependency resolutions and accepted build requirements recorded
in its lock. A mismatch rejects compilation; compilation cannot silently update
the lock or invent acceptance. Install/update owns changes to recorded decisions.

The root build selects the filesystem implementation and delegates scoped access.
Every downstream delegation may preserve or reduce that access, never enlarge
it. Selecting a dependency or importing a filesystem API supplies no authority.
If the root supplies a virtual filesystem, descendants use that filesystem;
unsupported operations reject rather than falling back to a real host provider.
The executor must possess the resources the root requests: a root declaration
cannot manufacture host authority either.

Execute each admitted build activation once. Check operations against its supplied
capabilities, account for resources, and validate completed outputs directly.
There is no operation-record playback, second execution for comparison, replay
codec, or replay-based release classification. Build acceptance does not promise
reproducibility or authenticate external input contents.

## Retained execution facts

Retain the selected activation, canonical input identity, attempted operations
and their actual results where needed for audit/accounting, explicit source
handoffs, completed output obligations, and bounded diagnostics. Failed attempts
remain failures; an absent observation cannot manufacture success or authority.
These facts neither authorize another activation nor prove a shipped program's
runtime behavior.

Captured inputs remain immutable for the activation. Input identity binds their
exact bytes and admitted metadata, not a host pathname that can later change.
The [scoped execution contract](scoped_execution.md) defines input isolation,
delegation, and output reuse within a build. Retaining completed output bytes for
publication or reuse does not rerun the build.

## Complete output custody

After evaluator/provider teardown, inspect the quiescent sponsored Output tree
before deleting the session. Retain its complete canonical tree: sorted portable
UTF-8 relative paths, entry kinds, empty directories, ordinary/executable mode,
file lengths/content commitments, and self-contained relative symlink targets.
An empty tree has an explicit commitment.

Exclude host roots, timestamps, ownership, ACLs, ambient permissions, and inode or
hard-link topology from canonical tree identity. Validate namespace kinds,
extents, and sponsored usage; unknown objects, external links, inconsistent
custody, and exceeded provisions reject. Bind topology-independent unique-content
accounting alongside the tree commitment.

Materialization consumes retained complete content into an existing empty
destination and re-inspects exact paths, kinds, modes, targets, and bytes.
A Build publishes generated source by passing its logical filename and exact
UTF-8 bytes to [`Build::include_source`](execution.md#generated-source-boundary).
The activation accounts for both the name and the contents. Each logical name
can be published once. Retain the publication with its generating context so
the compiler consumes those bytes even if an exported file has the same name
and different contents. Exporting a file with an `.omg` suffix is independent
of publishing source.

Older stored observations name a closed output file and the completed
filesystem attempt at which the Build selected it. Recover those bytes from the
retained tree under the rules of that format. Paths must be unique, their
ordinals nondecreasing, and each file closed before selection. Equal ordinals
are legal. The original source-extension, reserved-name and non-executable
regular-file checks apply to those observations.

Scoped grants and deterministic resource accounts alone do not exclude hostile
same-user processes racing disk-backed storage. Enforce the input/output isolation
contract or require an explicit adequate executor isolation premise. Neither a
lock nor a retained observation substitutes for that enforcement.
