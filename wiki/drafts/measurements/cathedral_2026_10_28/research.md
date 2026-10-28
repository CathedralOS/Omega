# Shared compilation investigation, 2026-10-28

Question: can Cathedral's UI layout and frame products reuse checked dependency
work instead of reconstructing it for each product?

Local evidence: both Build roots depend on `../../../platform/native`, with
different provider selections and entry machines. Recorded process wall times
are 14.016 and 12.889 seconds. Shared source does not establish shared semantic
inputs or the amount of duplicated work. Existing nested timers do not account
for the whole critical path, and checking is not a proof-only timer.

Primary source read: [Rust incremental compilation in detail](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html),
direct URL open, no search-engine query. Read dependency tracking, red-green
invalidation, stable identities, and persistence costs. Reuse requires tracked
inputs; unchanged outputs can stop transitive recomputation. Fingerprinting
and serialization themselves cost time, so a persistent fine-grained cache is
not automatically a win.

Proposed experiment: first count repeated work and account for process wall
time for these two products, then prototype invocation-local reuse of the
largest repeated immutable result through the existing compiler drivers.
Preserve all required target-body checking and product-specific Build meaning.
Compare both artifacts and runtime oracles; alter a dependency contract to
verify stale results cannot survive. Reject this candidate if duplicate work
is small or the end-to-end saving is lost to retention overhead.

No prototype or measured reuse benefit yet. Persistence and a general query
engine are deferred until measured reuse justifies their costs. Cold full
composition and incremental edit latency remain separate acceptance metrics.
