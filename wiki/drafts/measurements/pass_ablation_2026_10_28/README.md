# ControlFlowCleanup ablation

**Retain the dispatch-retention repair. Performance is inconclusive; these
measurements do not justify removing cleanup or changing its selection policy.**

The experiment used fixed base `c3e8a3559d5e718716eb62824df23bf759fecbe5`
on Windows x86-64. It is one bounded user-requested experiment, not release
qualification or closure of the broader dynamic-dispatch task.

## Correctness result

Enabling `ControlFlowCleanup` on the unchanged
`dyn_dispatch_selector_payload_bound/local/main.omg` wrongly refused with
`native execution scope reaches absent machine MachineId(2)`.
The same producer and program compiled with the pass disabled; its executable
exited 0. The strengthened match witness independently exposed an absent
`MachineId(3)` on the original producer.

Cleanup retained direct-call callees but omitted the machines named by the
published dynamic-dispatch tables. Those tables remain in the module, so every
present row must retain its callee. The nine-line compiler repair adds table
rows to the existing retention roots; the existing traversal then retains their
transitive callees. It neither bypasses checking nor invents a new execution path.

The existing fact root, nine places and their pinned positive/reject fixtures now
explicitly select cleanup. The match place exercises two different implementation
results (38 and 48), including a helper called by the second implementation,
and checks both selector paths. The public bound and consuming subtraction
remain `result <= 88u64` and `88u64 - result`; the reject twin weakens the bound
to 89 and rejects at that subtraction.

Fresh repaired-producer observations:

- Root plus all nine places: native exit 0 and Terminal exit 0 on Windows x86-64.
- All nine reject twins: nonzero checking exit with the intended `exact arithmetic`
  / `may overflow u64` diagnostic, not a package or missing-machine error.
- Pinned fixture bodies are byte-identical to their sample counterparts; Build
  files differ only in the application label. The focused corpus `--both` gate
  separately executed all nine positives and reported clean. The checked corpus
  gate rejected all nine negatives with their expected fragments. Its golden
  comparison found two diagnostic-location moves: the strengthened match witness
  (recorded only that row), and inherited nested-machine naming drift in
  `other_state` (confirmed with the frozen original compiler; left unchanged).
- Affected crate Clippy, Rust formatting, Python compilation, and `git diff --check`
  passed. The experiment also exercises failure and missing-artifact guards.
- Seven distinct performance configurations passed `omega run --both`: cleanup
  on/off for length, dispatch and stack32, plus the unselected control. The
  control's A/B rollback requests both have the same empty effective selection.

[Dispatch observations](evidence/dispatch-validation.json),
[corrected setup retries](evidence/runtime-retries.json),
[performance-engine observations](evidence/performance-quality.json),
[Clippy](evidence/clippy.log), [native/Terminal corpus verdict](evidence/corpus-result.json),
[reject corpus verdict](evidence/corpus-reject-result.json),
[scoped golden update](evidence/corpus-record-result.json).

## Timing result

One warmup per variant, then ABBAABBA: four measured observations per variant
per subject. A keeps cleanup selected; B removes it with the exact CLI rollback.
A no-op `CopyPropagation` disable makes both commands print their actual selection
receipt. Detailed phase profiling runs separately after timing. Each compile
uses a fresh output directory; all 48 warmup, measured and profile compiles
produced an executable that actually exited 0. Source bytes and producer bytes
were checked for stability.

| Subject | On median ms (range) | Off median ms (range) | Off vs on |
| --- | ---: | ---: | ---: |
| length/local | 6,314.9 (6,156.0-6,347.2) | 6,208.2 (5,852.8-6,228.5) | -1.69% |
| dyn_dispatch_selector_payload_bound/local | 5,796.3 (5,124.7-6,914.4) | 5,328.1 (4,665.3-6,040.6) | -8.08% |
| thirtytwo_arg_stack_calling_plan/other_machine | 12,334.9 (11,621.0-14,124.8) | 12,683.7 (11,504.0-13,198.1) | +2.83% |
| length/local, cleanup absent in both (control) | 5,216.4 (5,040.9-5,505.4) | 5,336.5 (4,949.9-5,606.2) | +2.30% |

Each subject's complete native executable is byte-identical across both variants
and every observation, so no instruction or executable-size difference was found.
This supports native code equivalence for these witnesses, not general pass
inactivity or universal runtime equivalence.

The selected pre-Terminal optimization phase's median was only 0.194-0.558 ms;
with cleanup disabled it was 0.086-0.358 ms. Those direct costs are small beside
whole compilation. Removing a pass can still affect later work, so the phase
cost is not a bound on its total effect. The whole-command results vary by
workload, show substantial drift, and the no-op control itself shifts 2.30%.
Do not promote the favorable dispatch median to a compiler-wide speedup.

The next useful question is why checking dominates these runs. In separate
on-variant profiles, `Stage 05: TypedTrees -> CheckedTrees` took 1,612 ms on
length, 1,216 ms on dispatch, and 6,025 ms on stack32. Investigate one repeated
checking operation on the retained stack32 input before trying another optional
pass. This experiment stops here; it establishes no reduction in that work.

[All observations](evidence/runs.json), [summary and stability checks](evidence/summary.json),
[separate detailed profiles](evidence/profiles/).

## Producer and input provenance

- Baseline optimized dev producer SHA-256:
  `030867b795b1e1c84dad3cd58c46bff310fd75bc425995a68eee88a8efdc2b83`.
- Repaired optimized dev producer SHA-256:
  `64983946d8bccabbfa18efcc9a25e8339ff8a5aa6cc297d044b1454bcda4308d`.
- The repaired producer is the fixed base plus [compiler.patch](evidence/compiler.patch).
  The commit containing this record additionally carries sample/fixture and
  documentation changes. Compiler profiles, exact input hashes, commands and
  output hashes are in [metadata](evidence/metadata.json) and the raw observations.
- Baseline and repaired executables are retained locally under
  `build/pass-ablation/baseline/omega.exe` and `build/pass-producer/debug/omega.exe`.
  Native products and package locks remain local under `build/pass-ablation/`;
  they are not committed or transported as package acceptance authority.
- Claim `ded67518eb1945e19e9c6a3ddd885069` was confirmed by registry read-back
  after the original registration request timed out.

The timing witnesses copy their tracked `main.omg` bytes unchanged. Their
experimental Build files add only explicit cleanup selection; the control keeps
it absent. Timing variants use rollback rather than editing sources between runs.
The existing benchmark module supplies profile verification, parsing, hashing
and setup. Its authored-selection scanner recognizes old receiver syntax, so
this experiment validates the compiler's observed receipts directly.

## Excluded attempts and limits

- The initially selected recoverable-restoration and indexed-restoration
  witnesses fail on the fixed baseline before optimization, with missing
  conserved linear return-claim mappings. Their diagnostics are retained in
  [restoration-baseline.txt](evidence/restoration-baseline.txt) and
  [indexed-baseline.txt](evidence/indexed-baseline.txt). They supply no timings.
- The original producer's length observations and failed dispatch compile remain
  in [baseline-probes.json](evidence/baseline-probes.json). They are not mixed with
  the repaired-producer comparison. See the exact
  [enabled failure](evidence/dispatch-on-baseline.txt),
  [disabled compile](evidence/dispatch-off.txt), and
  [two-implementation failure](evidence/match-baseline.txt).
- An initial validation invocation supplied `run --target windows_x86_64`, which
  is compile-only even on this host. The guard rejected it as missing runtime
  evidence. A subsequent root run overlapped the preceding target-body setup
  and encountered a locked transaction file; the target-body run also needed
  ordinary package-review refresh after its Build edit. Both were rerun after
  setup completed. Raw failures remain in the observation ledger and are
  superseded only by the explicit successful retries.
- The corpus gate wrapper stalled after the runner was built, with no compiler
  child still active. That wrapper was stopped. A fresh cached `mbx test --no-run`
  completed with exit 0 in 2.66 seconds; its Cargo output was bound to the
  [runner profile receipt](evidence/corpus-runner-profile.json). Both focused gates
  then ran with that explicit runner. The inherited negative-golden drift is
  documented above and in [other-state-baseline.txt](evidence/other-state-baseline.txt);
  it is not attributed to the cleanup repair.
- No full corpus/workspace sweep, native execution on other hosts, or remote
  integration/landing is claimed. Timing runs are serialized but not a claim
  of an otherwise idle machine. The broader dynamic-dispatch task remains open.

## Replay

From an isolated checkout containing the repair, build the optimized producer
under the normal build-slot hold:

```text
python tools/build_slot.py -- mbx build -p omega --locked --target-dir build/pass-producer
python wiki/drafts/measurements/pass_ablation_2026_10_28/validate.py build/pass-producer/debug/omega.exe build/new-dispatch-validation
python wiki/drafts/measurements/pass_ablation_2026_10_28/run.py build/pass-producer/debug/omega.exe build/new-pass-ablation
```

Use new output directories. Existing accepted projects whose Build source changed
need the normal `omega update --project <directory> --target windows_x86_64`
review before execution. Runtime checks deliberately omit `--target`; a success
must contain both `native exit 0` and `interp exit: 0`.
