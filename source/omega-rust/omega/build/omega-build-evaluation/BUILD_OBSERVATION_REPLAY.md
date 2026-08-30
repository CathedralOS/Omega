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

## Symbolic-link Output entry (summary v44, replay record v25)

The ordered Output-tree grammar now accepts an exact successful `symlink`
attempt. Replay retains operation tag 20, the verbatim target spelling in the
operand-0 path-like lane, the compiler-rooted operand-1 link path and matching
write authorization, scalar result zero, and zero post-error state. The link
path must follow its exact parent directory and cannot collide with a prior
directory, regular file, or symbolic link.

Provider-free replay recreates the exact virtual link mapping and requires the
complete operation sequence, final namespace, teardown, and sponsored
staged-tree equality. Staged-tree issuance independently requires a canonical
self-contained relative UTF-8 target that cannot escape Output. Absolute,
malformed, NUL-bearing, escaping, missing-parent, colliding, failed, and
alternate link operations remain outside this increment. A target retains at
most 4,096 bytes; link-path and target spelling share the existing 16 MiB
aggregate ceiling.

## Hard-link Output entry (summary v45, replay record v26)

The ordered Output-tree grammar now accepts exact successful hard links: the
portable operation at tag 19 and Win32 `CreateHardLink` at tag 27. Both names
must be canonical paths under the same Output root, and replay retains matching
write authorization for the existing and new names. The existing name must be
an earlier regular-file or hard-link entry; the new name obeys the ordinary
parent-before-child and collision rules. Provider-specific operand order,
successful result spelling, and zero post-error state remain exact.

Provider-free replay recreates each link in authored order and requires exact
attempt, namespace, teardown, and sponsored staged-tree equality. The staged
output commitment intentionally omits inode identity and hard-link topology:
each linked name is committed as ordinary regular-file content, so equivalent
hard-linked and duplicated-file trees have the same staged representation.
Missing, late, directory, or symbolic-link sources, cross-root names,
insufficient authorization, collisions, failures, and alternate operations
remain non-receipted.

## Source read-link event (summary v46, replay record v27)

The ordered Source-input grammar now accepts exact successful Source-rooted
`read_link` events at tag 21, interspersed with the existing Source read and
metadata events. Each event retains the authored rooted symlink name, its
separately authorized no-follow target, requested count, scalar result,
post-error state, complete mutable resolution/pre/post carrier, and the exact
meaningful returned bytes. The returned row distinguishes a complete target
from a capacity-limited prefix; a limited prefix is retained as itself and no
unobserved suffix is inferred.

Provider-free replay writes back the exact retained carrier and requires the
same result, evidence, event order, build result, and empty or reconstructed
Output tree. Returned target bytes remain inert data: using them as a path
still requires a new checked resolution through a compiler-issued root.
Failures, Output-rooted reads, malformed carriers, changed tails, inconsistent
counts or completeness, alternate path-result kinds, and hidden authority
remain non-receipted.

## Output-only tree (summary v47, replay record v28)

The ordered replay grammar now accepts a nonempty exact Output tree beginning
at filesystem attempt zero. A build that generates constant output therefore
does not need a fabricated Source filesystem event: its empty Source-event
prefix is replayed vacuously. Canonical package Source metadata and compiler
custody remain mandatory and are revalidated independently of that event
prefix.

Provider-free execution begins with the first Output attempt. Generated-source
handoffs retain their exact ordinals from zero, and the existing complete
directory, file, symbolic-link, and hard-link tree grammar remains unchanged.
An empty attempt stream, malformed pre-Output attempt, unexplained physical
Output, or changed canonical Source identity remains non-receipted.

## Source directory enumeration (summary v48, replay record v29)

The ordered Source grammar now accepts one closed directory-enumeration chain:
an exact flags-zero Source open, one or more successful `read_dir` calls at tag
23, and exact successful descriptor retirement. Every read retains its transfer
count and result, post-error state, exact directory-record region, complete
byte-carrier resolution/pre/post states, and complete mutable cursor
resolution/pre/post states. Multiple calls retain authored order; a caller that
observes end-of-directory retains that zero-length call rather than an inferred
completion claim.

Provider-free replay restores both mutable carriers without consulting a live
directory. Packed directory records remain target-specific inert bytes: replay
does not parse names, infer unseen entries, or grant authority to use a returned
name. Any later relative open or mutation requires its own exact checked replay
lane. Failed reads, leaked descriptors, malformed carrier tails, changed
counts, reordered calls, or incomplete chains remain non-receipted.

The Windows find-enumeration family is not part of this increment. Its current
plain-byte `directory/*` input contains the physical Source-root spelling, so
retaining it exactly would make replay location-dependent while ignoring it
would weaken prepared-input equality. It remains non-receipted until the
root-aware compiler-owned Build path facet can retain a Source root plus
relative pattern coordinate.

## Absent Output remove (summary v49, replay record v30)

The first exact failure lane admits a nonempty sequence containing only
authorized `remove` attempts at tag 9 against canonical compiler-rooted Output
paths. Every attempt must use the scoped real provider and return scalar `-1`
with post-error state `2` (`not found`). It retains one exact rooted operand and
the matching Output write authorization; no refused, raw path-like, handle,
mutable-carrier, metadata, or returned-path lane may appear.

Provider-free replay executes each remove against a fresh virtual Output
namespace, requires the same failure, and verifies that teardown still has no
Output entries or generated-source handoffs. A replay retains at most 4,096
such attempts and 16 MiB of aggregate relative-path spelling. An optional exact
Source prefix remains permitted, but failure-only operations cannot be mixed
with successful Output mutations in this first rung. Other error codes,
refused or unrooted paths, mixed roots, successful removes, and mixed
mutation/failure lifecycles remain observed but non-receipted.

## Unknown-descriptor close (summary v50, replay record v31)

The failed-handle grammar admits an optional exact Source-input prefix followed
by exactly one `close` attempt at tag 8. The attempt must use the scoped real
provider, return scalar `-1` with post-error state `9` (`bad descriptor`), and
retain one operand-zero logical input classified as `Descriptor/Unknown`. Every
other lane is empty: in particular, no raw provider descriptor, path, mutable
carrier, output identity, retired handle, refusal, diagnostic string, or
generated-source handoff is retained.

Provider-free replay executes the close against the fresh virtual handle table
and requires the same failed result, exact attempt record, empty namespace, and
clean teardown. Because the exact operation cannot mutate Output and replay
covers the complete attempted sequence, an initial matching run receives an
empty staged-output commitment and the complete-operation replay verdict.
Source-only replay remains partial; this rule does not infer an empty Output
commitment merely from the absence of Output paths.

Null or resolved inputs, alternate handle kinds or providers, successful
closes, other errors, retired identities, side lanes, repeated closes, and any
mixture with Output mutation or another failed-operation lane remain
non-receipted.

## Operand-free unknown-descriptor failures (summary v51, replay record v32)

The failed-handle grammar generalizes the tag-8 close rung to the complete
operand-free descriptor family: `close` at tag 8, `sync` at tag 43,
`sync_data` at tag 44, and `duplicate` at tag 45. The optional exact Source
prefix and single-operation limit remain unchanged. Each selected operation
must use the scoped real provider, return scalar `-1` with post-error state `9`
(`bad descriptor`), and retain exactly one operand-zero
`Descriptor/Unknown` logical input. All other evidence lanes remain empty.

Provider-free replay executes the selected operation against a fresh virtual
descriptor table and requires exact attempt, result, empty namespace, and
teardown equality. The complete no-effect sequence receives an explicit empty
staged-output commitment on the initial run and after record recovery. The
operation tag is part of the record: one family member cannot replay as
another.

Descriptor operations carrying scalars, immutable bytes, or mutable carriers;
native and find handles; repeated failures; alternate errors; and mixtures
with successful Output mutation or another failure lane remain
non-receipted.

## Closed replay verdict (summary v52, replay record v33)

The observation summary replaces two independently representable replay flags
with one version-1 closed verdict: `NotReplayed`, `SourceInputsOnly`, or
`Complete`. There is no state that claims complete operation replay without
source-input replay. `SourceInputsOnly` claims only provider-free execution
against the retained Source-input record with exact build-result and
observation equality; it claims no Output reconstruction, generated-source
handoff, teardown, or staged-output custody.

`Complete` is issued only when the same replay also matches the complete
attempted operation sequence and generated-source handoffs, reconstructs the
virtual Output namespace, closes all replay lifecycle state, and owns the
matching staged-output commitment or sponsored custody. The compiler fails
closed instead of issuing `Complete` without source replay or staged-output
custody. Package observation identity binds the verdict schema and disposition
alongside the exact attempts, handoffs, and tree commitment.

This is compiler-issued observation evidence, not package admission authority
and not proof that a human or LLM performed an audit. Host CPU and RSS controls
may protect CI availability, but they do not strengthen this evidence or turn
review into admission authority.

## Unknown-descriptor seek (summary v53, replay record v34)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one tag-10 `seek` on an unknown descriptor. The operation
must use the scoped real provider, return scalar `-1` with post-error state `9`
(`bad descriptor`), retain operand zero as `Descriptor/Unknown`, and retain the
authored operand-one `i64` offset and operand-two `i32` origin exactly. Every
path, byte, mutable carrier, output, retirement, refusal, and generated-source
handoff lane is empty.

Provider-free replay executes the same offset and origin against a fresh
virtual descriptor table and requires exact result, attempt, empty namespace,
and teardown equality. The complete no-effect sequence receives explicit empty
staged-output custody on initial evaluation and record recovery. Wrong scalar
types or ordinals, alternate handles or errors, repeated seeks, and mixtures
with Output mutation or another failure remain non-receipted.

## Unknown-descriptor write operations (summary v54, replay record v35)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one write-gated scalar operation on an unknown descriptor:
tag-17 `set_file_permissions` with operand-one `u32` mode, tag-41 `set_len`
with operand-one `i64` length, tag-46 `lock_file` with operand-one `i32`
operation, or tag-49 `change_file_owner` with operand-one `i32` user and
operand-two `i32` group. Each row fixes scoped-real provider, scalar `-1`,
post-error `9`, and operand-zero `Descriptor/Unknown`; every other evidence
lane and generated-source handoff is empty.

The real evaluator rejects the missing descriptor at the compiler write-grant
lookup before host mutation, so this row carries neither authorization nor a
grant refusal. Provider-free replay executes the exact selected tag and scalar
values in a fresh virtual descriptor table and requires exact result, attempt,
empty namespace, and teardown equality before issuing empty staged-output
custody. Known or null handles, changed scalar types or ordinals, alternate
errors, repetitions, and mixed lifecycles remain non-receipted.

## Unknown-descriptor set-file-times (summary v55, replay record v36)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one tag-42 `set_file_times` call on an unknown descriptor.
The row fixes the scoped-real provider, scalar `-1`, post-error `9`, and
operand-zero `Descriptor/Unknown`. It retains operand one's complete authored
mutable byte carrier at resolution time and as equal provider pre/post states;
the carrier must contain at least the 32-byte timespec pair consumed by the
evaluator. Every other evidence lane and generated-source handoff is empty.

The real evaluator rejects the missing descriptor at write-grant lookup before
host mutation and leaves the carrier unchanged. Provider-free replay restores
the exact retained carrier, reproduces the same failure in a fresh virtual
descriptor table, and requires exact attempt, empty namespace, and teardown
equality before issuing empty staged-output custody. Short or changed carriers,
alternate ordinals, handles, errors, repetitions, and mixed lifecycles remain
non-receipted.

## Unknown-descriptor reads (summary v56, replay record v37)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one read from an unknown descriptor: tag-4 `read` with an
operand-two `u64` count, or tag-6 `read_at` with that count and an operand-three
`i64` offset. The row fixes the scoped-real provider, scalar `-1`, post-error
`9`, and operand-zero `Descriptor/Unknown`. Operand one's complete mutable byte
carrier is retained as equal resolution and provider pre/post states, and the
authored count may not exceed its capacity. No observed-byte region exists for
the failed transfer; every other lane and generated-source handoff is empty.

The real evaluator rejects through its compiler-owned descriptor table before
performing a host read or changing the carrier. Provider-free replay restores
the exact carrier and executes the selected read against a fresh virtual table,
requiring exact attempt, empty namespace, and teardown equality before issuing
empty staged-output custody. Changed counts, offsets, carrier bytes or ordinals,
over-capacity transfers, alternate handles or errors, repetitions, and mixed
lifecycles remain non-receipted.

## Unknown-descriptor writes (summary v57, replay record v38)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one immutable-payload write to an unknown descriptor:
tag-5 `write`, or tag-7 `write_at` with operand-two `i64` offset. The row fixes
the scoped-real provider, scalar `-1`, post-error `9`, and operand-zero
`Descriptor/Unknown`, and retains operand one's complete authored payload.
Every other evidence lane and generated-source handoff is empty.

The real evaluator rejects at compiler-owned descriptor write-grant lookup
before sponsor accounting or host mutation. Provider-free replay restores the
exact payload and executes the selected write against a fresh virtual table,
requiring exact attempt, empty namespace, and teardown equality before issuing
empty staged-output custody. Changed payloads, offsets or ordinals, alternate
handles or errors, repetitions, and mixed lifecycles remain non-receipted.

## Unknown-descriptor file metadata (summary v58, replay record v39)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one tag-39 `read_file_metadata` call on an unknown
descriptor. The row fixes the scoped-real provider, scalar `-1`, post-error
`9`, and operand-zero `Descriptor/Unknown`. It retains operand one's complete
authored mutable carrier as equal resolution and provider pre/post states.
The carrier must satisfy the preparer's 144-byte metadata-ABI minimum. There is
no metadata observation; every other evidence lane and generated-source
handoff is empty.

The real evaluator rejects at its compiler-owned descriptor table before host
metadata access and leaves the carrier unchanged. Provider-free replay restores
the exact carrier and executes the call against a fresh virtual table,
requiring exact attempt, empty namespace, and teardown equality before issuing
empty staged-output custody. Changed carrier bytes or ordinals, metadata rows,
alternate handles or errors, repetitions, and mixed lifecycles remain
non-receipted.

## Unknown-descriptor modeled handle bridge (summary v59, replay record v40)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one tag-30 `get_osfhandle` call on an unknown descriptor.
The row fixes the scoped-real provider, scalar `-2`, unchanged post-error `0`,
and operand-zero `Descriptor/Unknown`. Every scalar, byte, mutable, path,
metadata, refusal, logical-output, retirement, and generated-source handoff
lane is empty.

Both evaluators answer this call from their compiler-owned synthetic descriptor
tables. Provider-free replay therefore requires the exact modeled result,
attempt, empty namespace, and teardown before issuing empty staged-output
custody. This receipts only Omega's descriptor-to-handle model; it claims no
custody of a native operating-system handle and no Windows security property.

## Unknown-native-handle close (summary v60, replay record v41)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one tag-29 `close_handle` call on an unknown native handle.
The row fixes the scoped-real provider, scalar `0`, post-error `6`, and
operand-zero `Native/Unknown`. Every scalar, byte, mutable, path, metadata,
refusal, output, retirement, and generated-source handoff lane is empty.

Both evaluators reject through their compiler-owned synthetic handle tables.
Provider-free replay requires the exact modeled result, attempt, empty
namespace, and teardown before issuing empty staged-output custody. This
receipts only Omega's synthetic-handle model; it claims neither custody of an
operating-system handle nor a Windows security property.

## Unknown-native-handle final path (summary v61, replay record v42)

The failed-handle grammar additionally admits an optional exact Source prefix
followed by exactly one tag-31 `final_path_name_by_handle` call on an unknown
native handle. The row fixes scoped-real provider, scalar `0`, post-error `6`,
and operand-zero `Native/Unknown`. It retains operand one's complete authored
mutable carrier as equal resolution and provider pre/post states, operand-two's
`u64` capacity bounded by that carrier, and operand-three's `u32` flags. No
returned path, other evidence lane, or generated-source handoff exists.

Both evaluators reject through their compiler-owned synthetic handle tables
before reading a host path or changing the carrier. Provider-free replay
requires the exact attempt, empty namespace, and teardown before issuing empty
staged-output custody. This receipts only Omega's synthetic-handle model; it
claims neither native path/handle custody nor a Windows security property.

## Unknown-native-handle mutations (summary v62, replay record v43)

One closed family now admits an optional exact Source prefix followed by one
failed synthetic-native-handle mutation: tag 32 `set_file_time`, tag 33
`lock_file_ex`, or tag 34 `unlock_file`. Every row fixes scoped-real provider,
scalar `0`, post-error `6`, and operand-zero `Native/Unknown`.
`set_file_time` retains its `i64` creation value and both complete authored
FILETIME inputs, each at least eight bytes. `lock_file_ex` retains four `u32`
scalars and an unchanged complete OVERLAPPED carrier of at least 32 bytes.
`unlock_file` retains its four `u32` range scalars. All other lanes are empty.

Both evaluators reject these exact rows through compiler-owned synthetic handle
and descriptor tables before sponsor accounting, host timestamp mutation, or
host locking. Replay therefore claims only Omega's modeled invalid-handle
behavior. It claims no operating-system handle, lock, timestamp, or Windows
security property. Tag 35 `get_last_error` remains outside this family because
it observes ordered provider state rather than an isolated handle input.

## Compiler-owned build log (summary v63, replay record v43)

`BuildLog::write_line` is an exact compiler-owned build operation. The checked
interpreter retains its newline-terminated bytes in a dedicated observation
lane, separate from runtime Console stdout and stderr, and the complete lane is
bound into build-observation identity. It grants no boundary-service reach and
does not change the realized filesystem observation class.

Package review debits every retained byte, including the newline, from one
16-MiB compiler-owned account shared by initial evaluation, automatic replay,
and the complete resolved closure. A write that would exceed the account
rejects before changing the retained log; output is never truncated. Usage
schema v4 records initial and replay BuildLog bytes, and successful closure
review requires their exact reconciliation with the shared account. This
limits retained BuildLog custody only; it is not resident-memory containment.

The same sponsor permits at most 65,536 canonical filesystem operation attempts
across initial evaluation, automatic replay, and the complete package closure.
The evaluator charges before appending a pending attempt row, retains exact
initial and replay counts in current usage schema v7, and successful closure review
reconciles both counts with the shared account. This bounds observation-vector
cardinality; it does not claim a bound on resident memory.

Sponsor schema v4 also permits at most 4,096 concurrently live filesystem
resources across the closure. An operation that can mint an owned descriptor,
native handle, duplicate, or find cursor reserves before any provider is
entered. Provider failure drops the pending reservation; successful close and
evaluator teardown release the owned lease. A borrowed native view aliases its
descriptor and therefore does not consume a second slot. Build usage retains
the monotonically observed session peak, successful closure review reconciles
that peak with the shared sponsor, and completion requires zero live leases.
This bounds compiler-admitted build resources only; it makes no claim about
unrelated descriptors in the host process or operating-system confinement.

Sponsor schema v5 additionally admits at most 1,048,576 recursive result cells
and 64 MiB of exact retained Text payload across successful initial and replay
evaluations in the closure. Fixed arrays and aggregate shape consume cells;
Text consumes one cell plus its exact payload-byte count. Structural names and
allocator overhead are deliberately excluded rather than mislabeled as a
portable byte size. Ceiling refusal prevents the result from crossing the
compiler boundary, and successful closure review exactly reconciles both
cumulative charges. This bounds successful result custody, not temporary
evaluator allocation or resident memory.

Sponsor schema v6 additionally permits at most 1,048,576 semantic interpreter
cells live concurrently across the closure. Every cell reserves before its
allocation. Reference and place aliases share that allocation and reservation;
the final alias releases both. Usage schema v7 retains each invocation's peak
and the monotonically observed session peak. Successful closure review
reconciles the session peak exactly and requires zero live cell leases. This is
a semantic allocation-cardinality bound, not a fabricated byte size, allocator
capacity, RSS limit, or hostile-process containment claim.

Sponsor schema v7 additionally permits at most 64 MiB of logical bytes in live
interpreter Text backing buffers across the closure. Every production Text
buffer is created through the evaluator meter; aliases share its byte lease,
and the final alias releases it. Usage schema v7 retains distinct initial and
replay peaks plus the shared session peak. Closure review reconciles the
independently retained invocation peaks with the sponsor and requires zero live
Text bytes. This excludes `Vec` capacity, allocator overhead, temporary
concatenation copies before they become Text, filesystem scratch, RSS, and
process-memory containment.

There is deliberately no generic “temporary logical payload” ceiling. The
evaluator Text backing-payload account is now complete. Synchronous sequential
and positioned file reads each admit one transfer buffer only after their
requested length passes the exact 16-MiB transfer-count gate. A separate peak
receipt would restate that existing per-operation bound without constraining
another lifetime, so none is added. Complete synchronous directory enumeration
truncates retained components to Omega std's existing 255-byte `DirEntry`
carrier and caps packed dirent bytes at 16 MiB per operation before retaining
each snapshot name and before allocating the packed buffer. Packed extent
strictly dominates the complete retained source-name payload, so no duplicate
name account is added. Each find-cursor snapshot has a separate 16-MiB
retained-name ceiling in the ambient/differential interpreter. Rooted package-
build evaluation rejects the unrooted find trio before operand evaluation or
provider service, so no package sponsor, usage, or manifest lane is added for
it. A root-aware Build-facet protocol must be admitted before that decision is
revisited. Partial instrumentation is not described as generic filesystem
scratch or memory containment.

The filesystem replay record remains at v43 because it proves only the bounded
filesystem operation grammar. Build re-evaluation compares the complete
observation, including BuildLog bytes, while the package-level observation
identity binds those bytes durably. Copying the log into the filesystem record
would add no filesystem claim and would conflate two independent observation
lanes.
