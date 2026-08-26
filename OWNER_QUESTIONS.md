# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-25.

## Q1 — Physical ABI for opaque by-value boundary data

### Context

`InterruptAcknowledgement` and `InterruptMaskGuard` are now public opaque
linear boundary data. Their provider-owned settlement fields are correctly no
longer source-visible. Both can nevertheless cross a boundary by value; for
example, `InterruptEntry::enter` receives an `InterruptAcknowledgement` and is
governed by a source-authored `Calling<C>` policy.

### Problem statement

Calling-policy evaluation needs a target-specific byte size and alignment
before it can validate a by-value placement. Opaque boundary data deliberately
has no ordinary Omega layout, and package review currently records its ABI and
mechanism as `Unbound`. The compiler therefore rejects the interrupt entry as
zero-sized. Restoring public structural fields, treating the value as a ZST, or
hardcoding its former five-`u64` shape would each contradict the opacity and
representation-TCB decisions.

### Proposed direction

Keep the source type opaque, but require the selected provider/installation to
supply a compiler-validated, target-specific representation descriptor before
evaluating any `Calling<C>` policy that passes the value by value. The policy
may inspect only the closed shape descriptor, never provider fields. Review and
eventual admission should replace `Unbound` with the exact ABI and mechanism
commitments and reject when no unique descriptor is selected.

### Alternates

- Acceptable if it matches the intended machine contract: make opaque
  obligations cross this boundary through an explicit reference/handle shape,
  so no by-value representation is promised.
- Tempting but wrong: restore public identity fields merely to recover layout.
- Tempting but wrong: assign a compiler-global magic size or accept zero-sized
  placement without selected representation evidence.

## Q2 — Package selector for a multi-package source

### Context

A fetched Git repository may have a package at its root or a workspace root
whose member paths lead to several packages. Member paths are deliberately not
stable package names. The selected member's own `builder.package("name")`
declaration remains authoritative identity evidence.

### Problem statement

`Source::Git` currently carries only repository and revision. That is
unambiguous for a repository-root package but cannot select one package from a
workspace. The lock cannot be the only selector because a fresh lockless
resolution must be reproducible, and an import alias cannot select because
aliases are local and may be explicitly renamed.

### Proposed direction

Make the expected canonical package name explicit in the authored Git source
request. Treat it only as selection intent: after authenticating the repository
root, project its workspace members, project each member's own declaration, and
require exactly one declaration to match. The package-authored declaration—not
the request string—continues to establish the name joined into `PackageKey`.
A repository-root package follows the same match rule, avoiding two resolution
models.

### Alternates

- Acceptable: a concise ordinary Omega wrapper operation may carry the expected
  name separately from `Source::Git`, provided it is mandatory, survives
  dependency projection, and has the same exact-match semantics.
- Tempting but wrong: select by member directory path; repository relocation
  would become package replacement and callers would duplicate workspace
  layout.
- Tempting but wrong: infer selection from the default alias or defer it to
  `omega.lock`; explicit aliases and first resolution make both ambiguous.

## Q3 — Application identity in the package graph

### Context

Applications now declare `builder.application("name")`, may own dependencies,
and form the root of a reconciled package closure. Compiler package handoff
currently identifies graph roots through `PackageKeyIdentity`.

### Problem statement

Giving applications no source-qualified graph identity requires a second root
identity system and weakens provenance across application updates. Treating an
application as an ordinary dependency, however, would erase the role
distinction and permit consumers to import an artifact root as a library.

### Proposed direction

Give an application the same name-plus-source-lineage `PackageKey` used for a
stable reach-unit identity, while retaining `Application` as its role. It may
own dependencies and produce artifacts but cannot satisfy another project's
package dependency. Exact source and artifact evidence remain instance facts.

### Alternates

- Acceptable if a concrete compiler constraint requires it: define a distinct
  source-qualified application-root key with the same lineage and instance
  commitments, then prove the graph handoff cannot confuse it with packages.
- Tempting but wrong: key an application by its authored name alone.
- Tempting but wrong: make applications importable packages merely to reuse
  existing graph code.
