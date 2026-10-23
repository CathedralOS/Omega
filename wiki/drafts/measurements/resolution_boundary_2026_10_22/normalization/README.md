# Resolved table normalization, 2026-10-22

The measured compiler copied its full resolved expression arena before rebuilding
normalized statements/types, then discarded the original arena. Move that arena
and use the existing same-table subtree copier for the expressions embedded in
types. This keeps the complete resolver and placed-result discovery. No call
sites, policy invocation sources, or declaration selections are filtered out.

This follows the rejected [discovery bypass](../placed_discovery/README.md):
that bypass lost placement-admission inputs. The retained mechanism here removes
storage copying without removing semantic work. It is independent of the exact
syntax produced by layer 0. It does not close ITERATION-PERFORMANCE or establish
that other boundaries are efficient.

Implementation: extend the expression copier's existing source abstraction into
statement/type normalization, transfer the owned arena with `mem::take`, and
keep statement-expression handles intact. Constraint and const-expression
subtrees still receive fresh copies in the same append order. Deferred completion
continues to seed predecessor statement/type tables and preserve retained
frontiers. Exact-predecessor and transactional rebase copies remain untouched.

## Acceptance and provenance

Base: `b8a183529f4af01bb76405f96f431b6dd086b79d` (evidence-only commits on compiler
source base `114906953e11e283b8082f03d8da99fbf39dff7e`). Baseline executable is
unchanged from the parent investigation. Candidate and diagnostic binaries are
frozen under `build/resolved-normalization/` with digest-bound effective opt-level
receipts. The recorded candidate patch binds the implementation. Do not archive
the worktree without retaining these ignored executables.

Research: [ownership transfer](research.md). Reproduction from the worktree root:

```sh
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/normalization/queue.py
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/normalization/capture.py candidate
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/normalization/queue.py build_diagnostic.py
python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/normalization/capture.py instrumented
```

Use a fresh output directory or preserve/verify existing immutable producer
artifacts before replay. Scripts require the recorded source base and existing
baseline executable/profile evidence. The diagnostic generator reads the old
normalizer directly from Git and compares complete `SymbolResolvedTrees` equality
after both full normalization and deferred-body normalization. Its extra cloning
and comparisons are excluded from timing claims. It restores production source
after freezing the diagnostic executable.

The checked input set has five positives: length, dynamic dispatch, nested record,
returned placed view and forwarded placed view. The five invalid controls pin
index bounds, wrong borrow backing, overlapping loans, exact policy ownership and
source-access authority. No fixture or authored oracle is changed. A compiler
exit is not counted as execution: each positive native image is executed and
candidate `run --both` separately observes native and Terminal exits.

## Retained result

- All 57 complete-carrier comparisons passed across the ten inputs. This checks
  expression/statement/type rows and handles, declaration and invocation source
  identities, selection ledgers and the rest of the retained carrier, beyond
  native byte equality. The old normalizer's source hash is in the summary.
- All 24 representation-crate tests passed, including the new nested constrained
  and const-expression normalization comparison and the existing every-node
  subtree-copy/occurrence tests. Affected crate check, formatting and production
  patch whitespace checks passed. Existing unrelated warnings remain.
- 30 compile/reject observations, 15 standalone native executions (all exit 0),
  and five candidate dual-engine observations (native 0, Terminal 0). Every
  positive native product is byte-identical across baseline, candidate and
  diagnostic producers. All five invalid diagnostics are identical after
  removing only timing and oracle lines. No positivity or rejection was inferred
  from publication alone.

| Original workload | Whole-arena copies before → after | Copied expression rows before → after | Removed rows |
| --- | ---: | ---: | ---: |
| Length | 8 → 4 | 34,630 → 17,300 | 17,330 |
| Dynamic dispatch | 8 → 4 | 34,572 → 17,278 | 17,294 |
| Nested record | 8 → 4 | 34,793 → 17,377 | 17,416 |

The baseline counts come from the parent trace; the candidate's oracle observed
exactly those four normalization invocations and their original row counts.
Source inspection confirms those whole-table clones were removed and no new
production clone site was introduced. These are copied-row counts, not bytes or
RSS. The other four full copies remain for their existing ownership contracts.

No end-to-end speedup is established. There is one alternating baseline/candidate
observation per workload; load averages ranged roughly 22–35. Oracle timings
include intentionally expensive comparison work. This is a deterministic
reduction of redundant storage work with preserved behavior, not a claim that it
is the dominant remaining cost. Narrower call-result selection remains a separate
unresolved opportunity; the rejected bypass is not restored.

Evidence: [summary](summary.json), [candidate observations](candidate-observations.json),
[full-carrier oracle observations](instrumented-observations.json),
[crate tests](tests.log), [environment](environment.json),
[candidate receipt](candidate-producer.json), [diagnostic receipt](instrumented-producer.json),
[implementation patch](candidate.patch), [temporary oracle patch](instrumentation.patch).
The diagnostic generator also includes the baseline normalizer source directly
from the recorded commit. No instrumentation remains in production source.

Freshly fetched main was `47676d7c9bb24827ddd09dd7d3b7bbc2034c62db`; its
symbol-resolved representation subtree had no delta from this source base.
Other compiler changes are not qualified by this experiment. This is a local
candidate, not a current-main/full-baseline/cross-host acceptance claim. Native
execution was on macOS arm64. No fixture, library or semantic rule was changed;
no language capability acceptance is subtracted. Claim:
`5c92481a31c942b5a6d86df51e7ef638`.
