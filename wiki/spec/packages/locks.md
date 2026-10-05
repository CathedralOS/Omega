# Project locks

The lock is retained project intent, not a certificate of package safety.
[Acceptance](acceptance.md) owns review and decision authority;
[source selection](sources.md) owns resolution and immutable graph construction.
The lock is the one acceptance record; a root grant in `build.omg` accepts the
root's own claims, and there is no second admissions file.

## Retained state

A lock contains source-qualified package keys, explicit root role, exact immutable
commit/tree/content and member selections, source requests, requester-local alias
edges, compact normalized acceptance policy, exact accepted compilation inputs,
and explicit project decisions.
Each package retains exact canonical rows for admission-claim and external-
realization callables, external executable supplies, dangerous capabilities,
supplied terminal permissions, and accepted restricted build requests, scoped
to their reviewed purpose, execution profile, and product target where applicable.
Every retained row preserves its complete readable meaning, not only a digest.

Each accepted package occurrence also retains a compiler input identity and its
exact semantic bindings: role, package owner, declaration path, normalized schema
digest, optional selected-provider-plan digest, and explicit terminal permissions.
These are inputs to compilation, not a serialized full review or a grant derived
from a package name. Purpose, checked target, and build execution profile remain
exact occurrence coordinates; root, dependency, Product, and Build acceptance
are not interchangeable.

[Assumed binding relationships](../build/assumed_binding_contracts.md) are
risk-bearing assumption rows: retain the required/supplied contract identities,
positional mapping, admitted semantic difference, runtime/root scope, and exact
selected implementation and semantic-dependency commitment. These are not the
ordinary selected-provider tables excluded below. A changed relationship or
implementation invalidates its acceptance; retaining a friendly provider name
or unchanged permission list is insufficient. The lock never authorizes use of
these assumptions during build execution.

Restricted-build rows bind the exact package and permission meaning, operation,
logical resource scope, bounds, and applicable context. They record the consuming
project's explicit acceptance, not authority supplied by a dependency's own lock.
Fresh checked contracts reconstruct the direct/transitive requests at each use;
the existing comparison rules decide whether retained acceptance still applies.
Resolution-only updates cannot add or widen approval. Unchanged accepted requests
need no recurring approval, while source changes remain visible in the audit.

Host resources and credentials are supplied separately at invocation. The lock
contains no secrets, machine-specific absolute grant paths, or temporary host
capabilities. Logical resource names are reviewable request data, not authority
to resolve an arbitrary host path. See
[restricted-build acceptance](acceptance.md#restricted-build-acceptance).

The lock does not retain the complete public API, ordinary checked callables,
selected-provider tables, representation snapshots, source-semantic dependency
graphs, or symbolic-demand snapshots. Explicit review, acceptance, and update
workflows derive those complete findings from the candidate, not by compiling
old source. Ordinary compilation uses the accepted binding inputs after identity
matching and does not reconstruct a full package review.

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
The outer format is version 4 and requires compilation inputs for every
occurrence. Older versions, including version 3, reject rather than guessing
compiler identity or bindings; the compact acceptance child uses policy row
schema version 3.
Unknown formats or unrecoverable baseline meaning reject with recovery guidance;
they never become empty acceptance or an inferred schema migration. Cache paths,
proof certificates, native artifacts, replay transcripts, audit receipts, and
compiler-private handles are not this payload. The
[lock codec](../../../omega-rust/omega/packages/package-manager/src/lock/README.md) owns
the exact framing and resource limits.

Projects normally commit the lock and trust whoever lands it. Recovery checks
format, graph consistency, and acquired content against pins, not a certificate
that its author reviewed carefully. Explicit review compares fresh findings
against retained policy. Ordinary compilation trusts current accepted inputs;
regenerating resolutions cannot invent approval, and native compilation needs
no second acceptance channel.

## Source and control state

Root `omega.lock` in a mutable local package is project
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
[configuration](../build/configuration.md#multi-target-compilation).

Locked compilation never refreshes a mutable selector. It may acquire missing
content at the recorded pin when allowed; offline absence leaves the graph
unchanged. Select a recorded target before acquisition: missing target policy
fails without network access.

Per the owner ruling of 2026-10-03, ordinary compilation compares each accepted
occurrence's source identity and compiler input identity with the lock. This
includes the local root: source edits are not an exception. A match consumes
the occurrence's accepted bindings without package-review discovery/bound
passes. A mismatch rejects, names the package, and directs the user to
`omega update`; compilation does not audit inline or silently renew acceptance.
Dependency product checking follows the modules named by the consumer's `use`
items and their transitive imports, not each dependency's package root.

An absent lock permits development compilation through the same checked
package route, including native production and `omega run`. It supplies no
accepted semantic bindings, restricted-build grants or receiver permissions,
and does not initiate review or create a lock. Dependencies that need those
inputs still require explicit acceptance. An unreadable lock, a missing target
section or stale accepted inputs never falls back to this unlocked behavior.

The compiler input identity binds the compiler's embedded build-input digest and
the exact selected toolchain library content. It is an invalidation key, not
evidence of a trusted compiler, host, or audit. Changing it requires explicit
review even if source pins are unchanged.

During that review, compare complete risk-row meanings rather than only hashes.
Changed risk meaning, source replacements, and root-role changes follow the
ordinary decision workflow. Rebuildable API or representation changes alone
need no new approval decision, but their source changes still require review
before renewing the accepted source identity. Source changes remain visible
audit recommendations even when permission meaning is unchanged.
