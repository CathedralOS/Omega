# Research: bounded cold symbolic work

2026-10-22. Reused the [design review's research](../design_review.md#research-and-decision)
before searching. New question: after measuring no prior Product judgment to
reuse, can one immutable relation set support compact, reusable graph results
without changing exact value identity or finite arithmetic precision?

Exact queries:

- `site.rustc-dev-guide.rust-lang.org dataflow analysis framework basic blocks indices`
- `site.llvm.org Programmer's Manual SmallVector dense data structures graph`

Read the [Rust compiler guide dataflow page](https://rustc-dev-guide.rust-lang.org/mir/dataflow.html),
including effects, per-edge effects and consuming results with cursors/visitors.
It separates computing facts from reading them repeatedly. Applicable here only
as a compute-once/read-many design cue: Omega's existing relation order, depth
limit and checked-overflow precision remain authoritative. No rustc framework,
lattice, new fixed point, or changed traversal policy was adopted.

Opened [LLVM Programmer's Manual](https://llvm.org/docs/ProgrammersManual.html#a-sorted-vector).
The returned excerpt was the top of the manual, not sufficient evidence for a
specific graph/container optimization. No LLVM container claim or library was
introduced. Contiguous Rust vectors with numeric edges were tested directly;
the local oracle and CLI measurements decide retention.

Local source review traced `same_value` into recursive constructor matching and
`ordered_gaps_by`, then its callers in interval refinement, subtraction floors,
delivered ceilings and live integer constraints. Governing fact validity and
mathematical values rules require exact observation/effect identity and scoped
fact lifetime. A long-lived equality cache or printed-name interning would not
satisfy those requirements. The experiment therefore borrows one immutable
relation slice and changes only exact leaf storage/reuse.

The [experiment](README.md) retained the implementation after independent old-walk
oracle tests, selected source/native controls and ordinary paired CLI gains.
This resolved the bounded question, so no additional source collection or
speculative representation rewrite was pursued.
