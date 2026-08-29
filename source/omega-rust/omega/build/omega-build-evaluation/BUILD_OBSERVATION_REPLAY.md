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

## Empty Output directory (summary v41, replay record v22)

The bounded Output grammar accepts one exact successful Unix `create_dir`
attempt after the Source-input prefix:

```text
create_dir(Output/direct-child, 493 /* 0o755 */) -> 0, post-error 0
```

The path must be one fresh canonical direct child of the compiler-issued
Output root. The attempt must use the scoped real provider and retain one exact
rooted-path resolution and matching write authorization. Provider-free replay
executes the same attempt in the virtual Output namespace and requires exact
attempt equality and a final staged tree containing only that empty directory.

This increment admits one directory attempt and at most 4,096 retained path
bytes. Nested directories, `create_dir_name`, multiple or mixed file/directory
outputs, alternate modes or providers, failed results, nonzero post-error
state, and any subsequent child or namespace operation remain outside the
grammar.

## Empty Output directory tree (summary v42, replay record v23)

The directory lane now accepts a nonempty ordered sequence of successful
`create_dir` attempts under one exact Output root. Paths are canonical and
distinct; every nested path must follow its exact parent, so replay never
infers an unobserved directory. Provider-free execution reproduces the complete
attempt sequence and requires exact final namespace and staged-tree equality.

One replay retains at most 4,096 directories, 4,096 bytes per path, and 16 MiB
of aggregate path spelling. Files and directories still do not mix. Missing or
late parents, duplicate paths, root changes, generated-source handoffs,
alternate operations, and failed outcomes remain observed but non-receipted.

## Mixed Output tree (summary v43, replay record v24)

The bounded Output grammar now retains one ordered sequence of directory
creates and complete regular-file chains. A file chain uses the already
admitted `create`/operation*/`close` grammar and remains contiguous. Every
nested directory or file must follow its exact parent directory; replay never
invents a parent from the final tree. Exact path collisions, a file used as a
parent, missing or late parents, root changes, and descriptor reuse reject.

Generated-source handoffs may name nested regular files and remain bound to
their exact post-close filesystem-attempt ordinal. Provider-free replay
reproduces the complete authored sequence and requires exact attempts, mixed
virtual namespace, teardown, handoff sequence, and sponsored staged-tree
equality. One tree retains at most 4,096 entries, 4,096 bytes per path, and
16 MiB of aggregate path spelling; existing operation, descriptor, extent, and
content ceilings still apply. Symlinks, hard links, implicit parents,
interleaved file chains, other namespace operations, and failed outcomes remain
outside this increment.
