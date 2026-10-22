# Float spelling ownership research — 2026-10-22

Reuse the [retained syntax ownership investigation](../retained_syntax/research.md) and [page/transaction investigation](../ingress_ownership/research.md). The question is unchanged: source-backed syntax carries a page handle and validated token range while the forest owns the source. No new external search was needed to apply the already researched boundary to the remaining float payload.

Baseline census: 463 source files, 456 successful parser forests, 607 retained source references. The 151 references beyond one per forest come from authored float spellings. A float spelling is not yet a numeric value: lexical validity and later exact numeric admission must remain separate. Generated negative and compile-time-materialized float spellings can differ from their diagnostic source span, so they retain owned text.

Hypothesis: forest custody eliminates per-occurrence reference-count increments/decrements without changing source lifetime, exact spelling, source spans, parser diagnostics, numeric landing, or serialization. Generated boxed spelling avoids the redundant Arc allocation but can copy bytes when explicitly cloned; measure the net allocation effect rather than claiming all generated copying disappears. Reject the change if complete syntax/diagnostics differ or source ownership escapes its forest.

Use the same 463-file allocation probe, a separate prelexed parser repetition harness, focused ownership/copy tests, and actual float native/interpreter plus invalid controls. Compare parsed and generated literal consumers, including constant materialization and generic numeric admission. No whole-compile or physical-memory improvement follows merely from the ownership census.
