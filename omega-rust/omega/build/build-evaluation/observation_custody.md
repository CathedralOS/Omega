# Build observation custody

This is the Rust implementation note for the
[observation contract](../../../../wiki/spec/build/observations.md).
The [interpreter preparer](../../../psi/semantics/checked-interpreter/src/evaluator/filesystem_preparation.rs)
and providers share a closed catalog of 50 exact filesystem operation identities.
Aliases and platform variants remain distinct. Resolve canonical toolchain
requirements, not user declarations with matching names.

## Preparation and authorization

Evaluate operands once, left to right. Check exact arity and kinds, including
unused ABI operands, before provider entry. Retain an already evaluated typed
prefix when a later operand fails. Operand-resolution snapshots and provider-entry
snapshots are different: evaluating a later aliased operand may change an earlier
mutable one. Retain provider-exit state even on a halt.

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
unknown-handle failures can be replayed without inventing a returned path.

## Operand and handle records

- Immutable bytes and typed non-handle scalars retain operand ordinals; scalar
  kinds distinguish `i32`, `u32`, `i64`, and `u64`.
- Mutable byte and `i64` carriers retain complete resolution, pre-entry, and
  post-entry snapshots, including unused tails and unchanged failure output.
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

Observed regions refer to retained post-entry buffers rather than copying or
charging bytes again. Reads retain exactly the successful result length from
offset zero; EOF is an empty region, failures have none. Directory operations
retain their packed records, including the fixed 320-byte find-entry carrier.
Metadata retains 14 semantic fields and checked `StatLayout` offsets/widths;
validate bounds and non-overlap against the ABI carrier (minimum 144 bytes),
including padding and tails. This is not another public program representation.

Successful returned-path regions exclude terminators and untouched tails.
`Complete` versus `LimitReached` read-link output preserves the provider's exact
length behavior, including exact-fit output; it claims no unseen suffix.

## Deterministic provisions

Provisions below are current compiler defaults, not language limits or host
memory/CPU isolation. Aggregate build work, BuildLog, attempts, live resources,
cells/Text, and result limits are installed by the
[package-review session](../../packages/manager/src/review/candidate/session.rs);
standalone compilation does not thereby acquire a package-closure account.
Check before allocation, copying, or provider mutation;
use checked count conversion. Failures retain partial usage rather than a
success-shaped observation.

| Account | Current default / rule |
| --- | --- |
| One byte input/transfer | 16 MiB. |
| Retained rooted paths | 16 MiB total; 4,096 bytes per path. |
| Retained operand evidence | 256 MiB across immutable/path/returned-path and mutable snapshots. |
| Replay retention | 16 MiB, charged before cloning using canonical upper-bound row weights plus payload. Not encoded length or RSS. |
| Output namespace | 4,096 entries, including the root; 256 MiB logical bytes and maximum object extent. |
| Replay/output paths | 4,096 entries; symlink targets at most 4,096 bytes and charged to retained paths. |
| Immediate duplicate and lock replay | At most 1,024 of each. |
| Package-closure build work | 100,000,000 logical evaluation units. |
| BuildLog | 16 MiB, including `write_line` newlines. |
| Filesystem attempts / live resources | 65,536 attempts / 4,096 live resources. |
| Live cells / Text | 1,048,576 cells / 64 MiB Text. |
| Successful result crossing | 1,048,576 cells / 64 MiB Text, separately bounded. |
| One invocation | 100,000 pure or 10,000,000 granted evaluation units, in both initial evaluation and replay. |

The [filesystem sponsor](../../../psi/semantics/checked-interpreter/src/filesystem_sponsor.rs)
counts names per entry, file content once per object, symlink payload bytes, and
unlinked objects until their final handle closes. Reserve before a host operation,
commit successful changes, and release failed reservations. An operation cannot
fund its own required reservation. Teardown must reconcile the complete tree.

The [build sponsor](../../../psi/semantics/checked-interpreter/src/build_evaluation_sponsor.rs)
shares closure budgets and reconciles initial/replay charges and peaks. Owned
resources reserve before provider entry; borrowed views take no second resource
slot. Cell/Text aliases share a lease released by the last alias. Completion
requires released live leases and a separately admitted result transfer.

These accounts exclude allocator capacity/overhead, non-Text scratch, and RSS.
The synchronous transfer ceiling bounds its temporary buffer; bounded directory
packing limits retained names (each truncated to 255 bytes). An ambient find
snapshot's 16 MiB limit is not package sponsorship. Do not invent a generic
temporary-memory guarantee from these bounds.

BuildLog is a dedicated checked operation, not Console stdout/stderr or a
filesystem class. Whole-build replay compares it separately; do not duplicate
its bytes in the filesystem record. Unavailable worker evidence remains an
explicit unavailable result, not a package-review or admission row.
