# Independent final machine checking

Final checking can use the invocation's existing workers once all inferred
contracts have settled. On a generated 256-machine workload, eight workers
reduced median Psi 06 discharge time from **35.731 s to 8.191 s (4.36x)**.
The unchanged source controls had lower medians, but their noisy comparisons
were inconclusive. This is evidence for the final-check boundary, not for the
whole [proposed parallel pipeline](../designs/compiler_work_ownership.md).

Each job borrows the immutable module and exclusively mutates one machine's
checking tables. Proof scratch is created inside admitted jobs. The batch
queues references, retains body results and joins them in machine order. It
creates no pool, copies no body payload and keeps proof allowances local to
their original goals. A structural refusal cancels only higher identities
that have not started; lower identities still finish so scheduling cannot
choose the first diagnostic. One worker or one machine keeps the serial path.

## Sources and conditions

- Production base: `d0e4a86579627fe69d8baae9e6d599416afcff36`.
- Candidate checker: `7594dc74194805d1013a0b889b4c7440d10fba12`.
  Subsequent benchmark and report changes do not change that checker.
- Windows x86_64, Intel i9-12900K, 24 logical CPUs, 128 GiB RAM.
  Idle priority and affinity `0xFFFFC0` leave six logical CPUs for the owner.
  All builds use mbx 1.29.2 with the shared 18-CPU/18-job ceiling.
- Development profile, opt-level 1, repository-pinned Rust toolchain.
  The example uses Rust's default System allocator; the ordinary compiler uses
  its production MiMalloc allocator. Results do not establish allocator-neutral
  scaling or performance on another host.
- Fresh processes, one warmup per role, alternating paired order, closed stdin.
  The primary confirmation and source controls have eight pairs. Warm OS caches
  are possible; these are not cold-disk measurements.
- This shared workstation was not quiescent. Other checkouts' Rust builds and
  tests were observed during the campaign. No build or test from this session
  overlapped its own timing comparisons. Absolute times and small differences
  need particular caution.

The generated input has 256 machines and 1,648 arithmetic obligations. Every
sixteenth body has 64 additions; the others have 1–4. These are real Psi 05
lowered bodies and real Psi 06 proofs, but the distribution is generated, not
measured from an Omega application. Source construction, Psi 05 lowering,
input cloning and output fingerprinting are outside the discharge timer.
Formation, inference, shared preparation, final checking and assembly are inside.

Every obligation and storage use must succeed. The complete elaboration and
discharge fingerprint is `92a681f1b71f0606` in every primary observation.
This debug-representation fingerprint is an oracle for this compiler/Rust
build; it is not a portable artifact identity.

## Primary confirmation

Times below are seconds. CPU and peak memory cover the primary process,
including untimed setup and fingerprinting; they exclude descendants. Resident
memory is not reserved worker-stack address space or a per-job allocation count.

| Pair | Before, 1 worker | Candidate, 8 workers | Before CPU | Candidate CPU | Before peak MiB | Candidate peak MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 42.624 | 15.941 | 31.766 | 36.266 | 228.0 | 247.5 |
| 2 | 39.388 | 14.186 | 33.359 | 35.656 | 228.1 | 250.5 |
| 3 | 65.195 | 17.090 | 34.766 | 36.297 | 228.0 | 250.7 |
| 4 | 46.934 | 10.883 | 35.016 | 36.250 | 228.4 | 247.8 |
| 5 | 28.295 | 3.899 | 25.375 | 30.984 | 228.0 | 251.6 |
| 6 | 18.549 | 4.117 | 18.344 | 30.281 | 228.6 | 249.2 |
| 7 | 26.668 | 4.782 | 24.406 | 34.750 | 228.5 | 251.8 |
| 8 | 32.074 | 5.498 | 28.828 | 35.609 | 228.1 | 251.6 |

The runner classifies the paired discharge difference as
`candidate_lower_in_this_run`. Its median paired percentage change is -77.31%;
the ratio of role medians is 4.36x. These are different statistics.
Median whole-process CPU is 30.297 s before and 35.633 s after (+17.6%).
The largest observed process peaks are 228.6 and 251.8 MiB, a 23.2 MiB increase.
Individual CPU ratios vary considerably; the median does not describe every
pair. The shared host also explains why the absolute timing spread is material.

Before evaluating the candidate, the decision file required at least a 20%
median discharge improvement at eight workers, process CPU at most 1.5x,
and peak resident memory at most baseline + 256 MiB. Discharge and peak memory
meet those limits. CPU meets the limit at role medians, but individual ratios
exceed 1.5x; these data do not establish a per-run CPU bound.
An initial five-pair series also favored the candidate (42.561 s
versus 9.097 s medians), but the runner marked it `insufficient_pairs`; the
eight-pair confirmation above is the primary result.

## Ordinary compilation controls

Both controls copy unchanged kept `main.omg` and `build.omg` files with the
existing `selected-body-inputs` helper. They run the actual compiler with
`--check --offline --target windows_x86_64`, eight invocation workers in both
roles, and the same library root. Each reports 237 compiled source files and
`wrote_output=false`. They include their real Build routes, rather than replay
only the final checker.

| Kept source | Before median | Candidate median | Paired percentage change | Runner interpretation |
| --- | ---: | ---: | ---: | --- |
| `tests/fixtures/run/calls/scalar_contract_route_exit` | 8.945 s | 7.810 s | -17.17% | inconclusive |
| `tests/fixtures/run/ownership/borrowed_root_replacement` | 6.924 s | 6.434 s | -4.94% | inconclusive |

Their paired 95% intervals include zero: scalar [-3.207, +1.214] s; borrowed
root [-1.186, +0.199] s. These runs establish matching successful outcomes and
record the observed medians. They establish neither a whole-compiler speedup
nor a tight whole-compiler regression bound on this host. Other hosted targets
and large runnable applications were not measured.

## Reproduction and retained artifacts

Use the [Rust example and comparison plan](../../../tools/benchmark/machine-checks/README.md).
The README explains how to add the same benchmark to the original base without
changing its production checker. All builds and tests use mbx; no PowerShell
benchmark script is required. The optional `trace.patch` only supplies timing
diagnostics and is absent from the ordinary compiler and paired executables.

Retained executable SHA-256 values:

| Executable | SHA-256 |
| --- | --- |
| Before example | `7ba18f609189e74619af29b2c0277820d5626bc0e2dd4baafa9446e6e7d55c9e` |
| Candidate example | `7a90186a5dc28053528075f92863f6f9f754ecaca24c115add5a373bc2d2ea37` |
| Before compiler | `13b07869bc5dced28eb478dbda2956aa330fff246679ad0bc23814cfb0d3383b` |
| Candidate compiler | `ad2c8d0f707e16dcde3c69be7757632a4f63671be8ced856f483e541f21eb499` |

Raw plans, immutable journals, stdout/stderr, input/binary digests and resource
observations are retained under ignored
`.agents/worktree/worker-33/build/performance/parallel-machine-checks/`.
The primary directories are `mixed-8-confirm-results`, `source-scalar-results`
and `source-borrowed-results`; `mixed-8-results` retains the exploratory series.
`omega-bench validate` passes on these four completed directories.
