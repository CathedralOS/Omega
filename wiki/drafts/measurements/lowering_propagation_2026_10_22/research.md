# Lowering graph setup, 2026-10-22

Continue the questions and measurements in
`../lowering_2026_10_22/design.md`. Producer baseline is `60cb9c9da7f`;
its Rust source is identical to the frozen first-wave candidate at `0047571a3f1`.

The first attribution split the approximately 43 ms interval into place
reconciliation (7–40 microseconds), membership setup (44–45 ms), and one
262-operation sweep (0.8 ms, 17 proposals, no updated operation). A second
probe identifies descriptor graph construction as essentially all setup cost.
Its absolute times increased with host activity; these are within-run
attributions, not comparisons of implementations.

This rules out repeated fixed-point iterations as the first optimization on
the packet workload. It does **not** establish that graph construction is unused:
the initial counters did not count history queries, which proved material in
the first candidate. The design review's worklist proposal remains a hypothesis
for other workloads.

## Candidate 1: demand-driven graph construction (rejected for this task)

Query: `site.doc.rust-lang.org std cell OnceCell get_or_init borrow checking single assignment`.
Read the [standard-library OnceCell documentation](https://doc.rust-lang.org/std/cell/struct.OnceCell.html),
description, examples and `get_or_init` behavior (lines 63–87 and 180–201 in
the retrieved page). It supports one-time initialization followed by shared
borrows without runtime borrow checking; it does not establish that the graph
is unnecessary or validate which snapshot is retained. The earlier LLVM
LazyCallGraph research remains applicable to demand-driven preparation in
principle, but its mutation/SCC machinery is not needed here.

Implementation prepared the existing complete graph on the first descriptor
or payload-history query. The candidate retained graph and descriptor analysis
for the invocation, while payload histories still died before each membership
update sweep. `lazy_graph/candidate.patch` preserves the implementation, which
was removed from production before the next experiment.

Sixteen alternating pairs per completed subject show packet lowering median
71.764 → 75.698 ms, with means 74.363 → 76.985 ms and minima
69.657 → 74.209 ms. The program does require a shared-reference history query,
so this change does not remove its expensive graph construction. Small witnesses
improved modestly, but this fails the chosen workload's acceptance. No semantic
acceptance or broad correctness claim is made for this rejected candidate.

An attempted fourth timing subject, `runtime_forwarded_view_field_domain_exit`,
refused package entry-binding admission before lowering. The raw refusal is
retained; no lock or package acceptance was changed. The remaining pairs used
the existing standalone `runtime_copied_range_record_domains_exit` consumer.
No completed measurements were rerun during this resume.

The rejected executable is preserved locally as
`build/lowering-propagation/omega-lazy-graph`; its digest is the candidate digest
in `lazy_graph/timings.json`. The original candidate path in those immutable
command records names the path used at measurement time.

## Candidate 2: prepare the scalar namespace once

The next probe attributes graph construction by machine and scalar-expression
query. The main graph costs 46.1–53.8 ms. In each of three runs, 293–371 scalar
expression calls individually exceed 50 microseconds; their cumulative time is
18.5–27.2 ms. Faster calls are not logged, so that is a partial attribution, not
the full scalar cost. Postcondition handling costs only 67–92 microseconds.
The main graph dominates the other two machines' few microseconds of setup.

Source and ARM64 disassembly show the same machine value-type table being
collected into a BTreeMap inside every scalar-expression query. This table is
invariant throughout graph adaptation. The second candidate prepares it once
in `TerminalScalarExpressions`, which borrows the immutable machine, then lends
it to every stored operand, edge predicate, and call-postcondition query.
The collection order, duplicate-ID behavior, origin lookup, literal recovery,
proof-bearing meaning, failure behavior, and expansion budgets are unchanged.
No expression-result cache or new index is introduced. This is direct invariant
preparation based on the measured code; no additional external algorithm is
needed. The graph remains eager and the rejected OnceCell context is absent.

The compiler emitted map construction in the old query routine; the new
adapter constructs it at entry and retains the calls to expression expansion.
See `assembly.json`. The full measured comparison and quality result are in
`README.md`. The next question is the remaining expression-origin lookup and
expansion cost, not more fixed-point scheduling on this one-sweep workload.
