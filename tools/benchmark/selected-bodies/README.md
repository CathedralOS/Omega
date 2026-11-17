# Retained selected-body copies

This experiment measures the typed program copied when a real Build selects
retained source. It separates the selected continuation, authored completion,
declaration projection and the saved predecessor source-job validation uses.
It adds no shared body storage or parallel typing.

Start in a clean isolated checkout. The capture patch applies to
`62c864eed68bda01f4a1a60bd08d95414957abb3`. Save the ordinary compiler
first. Run every build and measured process under the host build budget.
The commands below use POSIX executable names; on Windows pass the actual
paths ending in `.exe`. No command needs PowerShell.

```text
mbx build --release -p omega -p omega-bench
target/release/omega-bench selected-body-inputs target/release/omega build/performance/selected-bodies/inputs
git apply tools/benchmark/selected-bodies/capture.patch
mbx build --release -p compiler --example selected_bodies_profile
mbx build --release -p symbol-resolved-trees-to-typed-trees --example selected_bodies_replay
```

`selected-body-inputs` creates a new directory and copies the producer and the
scalar-contract and borrowed-root fixtures byte for byte; `inputs.json`
records their source paths and SHA256 hashes. It refuses an existing
destination and leaves the kept fixtures alone.

Create the plans with the same tool (`INPUTS` is the copied input
directory). Executable paths and targets are explicit on every host:

```text
target/release/omega-bench selected-body-plan target/release/examples/selected_bodies_profile INPUTS/scalar/main.omg windows_x86_64 scalar-plan.json
target/release/omega-bench selected-body-plan target/release/examples/selected_bodies_profile INPUTS/borrowed/main.omg windows_x86_64 borrowed-plan.json
```

Both kept Builds bind all four hosted targets; this experiment asks for the
Windows target. The driver calls the real public Build orchestration,
including source selection and checking. At the fixed base, observed and
source-verified counters must show one source completion, one publication
(including the Build artifact) and one Build execution. The legacy driver also
asserts and prints a settled-check counter of zero, but no production code
calls it: checker calls were not measured. Each target's accepted result
remains the test of correctness. The first failure, from a stale counter
expectation, is kept apart from valid runs.

The generated plan declares the exact copied source, adjacent Build and
library inputs, sets the library path and fixes the expected stdout, stderr and
exit status. It uses empty stdin, a 600000 ms timeout, one warmup and eight
pairs. `profile` runs once regardless, and never builds a compiler.

```text
target/release/omega-bench profile PLAN.json baseline selected-bodies NEW-CAPTURE-DIRECTORY
target/release/examples/selected_bodies_replay NEW-CAPTURE-DIRECTORY
```

The existing runner closes stdin, enforces its timeout, checks expected
output and records process resources and executable and input hashes.
This backend adds `typed/profile.log` and private `typed-NNNN.bin` snapshots,
all hashed in the profile record. Replay needs that record, verifies the log
and the snapshot list, lengths and SHA256 hashes, then checks after each copy
that tables are equal and program identity unchanged.

Snapshot serialization defaults to these limits: 256 MiB encoded,
2 GiB decoded allocation, 134,217,728 sequence elements in total and depth
128. The first real scalar capture exceeded the earlier default cap of
4,194,304 elements at 28,160,139 bytes. That capture is kept for diagnosis
only. Compiler, proof, publication and wire limits are unchanged.
The clone counter row is logged before export, so an export failure cannot
lose the measurement.

An optional plan environment entry `OMEGA_SELECTED_BODY_SNAPSHOT_LIMITS` sets
`bytes,allocation,elements,depth`. One shared parser rejects zero, malformed or
excessive values (maxima: 512 MiB, 4 GiB, 268,435,456 elements, depth 128).
Parsing happens outside clone windows. The profile records the setting used
and its limits; every export logs the limits in effect. Replay uses the
captured setting and checks it against the command that ran, ignoring the
current shell. Changing these limits needs no rebuild.

For whole-process control runs without counters, use the saved
`baseline-omega` with `--check --offline`, the same copied source and one
`--target` argument per target. A Rust helper saves one successful baseline run
in a new adjacent `.oracle` directory and writes its streams and exit status
into the plan as expectations. It records the producer hash and adds any
resulting source lock to the declared inputs. This probe is separate from the
timing pairs. Both slots name the saved compiler; no speedup is claimed.

```text
target/release/omega-bench selected-body-source-plan INPUTS/baseline-omega INPUTS/scalar/main.omg windows_x86_64 scalar-source-plan.json
target/release/omega-bench selected-body-source-plan INPUTS/baseline-omega INPUTS/borrowed/main.omg windows_x86_64 borrowed-source-plan.json
target/release/omega-bench compare scalar-source-plan.json NEW-SCALAR-CONTROL
target/release/omega-bench compare borrowed-source-plan.json NEW-BORROWED-CONTROL
```

These runs include Build execution and selection. The capture driver's
instrumented durations are left out of wall-time comparisons. `--verify` is
not used: at the fixed base it withholds accepted package checkpoints and
restricted grants, changing the measured work. It does not request independent stage validators.

After capture, reverse the patch and restore the original lockfile from the
recorded clean base. Only this patch links the helper crate; ordinary Psi and
compiler code has no profiling hooks.

```text
git apply --reverse tools/benchmark/selected-bodies/capture.patch
git restore --source=62c864eed68bda01f4a1a60bd08d95414957abb3 -- Cargo.lock
```

In the live compiler, each allocation window covers just the typed `Arc`
operation or direct `TypedTrees::clone`. Counters are per thread, so other
compiler threads add nothing. Export and logging run after the operation
and before the caller changes the data. Later operations may find warm caches,
so these nanoseconds are not a share of uninstrumented compile time.

`requested_bytes` sums successful allocation requests, including reallocations.
`live_delta` and `peak_delta` are the net change in outstanding requested
bytes on this thread since the window opened. A realloc counts only its
successful result, not the moment the allocator briefly holds both blocks. Neither metric includes
allocator overhead or describes resident-page behavior.
`arena_slots` includes the separately cloned dummy and each stored `T`;
`arena_payload_bytes` sums that count times `size_of::<T>()`. It includes padding
and excludes slot metadata and nested owned data. It is the size of the cloned
records, not measured memory traffic; nested allocations are counted
separately in requested bytes. The ordinary process's peak resident
measurement is the evidence for peak memory.

Snapshot decoding may rebuild or omit derived indexes; its allocations
need not match live ones. Compare replay and live counts explicitly;
equal tables do not mean equal allocations. Replay measures just
the owned typed data; live `Arc::make_mut` can also allocate its Arc
container, while owned-copy and `unwrap_or_clone` sites do not.
Keep raw snapshots, plans, logs, producer identities and resource records outside
Git. The measurement report names exact source/binary identities and limits.

## Retained side-record measurements

The later side-record repair uses two separate observers. The original Build
routes expose which retained field owners allocate when they fork. A nonempty
source-stage driver verifies that a successor keeps its predecessor's request
rows while producing new output. Their windows overlap neither each other nor
ordinary timing controls, so keep their results separate.

The patches below have recorded preimages at
`b6b44c31fe8693380fcee5aef7ceef540dc6041c` (the ownership repair before unrelated main changes). The ordinary
baseline is `2b586716b443764af2b88616403b9c8b8ee1f2d0`, a test-only descendant of `fe6e3d2e0104859a25ebfd1b0bfe4b424885e75f`. Preserve the
patches and the common driver before checking out either source in an acquired
worker. Both patches were checked against both recorded preimages; the Build
patch omits only two owner-type context lines that differ between them. Run `git apply --check` at that exact source before applying a patch;
a later source needs review and fresh measurements. These patches add private
examples and the existing observer dependency only while the experiment runs.
Neither patch belongs in the ordinary compiler configuration.

| Artifact | SHA256 |
| --- | --- |
| `side_records.patch` | `d764a276a05bf70ceda26bb9e0743dbac71e0926e0823ee8ceca27edb1f7bd56` |
| `nonempty.patch` | `6f3a74a29a4fb4300709370607c9edf0ff890cf12c0fef2f87655590408d5d0c` |
| Common `nonempty.rs` | `317a36f43ef327193a086eccb37dd84eaf01125212acd4d86e9d6ee19a28b170` |

For the Build routes, freeze byte copies of both fixtures in the measurement
report, their Builds and the selected library tree. Keep those same copies,
profile, feature union, toolchain and execution budget for both sources. Under
the host's shared build guard, build and preserve both examples:

```text
git apply --check tools/benchmark/selected-bodies/side_records.patch
git apply tools/benchmark/selected-bodies/side_records.patch
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build --locked -p compiler --example side_records_profile --example side_records_control < /dev/null
```

Copy `target/debug/examples/side_records_profile` and
`target/debug/examples/side_records_control` into a new ignored directory and
record their SHA256 identities before reversing the patch. Record the source
commit, original input digests, complete command, environment and effective
resource budget beside each saved executable.

```text
git apply --reverse tools/benchmark/selected-bodies/side_records.patch
OMEGA_LIBRARY_ROOT=/absolute/frozen/library OMEGA_SELECTED_BODY_PROFILE=/absolute/new/typed timeout 120s SAVED/side_records_profile --check --targets windows_x86_64 /absolute/frozen/scalar/main.omg 1 1 1 0 < /dev/null
```

Repeat for the unchanged borrowed-root fixture. `SIDE_SELECTED` records the
actual pending-body and source counts; an empty selected activation must not be
reported as nonempty completion. `SIDE_COPY` windows measure each real clone
without retaining another owner. The enclosing `tables` row is excluded because
its children have their own windows. Disabled observer branches remain in the
saved control, with the normal MiMalloc allocator and no counters. For timing,
unset `OMEGA_SELECTED_BODY_PROFILE`, run the saved control on the same frozen
input, verify exact output and exit, and alternate baseline/candidate order in
fresh processes. Keep capture times out of the ordinary timing comparison.

For nonempty completion, reverse the Build patch first: nested allocation
windows are invalid. Apply `nonempty.patch` instead and build its examples:

```text
git apply --check tools/benchmark/selected-bodies/nonempty.patch
git apply tools/benchmark/selected-bodies/nonempty.patch
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build --locked -p source-compiler --example side_records_nonempty_profile --example side_records_nonempty_control < /dev/null
```

Preserve the two executables and reverse the patch. The portable driver keeps the measured operations and guards, with formatting
and the frontend fixture module path adapted to its new directory. Its
registration patch includes the common file instead of embedding it twice as
the archived private examples did. These rebuilt executables therefore need
new binary identities and fresh observations.

The input repeats the computed-array helper used by the existing
`nonempty_selected_completion_retains_prior_refusal_rows` unit test, then keeps
one pending length body. On Bash, generate the measured input once before
comparing sources:

```bash
{
    printf 'machine length() -> u64 { 4 }\n'
    for helper_number in {0..63}; do
        printf 'machine helper_%s() { let bytes: [u8; length()]; }\n' "$helper_number"
    done
} > /absolute/frozen/nonempty/main.omg
```

Its SHA256 must be
`5faf1f435ea56953e3a9f3b4a08e6c738355585a97564f679ec84363032cbb90`.
With 64 helpers it must print exactly
`completed_nonempty_batches=64 retained_refusals=128 pending_length_body=1`.

```text
git apply --reverse tools/benchmark/selected-bodies/nonempty.patch
OMEGA_SELECTED_BODY_PROFILE=/absolute/new/typed OMEGA_NONEMPTY_PHASES=/absolute/new/phases.json timeout 120s SAVED/side_records_nonempty_profile 64 /absolute/frozen/nonempty/main.omg window < /dev/null
```

`window` measures actual completion and preparation deferral together. The
separate `fields` mode observes individual owner clones and isolated mutation
and existing-key recorder probes; its repeated probe calls are not production
call counts. In particular, a parent table total overlaps its child owners and
must not be summed with them. All invariant and pointer observations occur
outside the completion allocation window. Phase clocks separate setup,
resolution, completion, request validation and pointer checks. Run the saved
System-allocator control with `window` and the profile variable unset for
counter-free timing. Preserve phase files, streams and acceptance receipts
outside Git; do not add a growing result ledger.

The two-helper unit test is the kept regression for exact request retention,
predecessor isolation and refusal of unrelated work. The 64-helper observer is
measurement evidence only, not acceptance of a complete program or proof of a
total compiler speedup.
