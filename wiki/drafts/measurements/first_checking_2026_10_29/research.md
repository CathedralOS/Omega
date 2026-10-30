# First-time checking: numeric projection research

2026-10-29. Fixed base 2a087aa3ec325cfab27afae8f51b2f358b3bfc1d.
The prior convergence repair completes the original package. This experiment
starts with that repaired optimized producer, never the pre-convergence hang.
The unchanged operand-order fresh-cache update completes in 221.85 s under
OMEGA_PHASE_PROFILE. Inclusive association proof time is 104.14 s; shared
integer comparison projection is 79.95 s across 2110 calls. Overlapping scopes
are not independent elapsed costs. The target-only run request compiles successfully; it does not execute either engine.

Reviewed the existing contract_preparation_2026_10_28/research.md query research.
The changed question is whether repeated structural operand preparation can be
reused without erasing observation authority or changing active recursion.
Query: `site.rustc-dev-guide.rust-lang.org query evaluation pure memoization context cycles`.
Read https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html,
including input data, caching and cycles. Pure providers and exact immutable
inputs permit reuse; a cycle-limited answer depends on the active computation.
Do not import rustc's cycle error policy or add a general query framework.

Local review: StructuralTerm semantic equality ignores some observation/type
payload. A general resolver cache keyed by that equality would be unsound.
The existing same_resolution_payload comparison retains those fields, but
active application context also matters. No general memo is selected.
A diagnostic separates integer_value's completed observation, captured-value
identity expansion, and normal resolution before selecting a concrete change.
See diagnostic.patch and diagnostic.json. Timers are diagnostic only.

The first diagnostic completes first update in 187.22 s and target-only compilation (actual host comparison is recorded separately). It records 2,976,403 integer_value entries. Captured identity costs
1.62 s inclusive; normal resolution costs 186.14 s inclusive across 2,725,298
calls (nested contexts overlap; this exceeds any isolated elapsed saving).
Captured-equation orientation is therefore not the selected target. A second
diagnostic splits resolution by incoming semantic term kind. Do not compare
instrumented 221.85/187.22 s runs as a speedup: these contain different timers
and shared-host variation, and neither changes compiler behavior.

Prospective mechanism, pending the second profile: share only completed
normal-resolution answers within one immutable prepared_predicate_proofs batch.
Exact same_resolution_payload is mandatory; active application/premise context
must remain that of the one borrowed judge, and answers must be discarded before
scratch is returned to PreparedIntegerPremises. Arithmetic registrations,
operation formation, goal namespaces and fallback proofs must still be rebuilt
per goal. Reuse would remove repeated work, not add an equality or premise.

Candidate scope: integer_predicates.rs only. All six fresh IntegerObservations
constructors disable operand answer reuse. One prepared_predicate_proofs call
enables it after borrowing its scratch; reset_goal_work deliberately retains
answers between goals under that same immutable judge, and put_goal_work drops
them before storing reusable scratch. Plain/fallback/premise queries remain
uncached. resolve_integer_operand keys by full retained payload and calls the
unchanged depth-zero resolver; primitive-specific registration still occurs in
integer_value for every use. Nested resolver queries own separate observations.
The fork used only for arithmetic admission may clone these answers but cannot
publish them or call a different judge through them. No language/API/config/
exports/fixture changes are needed. The unit test exercises metadata-sensitive
answers and actual scratch reuse across different substitutions.

Before comparison: A is the verified 5a596bca convergence producer; B is the
same fixed compiler source plus candidate.patch. Decisive A-B-B-A comparison:
ordinary first package update with fresh LOCALAPPDATA and original input files,
then ordinary review/resume, compile, native execution and separate --both.
No phase profiler during timings. All resulting native images must be identical.
Retain a repeatable useful first-update gain (working threshold 15 percent)
with no behavior loss on focused controls. Overlapping/mixed ranges are not a
win. The secondary original single-evaluation program and the existing contract,
reference, copied-snapshot and six refusal controls gate behavior. Keep all
valid trials; an inconclusive result removes the candidate and records it.

The second diagnostic confirms selected application resolution dominates:
622820 application entries account for 269.70 s inclusive; integer literals
(1.38 million calls) cost 75.5 ms, projections 174.9 ms, captured leaves 3.0 ms
and variables 6.5 ms. First update finishes in 261.94 s with resume and target-only compilation success. Shared-host/profile variation remains substantial. This supports
screening reuse of expensive repeated application answers, not optimizing
literal lookup or captured-equation orientation. Both diagnostic producers and
patches are retained; neither is used as timing baseline A or candidate B.


Harness correction found during raw evidence review: explicit --target makes
run --both compile-only. Historical native executions invoked the produced exe
separately and remain valid, but these target-only calls are not Terminal
observations. The timing records retain their original argv unchanged; host.py
replays both producers on every timing project without --target and requires
actual `interp exit: 0` and native status text. quality.py now does the same.
The prior convergence PR receives its own nine corrected host replays and a
visible evidence correction; no old log is relabeled as runtime observation.
First-update timing and separate native image equality are unaffected.
