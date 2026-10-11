# Source libraries and their consumers

`source/library` is the existing home of reusable Omega package implementations.
Its packages carry ordinary build declarations, source-level contracts and the
same authority, review and trust requirements as other source dependencies.
See [using source packages](../../wiki/language_guide/packages.md) for declaration,
dependency and identity rules. A folder or catalog label does not grant admission
or claim that a package is complete on every target.

Discover the current packages directly rather than maintaining another list:

```text
python3 tools/sample_projects.py --root source/library
python3 tools/sample_projects.py --role library-consumer
```

Consumers remain ordinary programs at their current paths. For example,
`samples/cli/scientific/w100` has independent consumer build roots and separate
source-invalid controls; the outer grouping directory is not an executable.
The [sample discovery contract](../../samples/README.md#project-roles-and-discovery)
defines those harness roles. Small compiler behavior witnesses live in
`tests/omega`, rather than another library or sample hierarchy.

A kernel can start private in a sample and become a library when actual use
establishes a reusable contract. Stable contracts preserve observable behavior
and obligations while implementations evolve. One integration owner extracts
the kernel and migrates its consumers together in a bounded outcome, preserving
their authored inputs, frozen results, negative coverage and host qualifications.
Do not leave competing permanent implementations or call an unchanged consumer
validated against a new library without affected checks. A proposal to extract
a kernel is not authorization for a broad migration or new package acceptance.
