# Build observation custody

This is the Rust implementation note for the
[observation contract](../../../../wiki/spec/build/observations.md).
The [interpreter preparer](../../../psi/semantics/checked-interpreter/src/interpreter/evaluator/filesystem_preparation.rs)
and providers share a closed catalog of 50 exact filesystem operation identities.
Aliases and platform variants remain distinct. Resolve canonical toolchain
requirements, not user declarations with matching names.

## Preparation and authorization

Evaluate operands once, left to right. Check exact arity and kinds, including
unused ABI operands, before provider entry. Prepared mutable carriers retain their
resolved storage places and capacity, not copies of their contents. Later aliased
operands may change that storage before provider entry. Check transfers against
the prepared capacity and preserve provider errors.

Each authorized path carries its exact operand ordinal, access, Source/Output
root identity, and relative UTF-8 path. Root resolution does not itself grant
access. Resolve symlinks and nested roots before recording the granted path;
never retain physical host paths. Path-like names, patterns, and link targets
are distinct from both arbitrary byte payload and authorization.

Namespace mutation authorizes the parent and leaf without following a leaf
symlink; target-following opens authorize the resolved target. Hard links require
write grants for both names. `open_at`/`unlink_at` accept one portable component.
`read_link` returns inert spelling, not authority to follow it. Canonical/final
path operations cannot publish a package-absolute host path; a permitted rooted
result must reconstruct the same grant as a lossless relative path. Exact
unknown-handle failures retain their result without inventing a returned path.

## Live operand validation and durable custody

The evaluator validates operand and provider-output consistency during the
call. Build evaluation does not copy those payloads into its durable summary:
read buffers, argument-value lists, returned paths, metadata carriers, and
mutable pre/post snapshots are not build records. The summary keeps operation
identity, provider, outcome/error, authorized rooted paths, refusals, and
logical-handle custody. Selected input inventories and captured output trees
own the actual file bytes needed by the build result.

The interpreter also validates live handle custody:

- Descriptor, native-handle, and find-handle operands carry logical identities
  and `Resolved`/`Null`/`Unknown` disposition after typing. Raw host tokens are not
  durable identity.
- Lifetimes are monotonic. Duplicates and borrowed views retain source joins;
  successful close invalidates only the corresponding lifetime. Unexpected
  success for an unknown handle traps, and cross-domain tokens reject before
  host access. Token recycling cannot revive an old logical lifetime.
- Virtual descriptor duplicates share position. Source-read grants survive
  duplication/views; mutating descriptor calls reject before host access or
  sponsor mutation when the retained grant does not permit them.

Reads and directory operations write directly into checked prepared buffers;
EOF and provider failures preserve the operation's ordinary result behavior.
Metadata encoding checks semantic field values against selected `StatLayout`
offsets and widths and the ABI carrier (minimum 144 bytes). Buffer writes reject
truncation and preserve untouched tails. No additional read-buffer, returned-path,
or mutable pre/post copies are retained for an operation transcript.

## Deterministic provisions

Provisions below are current compiler defaults, not language limits or host
memory/CPU isolation. Aggregate build work, BuildLog, attempts, live resources,
cells/Text, and result limits are installed by the
[package-review session](../../packages/manager/src/review/candidate/compilation/session.rs);
standalone compilation does not thereby acquire a package-closure account.
Check before allocation, copying, or provider mutation;
use checked count conversion. Failures retain partial usage rather than a
success-shaped observation.

| Account | Current default / rule |
| --- | --- |
| One byte input/transfer | 16 MiB. |
| Retained rooted paths | 16 MiB total; 4,096 bytes per path. |
| Retained rooted output coordinates | 256 MiB aggregate custody ceiling. |
| Output namespace | 4,096 entries, including the root; 256 MiB logical bytes and maximum object extent. |
| Package-closure build work | 100,000,000 logical evaluation units. |
| BuildLog | 16 MiB, including `write_line` newlines. |
| Filesystem attempts / live resources | 65,536 attempts / 4,096 live resources. |
| Live cells / Text | 1,048,576 cells / 64 MiB Text. |
| Successful result crossing | 1,048,576 cells / 64 MiB Text, separately bounded. |
| One invocation | 100,000 pure or 10,000,000 granted evaluation units, for the single execution. |

The [filesystem sponsor](../../../psi/semantics/checked-interpreter/src/filesystem_sponsor.rs)
counts names per entry, file content once per object, symlink payload bytes, and
unlinked objects until their final handle closes. Reserve before a host operation,
commit successful changes, and release failed reservations. An operation cannot
fund its own required reservation. Teardown must reconcile the complete tree.

The [build sponsor](../../../psi/semantics/checked-interpreter/src/build_evaluation_sponsor.rs)
shares closure budgets and reconciles invocation charges and peaks. Owned
resources reserve before provider entry; borrowed views take no second resource
slot. Cell/Text aliases share a lease released by the last alias. Completion
requires released live leases and a separately admitted result transfer.

These accounts exclude allocator capacity/overhead, non-Text scratch, and RSS.
The synchronous transfer ceiling bounds its temporary buffer; bounded directory
packing limits retained names (each truncated to 255 bytes). An ambient find
snapshot's 16 MiB limit is not package sponsorship. Do not invent a generic
temporary-memory guarantee from these bounds.

BuildLog is a dedicated checked operation, not Console stdout/stderr or a
filesystem operation. Retain it separately; do not duplicate its bytes in the
filesystem operation rows. Unavailable worker evidence remains an
explicit unavailable result, not a package-review or admission row.
