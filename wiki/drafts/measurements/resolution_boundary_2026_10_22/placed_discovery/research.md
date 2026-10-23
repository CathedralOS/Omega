# Placed-result discovery research, 2026-10-22

Question: can a pre-resolution consumer request selected call-result declarations
without rebuilding all machine bodies? Prior local evidence is the parent
resolution-boundary record. The nearby source-assembly probe research concerns
parser lookahead and does not establish the resolver contract.

Query: `site.rustc-dev-guide.rust-lang.org queries demand driven compilation lazy`.
Read [Queries: demand-driven compilation](https://rustc-dev-guide.rust-lang.org/query.html),
particularly the typed `DefId` query example, dependency discovery, and result
ownership sections. It illustrates requesting a specific semantic result and
tracking its dependencies. It does not establish Omega semantics or justify
introducing a query cache here. No cache, query engine, or new invalidation
protocol is proposed.

Local trace: `retain_applied_results` consumes exact resolved call targets, static
actuals, and return type declarations, including sealed borrowed View validation.
A cheap name-only filter could lose aliases, wrappers, or prototype rejection.
First experiment: count actual selected outputs and applications, then bypass
only this discovery in a diagnostic binary. Compare original fact workloads and
placed-view positives/invalid controls. Any missing demand or changed required
rejection disqualifies unconditional removal, even if ordinary workloads improve.
The bypass is diagnostic and must never remain in production.
