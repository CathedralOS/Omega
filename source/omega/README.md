# Omega product compiler source

This package is the product root and Terminal-Psi consumer. The
sibling [`../psi/`](../psi/) package owns target-neutral source processing,
checking, and Terminal Psi; this package owns optimization, target realization,
artifact emission, and the product entrypoint.

The product compiler has two exact source implementations. `D` is written in
Delta; `C` is written in Omega using a deliberately conservative,
compositional subset of ordinary Omega.

```text
delta compiler + Delta source D → omega₀
omega₀ + Omega source C          → omega
```

`omega₀` may be conservatively generated and slow. It is already a full Omega
compiler because `D` implements the product language. The second build closes
the self-hosting edge and may improve the compiler executable; it does not add
language functionality.

Both compiler outputs are platform-independent Alpha tapes. Native target
realization belongs to this product phase only for user-program artifacts; it
does not turn any compiler rung into a native bootstrap artifact.

## Ownership

- [`../psi/`](../psi/) — target-neutral source, proof, and terminal semantics;
- this root — target realization, optimization, artifact emission, the
  Delta-written source closure `D`, and Omega-written source closure `C`;
- [`../omega-rust/`](../omega-rust/) — maintained Rust implementation and
  differential comparator, never bootstrap authority;
- [`../delta/`](../delta/) — final lower-rung compiler and direct first-build
  producer.

That source choice does not define a dialect or restrict programs the resulting
compiler accepts. Standalone viewers, interpreters, REPLs, and proof
explorers remain outside `C` unless the compiler executable imports them.

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md); bootstrap
closure is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `build.omg`, `main.omg` | Current roots of Omega-written compiler closure `C`; the closure is incomplete but is extended in place. | Delete or replace only when an exact package-root ruling changes `C`; do not preserve alternate hosted roots. |
| `omega_compiler.delta` | Incomplete Delta-written compiler closure `D`; currently owns strict source-view UTF-8 framing, the complete source-neutral lexical scanner, invocation-local source-shaped parser slices, the final exact Alpha tape encoder, bind-once label/fixup ownership, structural replay before sealing, and no invented application boundary. | Extend in place as `D`; replace a completed component only atomically with an equally complete final Delta implementation. |

The four empty target declarations in `build.omg` are temporary compatibility
scaffolding, not product architecture. Delete them as soon as immutable target
activation/reach closure lands, and normalize `windows_x64` to
`windows_x86_64` in that same migration.

`omega_compiler.delta` (`D`) now exists but is intentionally incomplete, and
both descriptive compiler tapes remain absent. The canonical sealed
package-closure request for the standalone Omega compiler is owner-blocked; no
raw-single-file stdin convention may stand in for it. Boundary-independent
final internals may be authored in `D` before that ruling, but no placeholder,
generated source closure, viewer, or standalone bootstrap owner is retained
while the artifacts are absent.

Delta cannot safely express a reusable validate-once source cursor: machines
and fields are public, while immutable views cannot be stored in data. `D`'s
parser therefore validates once and streams the same source through private
states of one canonical invocation. Its retained slices sequence empty,
trivia-only, ordinary `use path::member;`, basic `[pub] data`, and ordinary
empty-body machine roots.
One mixed root table preserves authored use/data/machine order. Optionally
public ordinary machines retain an arbitrary name-like path, optional
parentheses containing a comma-separated state-parameter list, and an
immediately empty body. An optional leading target selector is retained as an
exact span on the same machine row; selection and activation remain later
phases. The list retains canonical optional `const` and leading `mut`, consuming
or borrowed `self`, and shared/mutable/write-only binding-reference forms.
Non-receiver parameters retain `name: Type`. Non-receiver parameter types, data
fields, case-payload fields, and immediate machine return types share one engine
for bare named/Self, outer references, unqualified domains, inclusive
literal ranges, and nested fixed arrays and slices. References may retain an
exact explicit lifetime, and general `Self` uses the same `SelfType` base as
receivers. Every parameter row records its canonical const/mutable/self flags;
the implicit entry state owns the optional return node.

Each completed machine owns one implicit empty entry state and its contiguous
parameter span, matching the canonical parser: a free machine uses the
generated `entry` identity, while an attached machine names its entry with the
final authored declaration-path member. Machine and domain paths share the
general path-member arena, but a machine snapshots its path extent before
parameter types can append domain members. A trailing parameter comma rejects
as malformed. Constrained slice elements and the `Slice<T>` spelling, return
types placed after clauses, generics, clause-bearing headers, and
`boundary` forms; target declarations and public target-scoped combinations,
other public roots, bodyless declarations, and
nonempty bodies remain incomplete. The parser never skips a body as opaque
syntax. In the current 72-root `C` closure, all 112
parameter occurrences and all 72 complete parameter lists are representable,
and 53 headers reach body parsing. Every reached body is nonempty, so zero current
`C` roots complete.

Data syntax retains an optional `[copy]` property, bare named fields,
payload-free cases, contextual `case: Type` fields, structured case payloads
over the same bare named type leaf, one unqualified `Base in Domain`
constraint, one inclusive unsuffixed decimal-literal range
`Base [minimum..=maximum]`, recursively nested fixed arrays `[Type; length]`
over bare named leaves with the same unsuffixed decimal length spelling and an
optional outer domain, optional final member/case semicolons, mixed field/case
order, and relative spans in separate live-prefix tables. A case reaches its
contiguous payload-field span in a separate arena; direct and payload fields
share one binding control path. Type references are postorder tagged nodes: a
constrained root points to its base and to one source-shaped constraint, while
an outer Reference points backward to its complete referee tree and retains
shared/mutable/write-only access plus an optional exact lifetime span.
FixedArray and Slice nodes point backward to their element; FixedArray also
retains the exact length span. SelfType needs no payload.
Domain constraints point into the general path arena; literal ranges and array
lengths retain exact spans without interpreting their values. Bracket syntax
uses a bounded invocation-local frame stack and emits named, array, and slice
nodes in postorder, so every child index points backward.
Compact kind/index ledgers reach the use/data/machine rows and field/case child
spans instead of duplicating coordinates. Qualified, indexed, intersected,
combined, exclusive, expression-bound, or multiple constraints; constrained
slice elements, the `Slice<T>` spelling, generic types, rich array elements or
lengths; numbered identities;
field relevance; other public roots; and every other unimplemented valid form
stop as implementation-incomplete rather than becoming false Omega rejections.

The provisional backing tables hold 4,096 root/use/data/machine/implicit-state
rows and 16,384 path-member/data-member/direct-field/payload-field/case/
machine-parameter/type-node/constraint rows, plus 128 scratch array frames.
Only rows below their corresponding count may be inspected after `Complete`;
every other status may leave unowned partial prefixes and authorizes no
syntax-tree consumer. A repeated invocation invalidates old rows by resetting
every count. Root capacity dominates use/data/machine/implicit-state capacity,
while data-member capacity dominates direct-field/case capacity. Fields,
parameters, and machine returns share the type-node table, making `TypeNodes`
independently exhaustible. Its equal ceiling dominates payload-field,
machine-parameter, and constraint capacity in
the current slice because every retained row of those kinds owns at least one
type node.
Import and domain paths share the independently exhaustible path-member arena.
The meaningful resource distinctions are therefore `Roots`, `PathMembers`,
`DataMembers`, `TypeNodes`, and `TypeDepth`. These are private compiler budgets
to profile against the real compiler closure, not Omega source limits;
exhaustion is retained for the future outer `Incomplete` mapping.

No source identity, package alias, token ledger, decoded mirror, or transferable
preflight fact is retained. Q7 still owns binding each relative tree to a
package-owned source unit and fixing public diagnostic/outcome framing. A
public validate/advance split would be false authority, while revalidating the
whole view at every token would be quadratic; neither belongs in the compiler.
