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

Keep the existing `Source::Git` case for an unambiguous repository-root package;
the resolver reads that package's own declaration, so the caller does not
repeat its name. Add ordinary `Source::GitPackage { repository, revision,
package }` data—not grammar—for selecting a package from a workspace Git
source. Treat `package` only as selection intent:
after authenticating the repository root, project its declared member paths and
require exactly one member's own package declaration to match. That fetched
declaration—not the request string—establishes the name joined into
`PackageKey`. Using root-package `Source::Git` on a workspace rejects as
ambiguous.

### Alternates

- Acceptable: add an optional selector to the existing Git source data if Omega
  construction remains concise and omission is permitted only for a
  repository-root package. A separate source case is clearer with the current
  explicit data model.
- Tempting but wrong: require a package-name field for every Git source. A
  repository-root package is already unambiguous, so this makes every ordinary
  package declare the same name twice.
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

## Q4 — Scoped build machines as project manifests

### Context

Package identity and dependency projection recognize one canonical free
`machine build(builder: &mut Build)` in `build.omg`. That entry declares the
project role and owns the authoritative dependency projection. Standalone
compiler loading still recognizes both free `build` and scoped
`Owner::build` machines in `build.omg` as privileged build roots. Two positive
provider canaries and three deliberately failing build-authority canaries use
the scoped form.

### Problem statement

One `build.omg` currently has two incompatible meanings. Package-aware readers
reject scoped build machines because they cannot establish the single canonical
project role/dependency root, while standalone compilation executes them with
build authority. Enforcing roles globally would either reject an intended
composition surface or preserve a second project-manifest model. It would also
mask the authority diagnostics pinned by the malformed scoped canaries unless
their intended status is decided first.

### Proposed direction

Retire scoped machines as project build roots. Require exactly one free build
entry to declare the application, package, or workspace role and own dependency
projection. Component-specific provider configuration remains ordinary Omega
composition selected or called from that root rather than acquiring a second
manifest identity. Migrate the positive scoped canaries to the free entry and
recast the failing canaries so they continue testing their authority violation
under the canonical root.

### Alternates

- Acceptable if scoped ownership is semantically important: formally admit
  exactly one scoped root and specify how it declares project role, owns
  dependencies, receives the `Build` activation, and excludes any competing
  free root. Both compiler and package readers must then share that rule.
- Tempting but wrong: keep standalone acceptance and package-reader rejection;
  the same file would continue to mean different things by caller.
- Tempting but wrong: infer project role from the scoped owner name.
- Tempting but wrong: add a no-op free manifest beside the privileged scoped
  build; that restores duplicate build roots rather than one authoritative
  entry.

## Q5 — Fixed-array element cleanup order

### Context

Literal-length fixed arrays expose one canonical ownership path per element.
Moving one literal-indexed element leaves every unselected sibling obligation
live, and the cleanup plan must later dispose each remaining cleanup-bearing
element exactly once. Records already clean structural fields in recursive
reverse declaration order, but array elements are not declarations and the
language guide assigns them no cleanup order.

### Problem statement

General fixed-array cleanup, including partial arrays with more than one live
element, needs one deterministic semantic order before checked cleanup plans,
fuel, proof traces, and native artifacts can agree. Choosing increasing or
decreasing index order in the compiler would silently add language semantics.
The bounded two-element slice with exactly one moved element and one residual
does not expose this choice, but wider arrays and multiple residuals remain
blocked on it.

### Proposed direction

Define literal array construction in increasing index order and structural
cleanup in the reverse order of the live constructed elements: decreasing
index, skipping moved elements. This matches record cleanup's reverse-source
principle, makes partial cleanup a filtered suffix/order rather than a new
schedule, and gives interpretation, fuel, and artifact replay one canonical
sequence.

### Alternates

- Acceptable if iteration semantics should dominate: clean in increasing index
  order, but state why arrays intentionally differ from reverse record-field
  cleanup and pin construction-failure behavior to the same choice.
- Acceptable if element order must be type-directed: require an explicit
  collection-owned cleanup policy, but ordinary fixed arrays then need a
  canonical default before they can contain cleanup-bearing elements.
- Tempting but wrong: let each backend choose an order or treat order as
  unobservable. Cleanup calls can carry effects, requirements, guarantees,
  fuel, and diagnostics, so their sequence is semantic.
