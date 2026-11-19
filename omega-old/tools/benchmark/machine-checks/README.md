# Independent machine checking

The Rust example checks the same lowered program with different invocation
worker counts. `mixed` gives every sixteenth body the requested number of
additions and the others 1–4; `uniform` gives every body that number; `tiny`
gives every body one. This is generated lowered input, not an observed
distribution of Omega applications.

Build each revision under the host budget, and keep its executable before
changing revisions. Commands run from the checkout root; Windows executables
have an additional `.exe` suffix.

```text
mbx build --config profile.dev.opt-level=1 -p terminal-psi-to-terminal-psi --example machine_checks_mimalloc
target/debug/examples/machine_checks_mimalloc 1 1 mixed 256 64
target/debug/examples/machine_checks_mimalloc 8 1 mixed 256 64
target/debug/examples/machine_checks_mimalloc 8 100 tiny 1 1
```

Arguments are workers, repetitions, shape, machine count and additions in a
large body. Use the same compiler profile for both revisions.

`machine_checks_mimalloc` uses the product's allocator. `machine_checks` uses
System as an additional control. Both include the same `harness.rs`; the
allocator declaration is their only difference. Do not treat System's
allocation contention as production-allocator behavior.

For a controlled old-driver comparison, use a clean isolated benchmark checkout
of this change. Save its candidate executable. Restore only
`omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs`
from `d0e4a86579627fe69d8baae9e6d599416afcff36`, build the same example again,
and save that executable as before. Restore the file from the candidate's HEAD
before continuing. Other implementation modules are unchanged from that base;
the new table iterator is unused by the old driver. This holds the harness,
allocator and dependency graph fixed, rather than claiming a full old-checkout
rebuild. Record both source blobs and executable digests. At eight budget
workers the old driver still checks serially.

The original System experiment instead built the old checkout with the example
backported and `execution-admission` added as a dev dependency. The measurement
report keeps those binary identities separate. Ordinary compiler comparisons
use full before/after compiler executables, not the controlled driver variant.

Input construction, structural lowering, cloning the input between repetitions
and output fingerprinting are outside the reported discharge timer. The timer
includes the whole Psi 06 operation: formation, inference, preparation, final
checks and assembly. Every obligation must pass, every storage use must be
established, and repetitions must have identical output fingerprints. The
fingerprint covers the complete elaboration and discharge debug representations;
it is a comparison oracle for this compiler/Rust build, not a portable artifact
identity. Preserve the baseline's count and fingerprint when comparing a change.

For repeated comparisons, copy `plan.json` and edit its executable paths,
worker count and baseline oracle. Its working directory is relative to the
plan, so adjust that too if moving it. The existing runner closes stdin,
alternates paired runs, checks the oracle and records full-process wall time,
CPU time and peak resident memory alongside reported discharge time:

```text
mbx build -p omega-bench
target/debug/omega-bench compare tools/benchmark/machine-checks/plan.json build/performance/machine-checks-run
target/debug/omega-bench validate build/performance/machine-checks-run
```

Keep raw results under ignored `build/performance/`. Whole-process measurements
also include input construction and debug fingerprinting, so do not label them
compiler throughput. Pair this experiment with an unchanged source through the
ordinary compiler and native/Psi execution before landing a scheduling change.

`trace.patch` is an optional diagnostic patch, not enabled production logging.
Apply it to a clean copy of this change with `git apply`, rebuild the MiMalloc example,
and run one repetition at multiple workers with stderr redirected to a file.
Its JSON lines record each final job's start/end relative to batch readiness,
its worker, and the serial preparation and batch durations. All jobs are ready
after inference; gaps before a job starts are admission waits. Sweep the spans
to count simultaneous running jobs and distinguish waits from preparation.
Logging can perturb scheduling, so these traces explain idle time and are not
the executables used for timing comparisons. Reverse the patch with
`git apply -R` before rebuilding the ordinary compiler.
