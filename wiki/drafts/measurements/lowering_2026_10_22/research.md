# Lowering query scope, 2026-10-22

Question: can conformance retention avoid constructing the full live application
census when its emitted owner roster has no candidate occurrence, and reuse the
same census when it does?

Prior evidence: ../c2l_conjunct_lowering_cliff.md refers to removed code and is
not applicable. ../stage_by_stage_optimization.md and the source-assembly
records measure checking under the historical Stage 05 label, not this
terminal-production/lowering scope. Fresh attribution is in attribution.txt.

Search: `site.llvm.org docs LazyCallGraph reachable functions lazy analysis`.
Read the actual [LLVM LazyCallGraph header](https://llvm.org/docs/doxygen/LazyCallGraph_8h_source.html),
class documentation and Node population comments (source lines 63–109,
294–312, 1271–1283). Its analysis creates entry points first and discovers
edges on demand. This supports demand-driven query preparation as a candidate;
it does not justify changing Omega's liveness or establish a local speedup.
LLVM's SCC management and mutation machinery do not apply to this immutable
one-shot query. No cache, SCC algorithm or new dependency graph is introduced.

Experiment: preserve specialization/occurrence/trait-argument lane order, filter
exact owner handles before expensive occurrence liveness, retain the existing
live-source traversal whenever candidate occurrences exist, and feed the census
once into lowering. Arena candidates never establish liveness. Compare native
and Terminal behavior, canonical Terminal bytes, rejection diagnostics, and
alternating complete CLI invocations. Reject on output differences; report
lowering time separately from end-to-end checking cost.
