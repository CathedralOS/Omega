# Project locks

The lock is retained project intent, not a certificate of package safety.
[Acceptance](acceptance.md) owns review and decision authority;
[source selection](sources.md) owns resolution and immutable graph construction.

## Retained state

A lock contains source-qualified package keys, explicit root role, exact immutable
commit/tree/content and member selections, source requests, requester-local alias
edges, complete normalized accepted policy, and explicit project decisions.
Policy includes capability, public API, and assumption meaning, scoped to each
actually reviewed target where necessary. A digest alone cannot explain a change.

Use bounded deterministic diffable text with explicit outer and baseline schema
versions. Loading requires neither an old checkout nor a compiler invocation.
Unknown formats or unrecoverable baseline meaning reject with recovery guidance;
they never become empty acceptance or an inferred schema migration. Cache paths,
proof certificates, native artifacts, replay transcripts, audit receipts, and
compiler-private handles are not this payload. The
[lock codec](../../../omega-rust/omega/packages/manager/src/lock/README.md) owns
the exact framing and resource limits.

Projects normally commit the lock and trust whoever lands it. Recovery checks
format, graph consistency, and acquired content against pins, not a certificate
that its author reviewed carefully. Fresh compiler findings still compare against
retained policy. Regenerating resolutions cannot invent an approval; unchanged
accepted policy needs no second native acceptance channel.

## Source and control state

Root `omega.lock` and `omega.admissions` in a mutable local package are project
control state, excluded from local source capture including ASCII case variants.
Publishing policy cannot change the source identity recorded inside it. Symlinks
into excluded root control files reject. Nested policy-named files remain ordinary
source; exact materialized repository trees retain all their files, including
locks. The manager loads acceptance separately, so this exclusion never hides an
edit to accepted policy.

## Exact target sections and locked use

One lock may share its target-independent immutable source closure across
independently accepted sections for explicit requested targets. It has no support
matrix or discovered `all` set. Sections retain canonical semantic target identity,
not a CLI alias or temporary enum ordinal. Another target's acceptance cannot
authorize the current child. Multi-target work sharing follows
[configuration](../build/configuration.md#staged-multi-target-execution).

Locked compilation never refreshes a mutable selector. It may acquire missing
content at the recorded pin when allowed; offline absence leaves the graph
unchanged. Select a recorded target before acquisition: missing target policy
fails without network access. Recomputing stale analysis neither upgrades pins
nor implies an audit. Changes between fresh findings and the retained baseline
remain visible and require the ordinary comparison/decision workflow.
