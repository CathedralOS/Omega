# Independent final machine checking

Final checking can use the invocation's existing workers once all inferred
contracts have settled. On a generated 256-machine workload using the production
allocator and eight fixed physical cores, eight workers reduced median Psi 06
discharge time from **37.554 s to 6.596 s (5.69x)**.
The unchanged source controls had lower medians, but their noisy comparisons
were inconclusive. This is evidence for the final-check boundary, not for the
whole [proposed parallel pipeline](../designs/compiler_work_ownership.md).

The timing results belong to the recorded original commits. The PR was later
rebased onto `7391847fc4150a38cdfda55ccdf5c58ec4f4740c` to resolve competing
dev-dependency additions; both were retained. Preservation checks were rerun on
that source. Throughput measurements were not rerun on the new base, so the
numbers below are not fresh performance validation of that revision.

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
  Wide runs use Idle priority and affinity `0xFFFFC0`, leaving six logical CPUs
  for the owner. The fixed-core follow-up uses `0xFF0000`, described below.
  All builds use mbx 1.29.2 with the shared 18-CPU/18-job ceiling.
- Development profile, opt-level 1, repository-pinned Rust toolchain.
  The original example uses System; the matched example and ordinary compiler
  use MiMalloc. Results do not establish allocator-neutral
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

## Production-allocator comparison on fixed cores

The first example used System, while the compiler uses MiMalloc. A second
example includes the exact same harness with the product's allocator declaration.
Both executables are built in the candidate tree with identical dependencies.
The before executable restores only Psi 06 `lib.rs` to its exact base blob,
`e798214d4b0bc0890aaee6ac58f141323b5397c2`; all other implementation modules
are unchanged from the base, and the added table iterator is unused by that
driver. The after executable restores the candidate driver. This is a controlled
old-driver comparison, not a claim to have rebuilt the entire old checkout.

The original wide mask mixes SMT pairs and separate non-SMT cores. After the
wide-mask experiments below, a follow-up recorded the same numeric limits and
used only logical CPUs 16–23 (`0xFF0000`), which Windows reports as eight separate
non-SMT cores. A running child's Idle priority and actual mask were observed.
This reduces placement variation without making the shared host quiescent.
Both roles still own an eight-worker pool; the old driver checks serially.

| Pair | Old driver s | Parallel driver s | Old CPU s | Parallel CPU s | Old peak MiB | Parallel peak MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 52.608 | 19.740 | 27.922 | 26.563 | 315.60 | 385.34 |
| 2 | 34.326 | 18.927 | 25.844 | 27.047 | 317.93 | 406.36 |
| 3 | 27.410 | 5.489 | 25.109 | 24.844 | 316.95 | 434.45 |
| 4 | 30.761 | 5.388 | 25.203 | 25.766 | 317.84 | 419.36 |
| 5 | 37.023 | 6.814 | 26.281 | 22.469 | 273.91 | 367.97 |
| 6 | 38.085 | 6.658 | 26.203 | 24.844 | 317.95 | 416.24 |
| 7 | 39.835 | 6.310 | 27.219 | 23.094 | 317.89 | 396.24 |
| 8 | 40.257 | 6.535 | 27.078 | 22.859 | 296.45 | 402.88 |

The runner classifies the discharge difference as `candidate_lower_in_this_run`.
The role-median speedup is 5.69x; the median paired change is -82.04%.
Median process CPU is 26.242 s before and 24.844 s after. Every observed CPU
ratio is below 1.05x. The largest observed resident peaks are 317.95 and
434.45 MiB, an increase of 116.50 MiB. The configured comparison meets the 20%
elapsed-improvement, 1.5x CPU and +256 MiB memory limits. These observations do
not promise a CPU bound under other placement, host load or worker counts.

On that same subset, the eight-pair one-worker control measured 38.446 s before
and 39.511 s after (+2.77% by role medians). Its paired percentage interval is
[-23.60%, +0.18%]: inconclusive for a speedup, but below the predeclared +5%
regression guard. This does not turn the noisy wide-mask controls into passes.

The preceding MiMalloc wide-mask comparison measured 27.010 s versus 4.670 s
(5.78x), but one pair's CPU ratio was 1.589x and missed its provisional guard.
That miss is retained, not replaced by the fixed-core result. Its peak resident
memory increased from 319.25 to 440.88 MiB. The wide-mask one-worker MiMalloc
control measured 21.393 s versus 18.382 s, but its interval included zero.

## Original System-allocator control

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
eight-pair System confirmation above supersedes that sizing series. Its
35.731 s versus 8.191 s medians give a 4.36x speedup. Keep this control separate
from the production-allocator comparison.

The System one-worker comparison was inconclusive (25.583 s versus 30.492 s);
it does not establish the proposed 5% guard. The same-candidate two/four-worker
control measured 13.381 s versus 7.570 s, with five pairs, so the runner marked
it `insufficient_pairs`. The one-body control, which keeps the serial path even
with eight pool workers, measured 3.818 versus 4.032 ms per discharge across
100 repetitions and eight pairs. Its median difference is within the 1 ms/10%
tiny-input allowance; its interval includes zero. None of these controls
establishes a complete scaling curve on a quiet host.

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

## Work boundaries and remaining waits

An optional diagnostic build, using the wider `0xFFFFC0` mask and MiMalloc,
recorded all 256 final jobs on worker indices 0–7, with eight jobs in flight at
peak. The jobs queue 256 pairs of machine identity and mutable table reference
(4 KiB on this host), not 256 prepared proof contexts. Result payloads remain
owned by their machines; the ordered join moves their headers.

| Boundary | Observed duration in that trace | Reason and disposition |
| --- | ---: | --- |
| Formation, inference and shared preparation | 4.973 ms | Required inputs settle before final checking; this generated input does not make that prefix a material pinch. Summary concurrency is outside this change. |
| Final-check batch | 4.005 s | Independent jobs share the existing budget. No extra pool or worker stacks are created. |
| No active job while ready jobs remained | 0.128 ms total and maximum | Initial admission delay; no interval reached 1 ms. |
| Fewer than eight jobs active after the queue emptied | 1.061 s | Last large bodies finishing, with no unstarted body left. The final acceptance join still waits for them. |

These are admitted-job spans, not observations of OS CPU residency: a span can
include host preemption. Logging also perturbs scheduling. The trace explains
the batch's work availability and tail; it is not one of the timed comparison
executables and does not prove a whole-compiler idle-time breakdown.

The worker count bounds simultaneously prepared checking jobs. There is no new
hard per-job byte limit or byte-reservation system; process resident peaks above
are observations of this workload, not a memory guarantee for arbitrarily large
bodies. A one-body input stays serial. Focused nested rejection tests terminate
without extra pools and preserve the lowest structural refusal. The ordinary
acceptance owner still waits for all required judgments and publishes no partial
batch. Psi 05's mutable qualification catalog and frontend body ownership remain
separate work in the design.

## Preservation and repository checks

Sixteen distinct focused and nearby tests passed: the new parallel proof,
refusal and nested-budget controls; six result-summary tests; three inferred-
input tests; three machine-input association tests; and the dense table's
identity-order test. The new proof control replays accepted certificates through
the kernel and checks that tampering is refused. Its valid and invalid inputs
differ only in the caller's lower bound. Results, inferred contracts, knowledge
IDs and proof trees match at 1, 2, 4 and 8 workers.

The scalar and borrowed-root source copies each ran with `omega run --both`
at one and eight workers on Windows x86_64. All four invocations returned native
exit 0 and Psi exit 0. Standard input was closed. An initial spelling with
`run --both --offline` was refused by the option parser; execution evidence uses
the supported spelling without that option. The kept programs and their oracles
were not edited, and this change adds no `.omg` files.

Scoped commands, with development opt-level 1 and the host budget above:

```text
mbx test -p terminal-psi-to-terminal-psi --lib parallel_checks
mbx test -p terminal-psi-to-terminal-psi --lib machine_batch
mbx test -p terminal-psi-to-terminal-psi --lib tests::result_summaries
mbx test -p terminal-psi-to-terminal-psi --lib tests::inferred_bounds
mbx test -p terminal-psi-to-terminal-psi --lib machine_inputs
mbx test -p semantic-vocabulary --lib table_holds_one_consecutive_run_in_identity_order
mbx clippy -p terminal-psi-to-terminal-psi -p semantic-vocabulary --all-targets -- -D warnings
mbx xtask fmt --check
mbx xtask check-boundary
```

All listed checks passed. Critical parallel tests, both-crate all-target Clippy,
formatting and boundary checks were repeated after the harness-only allocator
refactor. The diagnostic trace patch was reversed before those checks; ordinary
compiler sources contain no timing hooks.

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
| Controlled old-driver MiMalloc example | `34a7976531c6783d6c8d9e433dec691641000d07ef82973faddd7c508330c704` |
| Candidate MiMalloc example | `aeed23636401eb8969f545e6b7af3240ea631191807d26d5ae60c54a670f3e81` |
| Before compiler | `13b07869bc5dced28eb478dbda2956aa330fff246679ad0bc23814cfb0d3383b` |
| Candidate compiler | `ad2c8d0f707e16dcde3c69be7757632a4f63671be8ced856f483e541f21eb499` |

Raw plans, immutable journals, stdout/stderr, input/binary digests and resource
observations are retained under ignored
`.agents/worktree/worker-33/build/performance/parallel-machine-checks/`.
The configured production-allocator directories are `mimalloc-fixed-8-results`
and `mimalloc-fixed-1-results`. Wider-mask results remain in `mimalloc-8-results`
and `mimalloc-1-results`. System results remain in `mixed-8-confirm-results`,
`mixed-1-results`, `scaling-2-4-results` and `tiny-1-results`;
`mixed-8-results` retains the exploratory five-pair series. Ordinary compiler
controls are `source-scalar-results` and `source-borrowed-results`.
`omega-bench validate` passes on these completed directories.
