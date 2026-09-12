# Project locks

The lock is retained project intent, not a certificate of package safety.
[Acceptance](acceptance.md) owns review and decision authority;
[source selection](sources.md) owns resolution and immutable graph construction.
Standalone compiler-policy file handling belongs to
[trust-ledger](../../../omega-rust/omega/build/trust-ledger/README.md), not this lock.

## Retained state

A lock contains source-qualified package keys, explicit root role, exact immutable
commit/tree/content and member selections, source requests, requester-local alias
edges, compact normalized acceptance policy, and explicit project decisions.
Each package retains exact canonical rows for admission-claim and external-
realization callables, external executable supplies, dangerous capabilities,
and supplied terminal permissions, scoped to each actually reviewed target.
Every retained row preserves its complete readable meaning, not only a digest.

The lock does not retain the complete public API, ordinary checked callables,
selected-provider tables, representation snapshots, source-semantic dependency
graphs, or symbolic-demand snapshots. Those findings are reconstructed by fresh
whole-candidate checking, not by compiling old source. They remain available in
candidate audit output but are not historical acceptance state.

Alias edges retain explicit build/product purpose. Checked and accepted sections
also bind the admitted execution profile and requested product target where
applicable. Matching source bytes permit acquisition reuse, not reuse of host
build results or policy as product evidence. Purpose-specific scheduling and
legacy migration follow [scoped execution](../build/scoped_execution.md#two-checked-contexts).
A legacy edge remains product-only; a missing build edge requires an explicit
declaration. A lock without purpose information cannot certify scoped execution,
and migration must recheck the edges without inferring permission from imports.

Use bounded deterministic diffable text with explicit outer and acceptance schema
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
nor implies an audit. Compare the fresh acceptance projection against retained
acceptance with exact complete-row equality. Changed risk meaning, source
replacements, and root-role changes use the ordinary decision workflow;
rebuildable API or representation changes alone do not require approval.
Source changes remain visible audit recommendations even with unchanged policy.
