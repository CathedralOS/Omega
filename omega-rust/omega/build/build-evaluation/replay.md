# Filesystem replay support

This describes the current bounded Rust replay grammar, not additional language
syntax. [Observation custody](observation_custody.md) owns preparation and
resource accounts; the [specification](../../../../wiki/spec/build/observations.md)
defines the claims a successful replay may publish.

## Owners and common checks

[src/replay_record.rs](src/replay_record.rs) bounds, encodes, and rehydrates
records. Its [submodules](src/replay_record) own operation-specific conversions.
The [checked interpreter](../../../psi/semantics/checked-interpreter/src/lib.rs)
executes provider-free replay; its
[filesystem replay modules](../../../psi/semantics/checked-interpreter/src/filesystem_replay)
validate directories, links, duplicates, locks, ownership, and exact failures.
The broader legacy sequence dispatcher remains in the interpreter entrance.

Replay starts with a fresh virtual Output tree, never a live filesystem provider.
It compares exact ordered attempts, operands, authorization/refusal records,
logical handles, results, post-error state, full mutable carriers, observed
regions, handoffs, exhaustion, teardown, and final output custody. Whole-build
replay additionally checks Build output and BuildLog. A supported operation name
alone is insufficient: its complete sequence and records must fit this grammar.

Nonempty Output replay may omit Source operations. Complete replay still binds
canonical Source metadata and compiler custody, including when Source was never
read. Source-only complete builds retain physical custody of an explicitly empty
Output tree; partial `SourceInputsOnly` is a different verdict. Do not manufacture
a Source attempt to make an Output-only build eligible.

Unsupported shapes remain volatile; they do not become receipts by dropping an
operand, result, failure, or output entry. No table here promises exhaustive
filesystem support.

## Source prefix

| Sequence | Required observations |
| --- | --- |
| `open(flags = 0); (read | read_at)*; close` | Closed, noninterleaved, distinct descriptor lifetimes. All reads succeed. Sequential position starts at zero and advances only for sequential reads; nonnegative positioned offsets leave it unchanged. No failed reads or lifetime reuse. |
| Follow/no-follow path metadata | May intersperse with Source operations. Exact 14 fields, checked layout, full padding/tails, and result. Successful descriptor metadata is not covered. |
| `read_link` | Exact Source root and no-follow grant, count, full carrier, meaningful target bytes, and complete/limited disposition. No authority to follow the returned spelling. |
| `open(flags = 0); read_dir+; close` | Closed directory sequence, exact count, byte carrier and `i64` cursor at resolution/pre/post phases, and packed regions. Records are inert bytes, not an exhaustive namespace or new path grants. Failed enumeration is not covered. |

## Output tree and file chains

Output is an ordered mixture of directory, file, symlink, and hard-link creation.
Parents precede children; collisions and files used as parents reject. Individual
file chains are contiguous. Retain the complete tree, not just selected generated
source. Handoffs are an ordered subset after each corresponding close; absence
of handoffs is valid for ordinary artifacts.

| Operation | Supported form |
| --- | --- |
| File creation | `create(mode = 438)` (0666), followed by zero or more supported descriptor operations and `close`. Empty files are valid. |
| `write`, `write_at` | Full successful writes. Zero-length writes remain observations; a zero-length positioned write does not extend the file. Positioned writes zero-fill gaps and leave sequential position unchanged. |
| `sync`, `sync_data` | Successful descriptor operations with exact recorded post-error state. |
| `set_file_length` | Nonnegative length; truncation/extension preserves position and obeys the peak object-extent limit. |
| `seek` | Checked offset with origin 0, 1, or 2; exact nonnegative result and resulting position. |
| Descriptor permissions | Exact `u32` mode; executable bits determine ordinary/executable output mode. No operation and an explicit reset are distinct traces. |
| Descriptor times | Unchanged input carrier, at least 32 bytes, exact result/error. Host timestamps do not enter tree identity. |
| `duplicate` | Fresh duplicate immediately closed; subsequent operations use the original. No duplicate graph, operations through the duplicate, or delayed close. |
| `lock_file` | Adjacent successful operations 6 then 8 (exclusive/nonblocking, then unlock), on the original descriptor. No shared, blocking, contended, delayed, or native range-lock success. |
| `change_file_owner` (49) | Exact `i32` user/group operands, success or failure under the virtual non-root model, and result/error state. Later operations and close preserve the observed error rather than clearing it by convention. This grants no host ownership authority. Path-based ownership is not covered. |
| Directory creation | Mode 493 (0755), parent first, including empty directory trees. |
| Symlink creation | Nonempty canonical relative UTF-8 target; no NUL, absolute target, or escape from the tree. Target is payload operand 0, link location rooted operand 1. |
| Hard-link creation | Portable operation 19 or Windows operation 27, with exact provider operand order and both write grants in the same Output root. Source is an earlier file or hard link. Virtual topology is retained for replay; the canonical tree represents the resulting files without inode identity. |

Output mutation is checked against the virtual tree and sponsor, not inferred
from a claimed final digest. The package's admitted root/grant decisions remain
inputs; replay cannot expand them.

## Exact failure sequences

Unless stated otherwise, a failure sequence may follow the supported Source
prefix. It has no generated-source handoff and ends with empty Output and clean
teardown. Records retain the exact scoped-provider model and all authored
operands, even where the failure is independent of an operand's value.

### Missing Output entries

A nonempty sequence of at most 4,096 `remove`/`remove_dir` attempts (9/12) may
target absent Output entries. Each has the exact root and write grant, result
`-1`, and error `2`. No mixed successful mutation belongs to this grammar.

### Unknown descriptor

A single unknown-descriptor attempt has result `-1` and error `9`, except the
separately listed `get_osfhandle` form. Operand 0 is the unknown descriptor.

| Operation | Remaining retained operands / effects |
| --- | --- |
| `close` (8), `sync` (43), `sync_data` (44), `duplicate` (45) | No extra operands or successful handle transition. |
| `seek` (10) | `i64` offset at 1, `i32` origin at 2. |
| Descriptor permissions (17), length (41), lock (46), owner (49) | Respectively `u32` mode; `i64` length; `i32` lock operation; two `i32` owner values. |
| Descriptor times (42) | Unchanged mutable operand 1, minimum 32 bytes. |
| `read` (4), `read_at` (6) | Unchanged mutable operand 1, `u64` count at 2 within its capacity; positioned form has `i64` offset at 3. No observed region. |
| `write` (5), `write_at` (7) | Immutable payload at 1; positioned form has `i64` offset at 2. |
| Descriptor metadata (39) | Unchanged mutable operand 1, minimum 144 bytes; no metadata observation. |
| `open_at` (14), `unlink_at` (15) | One portable component and exact `i32` flags. Unknown descriptor fails before path resolution, grant, or host access. |
| `read_dir` (23) | Unchanged mutable bytes at 1 and cursor at 3; `u64` count at 2. No observed region. |

An immediately following `errno` (50), with no operands, may report scalar `9`
and post-error `9` for the exact error-9 forms above. Standalone, delayed,
reordered, or repeated error reads are not covered.

Unknown-descriptor `get_osfhandle` (30) instead returns `-2` with post-error `0`
and no extra operands. It is not the error-9/`errno` sequence.

### Unknown native handle

Each form returns `0` with error `6` and no successful handle transition:

| Operation | Remaining retained operands |
| --- | --- |
| `close_handle` (29) | None. |
| `final_path_name_by_handle` (31) | Unchanged mutable operand 1, `u64` count at 2 within capacity, `u32` flags at 3; no returned path. |
| `set_file_time` (32) | Exact `i64` creation argument and two unchanged FILETIME carriers of at least 8 bytes each. |
| `lock_file_ex` (33) | Four exact `u32` operands and unchanged OVERLAPPED carrier of at least 32 bytes. |
| `unlock_file` (34) | Four exact `u32` range operands. |

An immediate operand-free `get_last_error` (35) may follow any of these, returning
scalar `6` and post-error `6`. No delayed or repeated form is covered. This is
the scoped provider's deterministic error sequence, not a claim about host TLS.

### Source write refusal

One Source `create(mode = 438)` (1) or `remove` (9) may fail before mutation with
result `-1`, error `13`, and exactly one `WriteOutsideGrantedRoots` refusal. This
form has no Source prefix, no BuildLog, no successful path authorization or
handle transition, and no handoff. Bind the exact root/relative path and scoped
provider identity. Replay injects the immutable grant decision before the virtual
operation; it does not consult or attest host permissions. Alternate modes,
refusal classes, repeated attempts, and mixed sequences are not covered.

## Unsupported host paths

The Windows find trio remains unsupported in rooted package build evaluation,
before operand evaluation/provider entry. Its raw directory/physical-path inputs
cannot be silently ignored or turned into package authority. Runtime filesystem
support and ambient differential tests are separate. A new build facet requires
a concrete customer and an explicit scoped contract.

Schema constants in [src/lib.rs](src/lib.rs) and
[replay_record.rs](src/replay_record.rs) own exact version gates. Current notes
describe one grammar; old schema numbers, landing milestones, and test-count
diaries belong in Git history.
