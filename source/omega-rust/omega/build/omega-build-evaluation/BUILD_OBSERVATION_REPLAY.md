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
