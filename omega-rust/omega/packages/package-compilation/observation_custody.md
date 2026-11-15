# Build observation custody

Rust implementation note for the
[observation contract](../../../../wiki/spec/build/observations.md).
The [Build host filesystem](../../build-system/build-settings/src/host/filesystem.rs)
and providers share a fixed catalog of 50 exact filesystem operations. Aliases
and platform variants stay distinct. Resolve canonical toolchain
requirements, not user declarations with matching names.

## Preparation and authorization

Evaluate operands once, left to right. Before the provider call, check exact
operand count and kinds, including unused ABI operands. A prepared mutable
operand keeps its resolved storage place and capacity, not a copy of its
contents; a later aliased operand may change that storage before the call.
Check transfers against the prepared capacity and keep provider errors.

Each authorized path records its operand position, access, Source or Output
root, and relative UTF-8 path. Root resolution grants no access.
Resolve symlinks and nested roots before recording the granted path; never
keep physical host paths. Path-like names, patterns and link targets are
distinct from arbitrary bytes and from authorization.

Namespace changes authorize the parent and leaf without following a leaf
symlink; opens that follow links authorize the resolved target. A hard link
needs write grants for both names. `open_at`/`unlink_at` accept one portable
component. `read_link` returns the link's text, not a right to follow it.
Canonical or final path operations cannot publish a package-absolute host
path; a permitted rooted result must rebuild the same grant as an exact
relative path. An unknown-handle failure keeps its exact result without
inventing a returned path.

## Live operand validation and durable custody

The evaluator checks operand and provider-output consistency during the
call but does not copy that data into the kept summary: read buffers,
argument lists, returned paths, metadata and before/after snapshots are not
build records. The summary keeps the operation, provider, outcome or error,
authorized rooted paths, refusals and logical-handle ownership. Selected
input files and captured output trees hold the bytes the result needs.

The interpreter also checks handles while they are live:

- Descriptor, native-handle and find-handle operands carry logical identities
  and, after typing, a `Resolved`/`Null`/`Unknown` state. Raw host tokens are
  not lasting identities.
- Lifetimes are monotonic. Duplicates and borrowed views stay linked to
  their source; a successful close ends only its own lifetime. Unexpected
  success on an unknown handle traps; a token from another domain is rejected
  before host access. A recycled token cannot revive an old lifetime.
- Virtual descriptor duplicates share position. Source-read grants survive
  duplicates and views; a mutating descriptor call whose kept grant forbids it
  is rejected before host access or sponsor change.

Reads and directory operations write straight into checked prepared buffers;
EOF and provider failures give the operation's ordinary result.
Metadata encoding checks field values against selected `StatLayout`
offsets and widths and the ABI carrier (minimum 144 bytes). Buffer writes
refuse truncation and leave untouched tails alone. No extra read-buffer,
returned-path or before/after copies are kept for a transcript.

## Deterministic provisions

These are current compiler defaults, not language limits or host memory/CPU
isolation. The [package build session](../package-manager/src/build_session.rs)
sets the total build work, BuildLog, attempt, live-resource, cell/Text and
result limits; standalone compilation gets no package-closure account from it.
Check before allocation, copying or provider mutation; convert counts with
checked arithmetic. A failure keeps its partial usage, not a success-shaped
observation.

| Account | Current default or rule |
| --- | --- |
| One byte input/transfer | 16 MiB. |
| Retained rooted paths | 16 MiB total; 4,096 bytes per path. |
| Kept rooted output locations | 256 MiB in total. |
| Output namespace | 4,096 entries, including the root; 256 MiB logical bytes and maximum object extent. |
| Package-closure build work | 100,000,000 logical evaluation units. |
| BuildLog | 16 MiB, including `write_line` newlines. |
| Filesystem attempts / live resources | 65,536 attempts / 4,096 live resources. |
| Live cells / Text | 1,048,576 cells / 64 MiB Text. |
| Successful result returned | 1,048,576 cells / 64 MiB Text, counted separately. |
| One invocation | 100,000 pure or 10,000,000 granted evaluation units, for the single execution. |

The [filesystem sponsor](../../../psi/foundation/filesystem-authority/src/filesystem_sponsor.rs)
counts names per entry, file content once per object, symlink target bytes,
and unlinked objects until their last handle closes. Reserve before a host
operation, commit successes, release failed reservations. An operation cannot
pay for its own required reservation. Teardown must reconcile the whole tree.

The [build sponsor](../../../psi/foundation/evaluation-accounting/src/lib.rs)
shares closure budgets and settles invocation charges and peaks. Owned
resources reserve before the provider runs; borrowed views take no second
slot. Cell/Text aliases share a lease the last alias releases. Completion
needs every live lease released and a separately admitted result transfer.

These accounts exclude allocator capacity/overhead, non-Text scratch and RSS.
The synchronous transfer limit bounds its buffer; directory packing limits
kept names (each cut to 255 bytes). An ambient find snapshot's 16 MiB
limit is not a package budget. These bounds promise nothing general about
temporary memory.

BuildLog is its own checked operation, not Console stdout/stderr or a
filesystem operation. Keep it separately, not copied into filesystem
operation rows. Missing worker evidence stays explicitly unavailable, not a package-review or admission row.
