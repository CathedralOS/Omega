/*
Opt-in compilation phase timings and process-wide activity counters.

We separate two diagnostic measurement mechanisms from compilation semantics:
1. `CompileTimings` (`compile_timings`): per-invocation phase durations across
   the Psi and Omega stage ladder. Collection is disabled by default so
   unmeasured builds avoid `Instant::now()` calls in subordinate passes; when
   enabled, the CLI formats and emits the timing ladder to stderr.
2. Activity counters (`compile_timings::counters`): relaxed process-wide atomic
   totals that count frontend reuses, settled checks, and
   Build activations for review gates and regression tests.

Trust records and artifact validation belong to realization's trust model, not
this host measurement crate.
*/

pub mod compile_timings;
