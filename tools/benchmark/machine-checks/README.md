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
mbx build -p terminal-psi-to-terminal-psi --example machine_checks
target/debug/examples/machine_checks 1 1 mixed 256 64
target/debug/examples/machine_checks 8 1 mixed 256 64
target/debug/examples/machine_checks 8 100 tiny 1 1
```

Arguments are workers, repetitions, shape, machine count and additions in a
large body. Use the same compiler profile for both revisions. Our measurement
used development opt-level 1, not the unoptimized default or release profile.

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
