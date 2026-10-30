# Reduce profiler recording overhead before tuning flow evaluation

Parent: **ITERATION-PERFORMANCE**. Workbench source is
`a3ea073731aae4522c3abc5583ca20814d2bf465`; compiler code is unchanged from
`505424a1b12340a3001b1d28fc5ac298e5de4ec8` before this experiment. The retained
baseline producer is `12ca439bfc0fd52daa45cd186195030b29af4a3bf1f3c5032909fda272f6cd22`.
The preceding [permission experiment](../permission_flow_2026_10_29/README.md)
was discarded for inconclusive whole-compile results.

**Retain the profiler lookup change as a measurement-workflow improvement.**
Every recorded event previously scanned accumulated rows. Nested timing scopes
charge child recording overhead to their parents, making small compiler
operations appear more expensive as profiling detail grows. Keep the same row
vector sorted when names first appear, then binary-search updates. Snapshot
order, string-content identity, saturating arithmetic, synchronization and
report format are preserved; the disabled path is unchanged. No compiler
semantic decision or generated program changes.

## What was measured

Initial temporary instrumentation divides transition/expression processing
into setup, operand capture, view classification, selected-copy lookup, mutation
filtering, observation publication, guard facts and exit facts. The first
six-run screen preserves native bytes and exit. Capture appears to cost about
100 ms over 6,331 calls, while its two newly timed child routines total under
1 ms. The collector itself must be considered before assigning that remainder
to lifetime-retention work.

A fixed-input collector probe replays ten grouped rounds of the initial
profile's 365 timer names and frequencies: 1,173,760 updates per observation.
It verifies every exact count. This is a synthetic collector check, not a
chronological compiler trace. Six ABBAAB observations give baseline median
179.74 ms (173.76–230.39), candidate 57.07 ms (55.79–60.63): **68.2% less
recording time**, about 3.15 times the throughput for this roster. Initialization
is outside the timed region; source, executable and toolchain identities are
retained in [probe inputs](probe-inputs.json) and [runs](probe-runs.json).

The real compiler comparison uses identical temporary scopes, changing only
the collector. All six observations preserve the stack32 native image and exit.
All 395 report names and all 12 targeted flow counts agree. Capture time is
99.61–101.69 ms with the old collector versus 67.95–78.84 ms with the candidate;
medians are 99.818 and 76.040 ms, **23.8 ms less time charged to this scope** for the
same 6,331 captures. This does not imply that the remaining 76 ms is all useful
retention work, or that every changed scope duration is profiler overhead.

The corresponding whole-profiled-compile medians are 2981.3 and 2833.2 ms,
but their ranges overlap broadly. No reliable whole-compile speedup is claimed.
The final production comparison removes every temporary scope and separates
ordinary compilation from profiling-enabled observations.

The initial assertion that every global event count must match failed:
23 unrelated counts vary, including among unchanged baseline runs. Preserve
[event-counts.json](event-counts.json), including that failed assumption. The
fixed-input probe independently verifies exact collector totals; real-run
comparisons require the matching report roster, matching target counts and
native artifacts. Do not silently describe all global counts as identical.


## Production comparison

The original run completed 27 native observations and then timed out (30 s)
executing the dispatch candidate. Its image is byte-identical to the successful
baseline. Six exact-image replays all exit 0 in 32-42 ms, and a fresh 12-run
dispatch comparison passes without changing the timeout. The timeout cause is
unresolved; keep the interrupted attempt in `comparison-runs.json` and
`comparison.log`, with separate `native-replays.json` and
`dispatch-recheck-runs.json`. No original failure is relabeled as a pass.

The table uses the completed original stack32/length lanes and the separate
fresh dispatch lane, four unprofiled observations per variant per workload.
All three lanes produce identical native bytes across variants and exit 0.
These noisy observations are a behavior check, not a normal-speed claim.

| Workload | Baseline median (range), ms | Candidate median (range), ms |
| --- | ---: | ---: |
| stack32 | 3037.9 (2756.4-3798.6) | 3178.8 (2923.9-3664.6) |
| length | 3395.5 (2303.8-4787.9) | 2888.1 (2766.3-3702.8) |
| dispatch | 4221.6 (2679.1-4415.9) | 3140.2 (2833.1-4908.5) |

Including both diagnostic screens, there are 51 successful compiler/native
observations and one native execution timeout, plus six successful native-only
replays. The successful observations are evidence of artifact preservation;
they do not erase the failed attempt. All production comparison transcripts
exclude the temporary `05.flow_detail` instrumentation. Source, Build, producer
and profile identities are preserved in the comparison metadata and raw records.

## Validation and limits

`mbx nextest run -p phase-profile --locked --target-dir build/pass-producer`
passes its expanded test (one test, zero skipped): ordinary counting/scopes,
equal names at different addresses, count/time saturation, concurrent updates,
and sorted unique snapshots. `rustfmt --check` and `git diff --check` cover the
production edit. The temporary flow guards are restored after building; only
`phase-profile/src/lib.rs` remains changed in compiler source.

No new language rule, fact-row completion, full corpus, other-host native
qualification or ordinary compiler speedup is claimed. The baseline and
candidate use the same optimized dev profile and recovery cache configuration;
shared host contention remains visible in the unfiltered timing records.

## Publication and continuation

[PR789](https://github.com/CathedralOS/Omega/pull/789) publishes the preceding
permission experiment record only. Submitted head
`a9eef12470d433b980d4a9b5afa7c5e100b095d4` published as
`66fc7ac3c97bfb21af2f79f90fa96757495e5bb1`. PR, merge-queue and post-merge
smoke checks passed; fetched-main ancestry was verified. See
[publication identities](publication.json). This new profiler change is a
separate local iteration; those CI results do not validate it.

The next question remains operand capture/lifetime retention, now with lower
measurement interference. Its remaining cost is roughly 76 ms on this witness,
so further local tuning needs demonstrated saved work and whole-workload
benefit. Do not treat the synthetic collector gain as progress toward a faster
unprofiled compiler, or sum inclusive scopes into a promised speedup.

## Replay

The [research log](research.md) records hypotheses, primary-source research,
failed replay/count assumptions and their resolution. Apply `profiler.patch`
to the fixed workbench source for the production candidate. Apply
`diagnostic.patch` to either side only for the matched detailed-profile
comparison. Build with `mbx build -p omega --locked --target-dir
build/pass-producer`, `MBX_LEARNED_INCREMENTAL=0`, and an absolute
`MBX_CACHE_DIR` pointing to the retained `build/conjunction-flow/recovery-cache`.
Keep each local build/test chain under one `tools/build_slot.py` hold.

Use the existing `spill_loop_2026_10_28/ablate.py` for each six-run profile
screen and `checking_simplification_2026_10_28/compare.py` for three-workload
production comparison. Arguments are `BASELINE CANDIDATE FRESH_OUTPUT`.
The standalone `probe-baseline.rs` and `probe-candidate.rs` include their
adjacent retained profiler sources; compile each with the pinned `rustc`,
`--edition 2024 --crate-name profile_probe -C opt-level=1`, then run with
`OMEGA_PHASE_PROFILE=1` in ABBAAB order. Their grouped input order is deliberate
and does not claim to reconstruct the original interleaved event stream.
