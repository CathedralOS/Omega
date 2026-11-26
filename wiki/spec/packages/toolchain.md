# Core, ordinary libraries, and semantic bindings

The file-import root `{omega}/language/core/` is bundled as part of the language. The checker depends
on it, its version is the language version, and two core versions cannot coexist
in one graph. This is explicit semantic coupling, not an implicit package default.

System has no equivalent privilege. It is an ordinary fetchable convenience package
that may be replaced, split, omitted, or retired. Its name, repository, lineage,
path, filenames, and same-spelled declarations confer no compiler authority.
Freestanding programs need no system, and the compiler-owned Build protocol works
without it. Package-aware compilation admits only core's magic toolchain mount;
ordinary libraries use declared dependency edges and package-local imports.

Ownership comes from that resolved graph, not from executing a library's build
file. Repeated selection of the same system instance preserves one ordinary package
owner; core preserves its exact toolchain owner. Neither may collapse onto an
anonymous root's owner or acquire authority merely because a default build
supplied it. [Default builds](../build/declarations.md#default-builds-and-anonymous-owner-identity)
preserve supplied provenance and use content identity only when none exists.

Core is available in both checked build and product contexts. System use in build
code requires a build dependency; product use requires an ordinary dependency;
both uses require both edges. Neither edge grants ambient host services or makes
host and target type instances interchangeable. See
[dependency scopes](../build/scoped_execution.md#two-checked-contexts).

## Selecting a bundled package

The standard Omega distribution ships the core and Build libraries and the
ordinary source packages `system` and `allocation`. A project can build
offline against an ordinary package shipped with the selected toolchain.
Its Build selects the package by its declared name.

```omega
let system: DependencyDescription =
    |dependency|::from_source(Source::Bundled { name: "system" });
_ = |dependency|::add(build, system, @allowed[..]);
```

The Build driver looks up that exact name in the selected distribution's
verified library bundle. If the bundle does not supply the requested
package, report its name and fail the request. Lookup never fetches from
the network or searches the project's directories.

Registration gives the immediate product its ordinary package binding and
reach ceiling. The initial Build still starts with core and Build. A project
that later selects a pinned Git source can keep the same import alias and
ceiling. The new source goes through ordinary dependency review.

## Semantic bindings in a staged Build

Each compiler role takes its identity from the selection that owns it. The
target profile designates its entry contract. A selected provider plan fixes
the provider and the exact requirement it implements. An installed host
service identifies the declaration it backs. Native imports use the evaluated
requirement, target and locator from `Build::bind_native`.

Check the exact package instance, declaration and normalized schema in the
frozen occurrence, including the selected provider plan where the role requires
one. Derived identities travel with that occurrence. Reuse requires those
inputs to match. Reject missing, stale, ambiguous or mismatched selections;
a same-named declaration in another package cannot fill the role. The staged
workflow has no separately supplied binding table or per-project entry-contract
override. The [version 4 lock](locks.md) records the earlier acceptance route.

Recognizing a host declaration grants no access to a clock, filesystem or
other resource. Execution uses the provider and backing installed for that
invocation.

Package declaration visibility remains real inside core and ordinary libraries.
Source-facing float namespaces, formats, meanings, and operators are public
because their declarations say so. The two primitive-to-proof projections remain
private compiler-sealed operations; public float contracts may cite only those
exact checked toolchain declarations, not same-spelled authored replacements.

Primitive identities are compiler-owned; the toolchain designates exact core
owners of their canonical operator families. This permits ordinary checked
`machine +` bodies in authorized core code but does not make every core machine
a compiler primitive. Bodyless supply requires a separate exact closed-catalog
entry. Ordinary packages can own domain-specific arithmetic over primitive
carriers, not inject operations into the primitive's unqualified closed family.
See [operator supply and ownership](../language/expressions.md#executable-supply).

Package-aware compilation does not inherit standalone library provenance.
