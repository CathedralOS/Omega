# Build Observation Replay Lanes

This file records implementation-local replay increments. The language and
package policy remain in the repository design briefs.

## Descriptor duplication (summary v39, replay record v20)

The bounded Output grammar accepts one additional exact pair between a fresh
direct-child file's `create` and final `close`:

```text
duplicate(original) -> fresh
close(fresh) -> 0
```

Both calls must use the scoped real provider, return successfully with zero
post-error state, and retain exact descriptor input, duplicated-output lineage,
fresh logical identity, and retirement. The duplicate must be closed
immediately; the original remains the only descriptor accepted by the existing
write, seek, length, permission, timestamp, and sync lanes. Provider-free replay
executes both calls in order and requires exact attempt, identity, namespace,
result, staged-tree, and teardown equality.

One replay may retain at most 1,024 Output duplicates. Duplicate identities are
nonzero and globally distinct from Source descriptors, Output roots, and every
other duplicate. Existing attempt-lane, retained-evidence, extent, staged-tree,
and sponsor ceilings still apply.

Failed duplication or close, delayed retirement, use through the duplicate,
duplicate-of-duplicate graphs, and descriptor reuse remain observed but
non-receipted. Those require later grammar increments rather than inference
from an equivalent final file.

## Descriptor locks (summary v40, replay record v21)

The bounded Output grammar accepts one additional exact pair on the original
fresh direct-child file descriptor between its `create` and final `close`:

```text
lock_file(original, LOCK_EX | LOCK_NB) -> 0, post-error 0
lock_file(original, LOCK_UN) -> 0, post-error 0
```

The acquire and release must be adjacent, use the scoped real provider, retain
the same resolved original descriptor lifetime, and carry exact `i32` operation
scalars `6` then `8`. Provider-free replay executes both calls in order in the
fresh virtual Output namespace and requires exact attempt, scalar, descriptor,
result, post-error, namespace, staged-tree, and teardown equality. The
non-blocking exclusive acquire is the minimal closed lane: it cannot introduce
an unbounded provider wait, while a contended or otherwise failed call remains
observed but non-receipted.

One replay may retain at most 1,024 successful lock/unlock pairs. Locks through
duplicates, shared or blocking acquisition, delayed release, failed outcomes,
contention histories, and Win32 ranged locks remain outside this increment.
