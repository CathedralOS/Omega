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
