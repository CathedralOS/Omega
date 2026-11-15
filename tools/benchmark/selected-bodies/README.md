# Retained selected-body copies

This experiment measures the typed program copied when a real Build selects
retained source. It distinguishes the selected continuation, authored completion,
declaration projection and the saved predecessor used by source-job validation.
It does not introduce shared body storage or parallel typing.

Start in a clean isolated checkout. The capture patch is checked against
`62c864eed68bda01f4a1a60bd08d95414957abb3`. Retain the ordinary compiler before
applying it. Run every build and measured process under the host build budget.
The commands below use POSIX executable names; on Windows pass the actual
paths ending in `.exe`. No command needs PowerShell.

```text
mbx build --release -p omega -p omega-bench
target/release/omega-bench selected-body-inputs target/release/omega build/performance/selected-bodies/inputs
git apply tools/benchmark/selected-bodies/capture.patch
mbx build --release -p compiler --example selected_bodies_profile
mbx build --release -p symbol-resolved-trees-to-typed-trees --example selected_bodies_replay
```

`selected-body-inputs` creates a new directory, preserves the producer and copies
the existing scalar-contract and borrowed-root fixtures without modifying a
byte. `inputs.json` records their source paths and SHA256 identities. An existing
destination is refused. The kept fixtures remain unchanged.

Create the plans through the same Rust tool (replace `INPUTS` with the copied
input directory). Executable paths and targets are explicit on every host:

```text
target/release/omega-bench selected-body-plan target/release/examples/selected_bodies_profile INPUTS/scalar/main.omg windows_x86_64 scalar-plan.json
target/release/omega-bench selected-body-plan target/release/examples/selected_bodies_profile INPUTS/borrowed/main.omg windows_x86_64 borrowed-plan.json
```

Both kept Builds bind all four hosted targets; this experiment requests the
Windows host target explicitly. The driver calls the real public Build
orchestration, including source selection and
checking. At the fixed base, observed and source-verified counters require one
source completion, one publication (including the Build artifact), one Build
execution. The included legacy driver also asserts/prints a settled-check
counter of zero. That counter has no production caller: checker-call count
was not measured. Accepted per-target outcomes remain the semantic oracle.
The initial stale counter-oracle failure is retained separately from valid runs.

The generated ordinary plan declares the exact copied source, adjacent Build and
library inputs, sets the library path and freezes stdout/stderr/exit expectations.
It uses empty stdin, a 600000 ms timeout, one warmup and eight pairs. `profile`
executes one observation, irrespective of pair count. It never builds a compiler.

```text
target/release/omega-bench profile PLAN.json baseline selected-bodies NEW-CAPTURE-DIRECTORY
target/release/examples/selected_bodies_replay NEW-CAPTURE-DIRECTORY
```

The existing runner closes stdin, enforces its deadline, checks the oracle,
records process resources and binds executable/input identities. This backend
adds `typed/profile.log` and private `typed-NNNN.bin` snapshots. The profile
record hashes the log and every snapshot. Replay requires that completed record,
verifies its log and snapshot roster/lengths/SHA256 hashes, then checks semantic
table equality and unchanged program identity after each copy.

Snapshot serialization defaults to finite private limits: 256 MiB encoded bytes,
2 GiB decoded allocation, 134,217,728 cumulative sequence elements and depth
128. The first real scalar capture exceeded the default 4,194,304-element
export cap at 28,160,139 bytes. The failed capture is retained; its observations
are diagnostic only. Compiler, proof, publication and wire budgets are unchanged.
The clone counter row is logged before export so another export failure cannot
discard the measured operation.

An optional plan environment entry `OMEGA_SELECTED_BODY_SNAPSHOT_LIMITS` sets
`bytes,allocation,elements,depth`. One shared parser rejects zero, malformed or
excessive values (maxima: 512 MiB, 4 GiB, 268,435,456 elements, depth 128).
Parsing occurs outside clone windows. The profile records the executed setting
and numeric limits; every export logs its effective limits. Replay uses the
captured setting and verifies it matches the executed command, independently of
the current shell environment. Changing these packaging limits needs no rebuild.

For counter-free whole-process controls, use the retained `baseline-omega`
with `--check --offline`, the same copied source, and explicit repeated
`--target` arguments. A Rust helper retains one successful baseline observation
in a new adjacent `.oracle` directory and freezes its exact streams and exit
status into the ordinary plan. It records the producer hash and includes any
resulting source lock in the plan's declared inputs. The probe is separate from
the paired timing samples. Both slots name the retained compiler; no candidate
optimization or speedup is claimed by this experiment.

```text
target/release/omega-bench selected-body-source-plan INPUTS/baseline-omega INPUTS/scalar/main.omg windows_x86_64 scalar-source-plan.json
target/release/omega-bench selected-body-source-plan INPUTS/baseline-omega INPUTS/borrowed/main.omg windows_x86_64 borrowed-source-plan.json
target/release/omega-bench compare scalar-source-plan.json NEW-SCALAR-CONTROL
target/release/omega-bench compare borrowed-source-plan.json NEW-BORROWED-CONTROL
```

These runs include
Build execution and selection; the capture driver's reported durations are
instrumented attribution and are excluded from wall-time comparisons.
`--verify` is excluded: at the fixed base it withholds accepted package
checkpoints and restricted grants, changing the acceptance workload. It does
not request independent stage validators.

After capture, reverse the patch and restore the original lockfile from the
recorded clean base. The helper crate is linked only by this patch. Ordinary
Psi/compiler dependencies and source contain no profiling hooks.

```text
git apply --reverse tools/benchmark/selected-bodies/capture.patch
git restore --source=62c864eed68bda01f4a1a60bd08d95414957abb3 -- Cargo.lock
```

The live producer's allocation windows cover the exact typed `Arc` operation or
direct `TypedTrees::clone`. Counters are thread-local; other compiler threads
cannot contribute. Snapshot export and logging follow the operation and precede
its caller's semantic mutation. Subsequent operations may observe warmed state,
so these nanoseconds are not an uninstrumented compiler time share.

`requested_bytes` sums successful allocation requests, including reallocations.
`live_delta` and `peak_delta` are net outstanding requested-payload changes from
window entry on this thread. A realloc observes only its successful API result,
excluding allocator-internal transient coexistence. Neither metric includes
allocator overhead or describes resident-page behavior.
`arena_slots` includes the separately cloned dummy and each stored `T`;
`arena_payload_bytes` sums that count times `size_of::<T>()`. It includes padding
and excludes slot metadata and nested owned payloads. This is logical cloned
record volume, not measured memory-bus traffic. Nested allocations are counted
separately by requested bytes. The ordinary process's peak resident measurement
is the evidence for actual process peak memory.

Snapshot decoding can reconstruct or omit derived indexes, and its allocation
state need not match the live input. Compare replay counts with live producer
counts explicitly; semantic equality does not establish allocation equivalence.
Replay isolates the owned typed payload; live `Arc::make_mut` can additionally
allocate its Arc container, while owned-copy and `unwrap_or_clone` sites do not.
Keep raw snapshots, plans, logs, producer identities and resource records outside
Git. The measurement report names exact source/binary identities and limits.
