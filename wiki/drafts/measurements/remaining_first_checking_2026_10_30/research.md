# Remaining first-time checking

2026-10-30. ITERATION-PERFORMANCE, continuing the user's original operand-order
package-preparation workload. Fixed source checkpoint
0ac181bf065e398d6d6ae455c2659416895d6b49. Compiler inputs match the retained
operand-resolution producer 1c7645a93c087116c69bb5951cc940709cdadecfeae847d38f9de957d3ad4d60;
the final checkpoint adds documentation only. PR #849 publishes that iteration,
stacked on convergence PR #822. Its manually dispatched compiler admission run
40988721734 passed at the exact submitted head; the pull-request event only
runs automatically for a main base.

Read the preceding first_checking_2026_10_29/research.md and its three diagnostic
profiles. They locate selected-application normalization as the expensive operand
work, but precede the retained batch-local reuse. Refresh the phase profile on
that producer before assigning the remaining cost. All profiles use fresh
projects/cache roots and ordinary package update/review/resume. Actual host
comparisons omit --target and require both engine exit reports. Detailed timing
scopes are diagnostic observations, never A/B speedup evidence.

Changed research question: can resolution avoid proving a selected call's
requirements when there is no result equation to obtain? Query:
`site.llvm.org demand driven analysis lazy computation results unused compiler`.
Read LLVM's Passes documentation (lazy-value-info and memdep), then the Using
the New Pass Manager analysis/invalidation sections:

- https://llvm.org/docs/Passes.html#lazy-value-info-lazy-value-information-analysis
- https://llvm.org/docs/NewPassManager.html#using-analyses

These describe computing analyses when requested and maintaining exact validity
when results are reused. The transferable hypothesis is to avoid unnecessary
questions before optimizing their implementation. LLVM's pass framework and
cache are not appropriate additions to this local proof resolver. No speedup
claim transfers from those sources.

Local review rejects a blanket empty-ensures shortcut in declared_guarantees:
normal_returns.rs consumes successful completion to record a captured call's
denotational identity even if its authored equation list is empty. A changed
Some/None there could alter proof meaning. Boolean body-wide results and family
member intersections also supply equations without a direct authored ensures.
Any candidate must preserve those producers and unconditional formation checks.
In particular, wiki/spec/proofs/contracts.md requires selected preconditions
even when no callee conclusion is used. Skipping an unproductive normalization
query cannot replace the independent formation proof. The existing
authored_numeric_name_requires and closed_constructor_requires_restricted_nested_call_rejected
controls cover requirement failures without a useful authored result equation;
the scoped_integer_call_wrapping_actual_requires_formation control covers
formation inside a specification contract. These remain relevant even if a
candidate returns no normalization answer.

The diagnostic instruments normal-form/equation queries, guarantees with/without
expression ensures, their prerequisite proving, body-wide Boolean discovery,
body unfolding, and active-question bookkeeping. It changes no proof outcome.
Inclusive recursive scopes overlap: do not add their totals as elapsed savings.
The build uses this worktree's own target directory and mbx's shared object
cache; it must not overwrite another worktree's Cargo outputs.

Selection rule: choose a small behavior-preserving change only when the refreshed
profile shows material avoidable work. Compare the unchanged first update with
fresh independent caches in both A/B orders, profiler disabled. Retain only a
repeatable useful gain beyond the observed spread; require exact native images,
actual native/Terminal execution, and affected formation/guarantee refusal
controls. Reject or record an inconclusive candidate without broadening answer
lifetimes across mutable proof contexts.

Coordination: status was read; the registration attempt timed out and returned
unavailable (pending ticket 1a92afc0764b427fa2e71bf095de2b2b). No successful claim
is asserted. Scope is recorded here and in ignored continuation-claim.json.

## Diagnostic result and candidate, before A/B

The published operand-resolution producer completes fresh update in 144.56 s,
resume in 4.54 s and actual host comparison in 9.59 s. Detailed stage 05 scopes
total 128.80 s inclusive. The finer diagnostic completes update in 141.78 s,
resume in 4.52 s and actual host comparison in 8.55 s. Those differently
instrumented observations are not a speedup comparison.

The diagnostic records 224375 non-Boolean specification guarantee queries with
no expression ensures. Their prerequisite proving costs 28.84 s inclusive;
the whole guarantee query costs 28.95 s. In comparison, prerequisite proving
for declarations with expression ensures costs 0.345 s. Body-wide Boolean
discovery costs 0.011 s and active-question bookkeeping 1.326 s. Body unfolding
costs 128.05 s inclusive across 1.39 million entries; its recursive descendants
overlap other scopes, so it is not an independent saving to add to the rest.

Candidate: sixteen lines in declared_guarantees return no answer when the result
is a specification Application, the declared return carrier cannot supply the
Boolean-body equation, and no expression ensures can supply an authored equation.
The original implementation in that case returns either no guarantees when a
requirement fails, or empty equations/content when they pass. Both become no
normal-form rewrite. A family member with no equations also forces an empty
intersection; early absence cannot add a family equation. Content conservation
only runs for CapturedValue results, which this branch excludes. normal_returns.rs
passes captured results and therefore retains its original precondition checks
and successful completion needed for denotational identity. The other caller,
specification_application_equations, only consumes equations. Independent source
formation and the unchanged body unfolding path still check their obligations.
No new cache, resolver lifetime, syntax, fixture oracle, API or configuration.

Before timing, choose a ten-percent first-update mean improvement with separated
A/B ranges as the useful screening threshold. The 28.84 s inclusive cost suggests
an opportunity, not a guaranteed saving. A-B-B-A uses the verified published
operand-resolution producer as A and the same compiler source plus the candidate
as B, with profiler disabled. Preserve every valid trial. Native images must
match exactly; actual host comparison must report both engine exits. The prior
four positives and six refusals are supplemented with Boolean join and normal
return controls, a no-ensures restricted call, a specification formation refusal,
and conflicting/unknown Boolean return refusals. Reject the candidate if the
timing threshold or correctness checks fail; retain the diagnostic record.

The focused new specification_result_without_guarantees_requires_formation pair
pins the exact boundary, not a newly repaired language capability. Its callee
has a requires clause and no ensures. The positive establishes the prerequisite
before mentioning that result in an ensures clause and checks two runtime inputs.
The reject twin removes only the caller prerequisite. Both baseline and candidate
must accept/run the positive and reject the negative for that selected requires
obligation. Existing broad family/capture behavior stays on the general route;
no nine-place capability closure is claimed for this performance change.
