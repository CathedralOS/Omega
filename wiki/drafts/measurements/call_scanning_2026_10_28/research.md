# Value-position expression scanning

2026-10-28. Continue ITERATION-PERFORMANCE after
[statement validation](../statement_validation_2026_10_28/README.md).
Fixed source base: `ba621694ff483f2b66c2e8343aac4d8b24098581`, submitted
as [PR 750](https://github.com/CathedralOS/Omega/pull/750). The PR remains pinned;
this next local experiment does not update it. The existing detached worktree
retains its warm builds under the repository's ongoing-outcome rule.

The previous candidate's value-position-call scope has a 569.502 ms median on
the unchanged stack32 witness. It scans more than calls: names, fields, binary
operand types, short-circuit conditions, casts, indexed access and call bounds.
The next diagnostic separates the name/member prelude, call bounds, binary type
checks and short-circuit environment preparation without skipping validation.
Inclusive helper scopes can nest and must not be summed as disjoint process time.

Reuse the prior [query research](../checking_simplification_2026_10_28/research.md)
and [lifetime discussion](../statement_validation_2026_10_28/research.md).
No new search yet: the measured inner operation has not been selected. Any
candidate must preserve evaluation order, selected operator meaning, exact
call occurrence and invalidation of mutable-storage facts. An apparently
call-free expression can still need bounds for indexing/casts and cannot simply
bypass this scanner.

Baseline producer: `build/statement-loop/candidate/omega.exe`, SHA-256
`67c79dda5d51d0034320ae0244b957a041794c751b69f07cdd42a08d453702b3`.
It is the exact optimized producer tested before committing this source head;
only experiment records were finalized after the build. Diagnostic changes
are in [diagnostic.patch](diagnostic.patch), a zero-context diff replayed with
`git apply --unidiff-zero`; reverse application against the instrumented source
was checked before running it.


## Selected experiment

The first profile measures 517.653 ms in call-bound validation out of 578.157 ms
in value-position scanning. The finer profile measures 484.386 ms in
`reject_refuted_value_call_requires` (399 calls) out of 536.922 ms in call bounds.
The scopes are inclusive. Both diagnostic compilations produced the same PE and
native exit 0. [detail.patch](detail.patch) records the temporary instrumentation;
it is absent from the candidate.

Hypothesis: gathering the existing ordered list of callee requirement expressions
before preparing argument terms and the closed-operand judge avoids unnecessary
work when that list is empty. The only diagnostic emission in this function is
inside the loop over this same list. Argument count, type, access and formation
checks remain in its caller. Nonempty lists retain the existing argument map,
closed-operand judgment, diagnostic text and order. No cross-call cache is added.
The existing demand-driven query research applies directly: when there is no
query to judge, skip engine preparation. No new framework or algorithm is needed.

Scope: the one shared refutation helper and existing call-site controls; no caller,
syntax, fixture oracle, configuration or public interface changes. The sole caller
is `validate_value_call_arguments`; its independent validation remains unchanged.
The shared `argument_term_map` and positive proof route are untouched.

Screen against the retained exact-source baseline using six interleaved ABBAAB
profile observations on the unchanged stack-call witness. Reject a candidate that
changes results, diagnostics or emitted code. If scanner time does not show a
repeatable reduction, park the candidate without a whole-compile speedup claim.
If it does, run focused call controls and the existing three-subject unprofiled
comparison. All observations remain in the record, including regressions.

## Result

The six-run screen is inconclusive: scanner medians 697.609 -> 665.262 ms,
with overlapping ranges. The candidate was removed from production source;
its patch remains for replay. The planned wider validation was deliberately
not started after this screen. See [the result ledger](README.md) and
[raw observations](screen-runs.json). Next separate argument-term preparation,
judge construction and per-requirement judgment; this result does not justify
caching proof state or attributing the cost to the empty-list case.
