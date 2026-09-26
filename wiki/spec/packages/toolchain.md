# Core, ordinary libraries, and semantic bindings

`omega::language::core` is bundled as part of the language. The checker depends
on it, its version is the language version, and two core versions cannot coexist
in one graph. This is explicit semantic coupling, not an implicit package default.

Std has no equivalent privilege. It is an ordinary fetchable convenience package
that may be replaced, split, omitted, or retired. Its name, repository, lineage,
path, filenames, and same-spelled declarations confer no compiler authority.
Freestanding programs need no std, and the compiler-owned Build protocol works
without it. Package-aware compilation admits only core's magic toolchain mount;
ordinary libraries use declared dependency edges and package-local imports.

Core is available in both checked build and product contexts. Std use in build
code requires a build dependency; product use requires an ordinary dependency;
both uses require both edges. Neither edge grants ambient host services or makes
host and target type instances interchangeable. See
[dependency scopes](../build/scoped_execution.md#two-checked-contexts).

## Consumer-supplied semantic bindings

When target entry/profile integration or risk classification needs to recognize
an ordinary package declaration, bind its exact nominal identity and normalized
schema in the accepted resolved closure. A candidate designation only guides
confined review; accepted binding comes from consumer policy. It grants no
provider or capability and cannot be inferred from an alias or source location.

Rejoin package, exact declaration, complete schema, and any role-required selected
plan. Every supplied row settles exactly once; foreign, stale, ambiguous, or
unmatched rows reject. Downstream consumers use the resolved symbol, not another
name search. Requirement-only roles cannot synthesize a provider. An interpreter
routing token identifies the exact declaration in one checked program; it grants
no filesystem access or policy acceptance.

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

Current narrow roles and standalone compatibility limits live
[beside package compilation](../../../omega-rust/omega/build/package-compilation/semantic_bindings.md).
Removing broad standalone library provenance is tracked by
`OPTIONAL-STDLIB-SEMANTIC-BINDINGS`; package-aware compilation must not inherit
that temporary compatibility behavior.
