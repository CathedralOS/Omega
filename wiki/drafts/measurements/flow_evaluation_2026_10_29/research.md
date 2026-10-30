# Transition and expression flow preparation

Parent ITERATION-PERFORMANCE, fixed workbench source a3ea073731aae4522c3abc5583ca20814d2bf465. Compiler source is unchanged from 505424a1b12340a3001b1d28fc5ac298e5de4ec8; baseline producer SHA256 12ca439bfc0fd52daa45cd186195030b29af4a3bf1f3c5032909fda272f6cd22, optimized dev level 1.

The [preceding experiment](../permission_flow_2026_10_29/README.md) did not establish a gain from pruning failed owned roots. Its final baseline profile records 261.5 ms in transition flow and 213.3 ms in expression flow. Existing nested invocation scopes account for 155.0 ms, including 85.5 ms capturing normal-return stores and 44.3 ms building call facts. These inclusive scopes are not independent contributions to add to wall time.

First resolve attribution: temporary guards separate execution setup, expression evaluation, operand capture and mutation filtering, view classification, selected payload-copy queries, observation publication, transition guard facts and exit facts. Six interleaved baseline/diagnostic runs use the existing stack32 harness and require identical native images and exit 0. Restore source immediately after building the instrumented producer. No diagnostic code is a production optimization.

Contracts: evaluation order, completed operand identity, storage epochs, guard fallthrough and selected branch contexts remain unchanged. Reuse the existing research logs on narrowed work domains and LLVM MemorySSA invalidation; a newly selected nontrivial mechanism receives its own focused primary-source search. Do not broaden cache lifetimes merely because calls look similar.

Publication of the prior result is PR789, submitted a9eef12470d433b980d4a9b5afa7c5e100b095d4 from an isolated publication worktree based on published PR767. Only the completed documentation commit is copied there; measurements stay bound to their original warm source and producers.

## Profiler bookkeeping is part of the measurement

The initial diagnostic reports roughly 100 ms in `capture_operand`, but its new child scopes individually total less than 1 ms. Before attributing the remainder to retention, inspect the measurement owner: `phase-profile::add` scans every existing row for each recorded event; a parent scope includes its children's recording overhead. This is a hypothesis about attribution contamination, not proof that the whole 100 ms is overhead.

Research 2026-10-29 queries: `site.llvm.org docs XRay instrumentation overhead profiling`; `site.doc.rust-lang.org std collections HashMap Entry LazyLock`. Opened LLVM XRay introduction and platform/runtime description (https://llvm.org/docs/XRay.html); selective tracing is relevant context but its binary patching runtime is not being adopted on this Windows workload. The HashMap documentation search suggested indexed updates, but a new map is unnecessary for the existing small row vector. Read Rust slice `binary_search_by` and sorted insertion contracts (https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search_by). Maintain lexical name order at first insertion and binary-search subsequent updates. This preserves content-name identity, saturating totals, synchronized process-wide accumulation, and the already-sorted public snapshot, with no change to disabled scopes.

A counter-only probe reconstructs the 365 diagnostic names and their observed frequencies. It pre-registers names in report order, then replays ten grouped rounds (1,173,760 counter events). This is a synthetic collector check, not a chronological trace or end-to-end compiler benchmark. Six ABBAAB observations: baseline 230.4/179.7/173.8 ms; candidate 55.8/57.1/60.6 ms. Every exact event total is checked. The median collector reduction is about 68%; this does not establish the fraction of the 100 ms attributable to instrumentation.

Next, compare identical temporary flow guards with only the profiler collector changed, checking all reported names/counts and native images. Build the production profiler candidate without the temporary scopes and run the existing three-workload comparison, separating profiling-enabled observations from ordinary compilation. Retain only a supported measurement-workflow benefit; do not claim an ordinary compilation speedup from enabled-profiler timings.

The small diagnostic/probe build initially waited on both normal build slots; its owned waiting process was stopped and the short probe ran in the diagnostic lane. The later full compiler chain acquired the normal pool. One replay attempt refused the diagnostic patch's mixed CRLF encoding before compiling a candidate; regenerate the same guarded source and retain Git's normalized diff. Production flow source is restored after each diagnostic build.

## Matched guarded compiler result

The six-run collector comparison preserves native bytes and exit. The exact 395-name report roster and all 12 targeted flow-scope counts agree. The initial stronger assertion of identical global counts failed: 23 other counters vary, including within unchanged baseline runs. Record every value in event-counts.json rather than suppressing the failure or treating a process-varying memoization count as a collector defect. Fixed-roster synthetic replay and the concurrent content-name aggregation tests independently check exact accumulation.

Capture timing ranges do not overlap: original collector 99.61-101.69 ms, sorted collector 67.95-78.84 ms (medians 99.818 and 76.040 ms), with 6,331 captures each. This confirms some measurement contamination, not that all capture cost is bookkeeping. Whole profiled medians are 2981.3 and 2833.2 ms, but broad overlapping ranges prevent a reliable whole-compile speedup claim. Keep the remaining capture/retention question open, and distinguish ordinary compilation from profiling-enabled workflow cost.

## Native execution interruption

The production comparison completed 27 compile/native observations, then timed out after 30 seconds running the dispatch candidate in observation 03. Compilation succeeded. Preserve the complete comparison.log and partial runs.json; do not relabel that native attempt as a pass. Its image SHA256 dc9300a3b440dff440062b5c9d3e238df5d916130356214c074a186898866d04 exactly matches the previously successful baseline images. At inspection the host had roughly 83 GiB free physical memory. No timeout process remained after the harness stopped it.

Six ABBAAB native-only replays of the exact baseline/candidate images all exit 0 in 32-42 ms. This does not establish the timeout's cause. Complete a fresh dispatch-only comparison with the same 30-second native timeout; retain both the interrupted observations and the new record. Do not rerun the successful stack32/length lanes. Diagnostic replay uses the short diagnostic build-slot pool while the normal pool is occupied; host contention still precludes ordinary-speed claims.
