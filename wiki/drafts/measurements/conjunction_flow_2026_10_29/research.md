# Conjunction projection: next measured target

Continue ITERATION-PERFORMANCE after PR760. Fixed source base `894c94a2eaef8c2e8c362b7282d017f5d53e5516`, verified published by
PR760. Historical measurements remain tied to their producer.

The rejected return-bound memo showed six costly first computations (103-118 ms
in its diagnostic), so extending cache lifetime is rejected. Inspect actual work
inside a projection instead. `project_possible` computes a left result, then
calls `condition_environment` on that entire prefix before visiting the right.
Nested conjunctions can therefore replay earlier prefixes. Measure complete
return projection, condition-environment preparation, and per-node visits before
choosing an implementation. Preserve the same fixed stack32 source and native
oracle; also use length/local and dispatch/match_join for regression comparisons.

Research 2026-10-29. Query: `site.clang.llvm.org docs DataFlowAnalysisIntro transfer
function flow condition short circuit`. Read the transfer-function, flow-condition
and invalidation discussion in https://clang.llvm.org/docs/DataFlowAnalysisIntro.html.
The relevant principle is carrying abstract state through ordered transfers while
joining alternatives and withdrawing facts after mutation. It suggests processing
a conjunction's operands in order rather than replaying growing prefixes; it does
not prove Omega equivalence or predict performance. No Clang dependency or new
analysis framework is proposed. Static folded expressions, prepared mathematical
applications, disjunctions, impossible conditions and partial-operand formation
must retain their existing handling. Reuse the current condition evaluator for
all effects and guards. Reject a candidate if those outcomes differ.

A prospective local worklist would flatten only ordinary conjunction structure,
carry the existing environment forward between operands, and keep other nodes
atomic. Attribution and the selected implementation are recorded below.
Stop this bounded continuation after the next supported optimization
and its focused checks, or reject it with evidence and take the next measured
opportunity if this target is too small.

## Attribution and selected implementation

At fixed base 894c94a2..., the clean optimized baseline compiles in 3516 ms and
runs natively at zero. Its image is now 81408 bytes, SHA256 ad8b1cc4..., reflecting
other landed backend changes; do not compare it as the old 114176-byte image.
The diagnostic preserves this new exact image and reports 33 return projections
at 911.669 ms, including 6054 condition-environment constructions at 891.028 ms.
The recursive project-node scope sums to 9276.972 ms inclusively and must not be
added to wall time. The environment work is a substantial measured target.

Candidate: walk the ordinary conjunction's left spine, then project its operands
in the same order while carrying the existing condition environment forward.
Folded predicates and prepared mathematical applications stay atomic; disjunction
projection is unchanged. This follows the existing true-conjunction transfer
T(a && b, E) = T(b, T(a, E)); the evaluator's effect invalidation and impossible
interval checks remain in place. No cache, context lifetime, caller actual,
proof admission, selected operator, or public interface changes.

The shared projector's callers include ordinary scalar return bounds and
mutable-referent bounds. Exercise both existing call/collection controls and
short-circuit mutation/formation controls. Add a nested left/right conjunction
with an atomic folded true pair and distinct scalar actuals; its reject twin
changes only the second actual (9 to 8) while consuming exact subtraction by 9.
Record the baseline diagnostic before implementing. Screen whole-compile and
statement cost on the fixed source; preserve all observations and require exact
native image equality. If supported, run the three-subject unprofiled comparison.

## Semantic review

The original recursive projection for `(a && b) && c` projects `a`, transfers
`a` into its environment, projects `b`, then transfers the entire `a && b`
prefix from the original environment before projecting `c`. The candidate
retains the same projection order and interval intersections while computing
the second transfer from the environment already produced by `a`.
`guard_narrowing::evaluated_condition_with_frames` implements positive ordinary
conjunction as exactly those ordered transfers. It owns call-write invalidation
and alternative-path joins. No operand is reordered or evaluated speculatively.

Only the left spine is flattened. Right-nested conjunctions recurse through the
same projector; disjunctions, static Boolean expressions and prepared mathematical
applications remain atomic. Each intermediate environment is still checked for
an impossible interval before projecting the next operand. A missing projection
still exits early. Scalar interval intersection is associative here.

Creating a condition evaluator per operand does not discard semantic history:
`CallFrameResolver::new` reads frozen-program tables and memoized write answers;
the transferred `ValueEnvironment` owns path-dependent facts. Call-free operands
never consult that resolver, and a failed resolver construction retains the same
conservative unknown-write behavior. The focused controls test actual identity,
mutable storage, short-circuit paths, formation and disjunctive return bounds.

The resumed session confirmed PR760's post-merge smoke run 40841499791 succeeded.
The previous candidate build was interrupted before producing a binary; no
timing observation from that attempt is used. The resumed build queues through
the shared build slots, leaving other worktree builds running.

## Build-state recovery

The resumed build reported E0786 corrupt metadata for validation, provider
planning, selected dispatch, compilation products and package compilation, plus
a missing representation-planning crate. The validation metadata's first 32 bytes
were all zero. A package-scoped Cargo clean removed the outputs, but mbx restored
the corrupt validation metadata on retry. Neither attempt produced a candidate.

Research query (2026-10-29): `mr boxington mbx disable cache environment variable`.
Read the official [configuration](https://mr-boxington.jdx.dev/configuration)
entries for cache_dir, incremental and learned_incremental, and the feature
switches in the [FAQ](https://mr-boxington.jdx.dev/faq). Used a process-local fresh
MBX_CACHE_DIR and MBX_LEARNED_INCREMENTAL=0 for recovery; no global configuration
or other worktree artifacts were changed. Cleaned only the seven affected
packages in this task's target directory. This is infrastructure recovery, not
an optimization to publish.

Because learned incremental compilation can change how the producer is built,
rebuild the original source with those same settings before the decisive A/B.
Comparisons against the earlier producer remain preliminary, not the retained
performance claim. Retain the first two failed build logs and the exact matched
build inputs alongside the measurements. Focused source-oracle validation still
uses the original verified baseline and the actual candidate binary.
