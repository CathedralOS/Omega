# Deferred completion table custody audit

At base 4c23cc3391c2e539d4065563ac95de7b18f2fc67, the only production caller
of `append_completed_body_tables` is `DeferredResolvedFrontier::complete_machine_bodies`.
It retains the predecessor Arc and `seed_body_lowerer` deep-clones that exact
carrier. `seed_resolved_base` validates source custody, builds resolution
sidecars, then installs the carrier without altering its normalized tables.

Generated declarations and selected bodies append declaration storage and
expression graphs. `settle_resolution` resolves constants, completes receipt
helper demand, assigns symbols and selections and settles constraints. They
operate on declaration children; normalized type-reference rows are not written.
Normalized statement writes in this resolver are confined to `body_calls` and
`recursive_tail_calls`, reached by `finish_resolved_tables` after the append.

The optional machine-equation loop uses separate provisional lowerers and full
normalization. Its result informs generated syntax; the final lowerer is seeded
anew from the retained predecessor. A provisional normalized table cannot leak
into the final successor. Prepared allocation joins change symbols, domains and
service rows only. Earlier completed bodies and their routed statement rows are
therefore already present in the successor when append normalization begins.

The previous append normalizer cloned predecessor statement/type tables,
appended selected/new declarations, then replaced the successor's existing
copies. Transfer those existing owned copies with `mem::take`. The original
predecessor remains shared and immutable. Expression handling, declaration
frontiers, machine identity selection, append order, final routing and failure
custody remain as before. No new semantic cache, conditional fallback, syntax
storage change or shared-arena change is needed.

Research reuses ../normalization/research.md and its primary Rust `mem::take`
reference. This is the same owned-field transfer mechanism; no new research
question or changed API requires repeating that search.

Reject the candidate if either retained table differs before append, or if the
complete normalized carrier differs from the frozen clone-based normalizer.
Diagnostics compare all fields via existing carrier equality (with the repository's existing equality semantics; see equality-limits.md). Native/Terminal observations and exact refusal
outputs independently check the real consumers. Counts are live main rows,
not allocation bytes, RAM or whole-compiler time.
