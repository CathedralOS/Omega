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

## Hosted service reach

GUI provider interfaces select operations, not their service effects. Shared
core identities describe window management, presentation, event observation and
dispatch, global input observation, clock observation, and execution delay.
Core Graphics image-buffer construction does not by itself present a surface.
The macOS physical-key poll is global observation, even when a caller first
checks that its own window is focused.

Unrestricted Objective-C messages, autorelease finalizers, and native window
callbacks can execute native code. Their conservative published rows include
filesystem and process effects rather than concealing those behind framework
names. Narrow Win32 queries and image-buffer operations retain their distinct
contracts. External bindings still require their own trust review; a service
row neither proves opaque native behavior nor grants physical authority.

Under the 2026-10-22 `optional-reach-ceilings` decision, checked library bodies
may infer reach; an authored clause is a ceiling, and an empty clause permits
no services. Boundary requirements must explicitly declare their rows. The
existing mandatory/additive implementation and carrier-qualified helper
declarations remain migration work under `OPTIONAL-REACH-CEILINGS` and
`STATIC-MACHINE-MIGRATION` in [TASKS.md](../../TASKS.md); publishing this contract
does not claim those source migrations are complete.

The consuming root can make an exact boundary identity mandatory in review
with `Build::audit_service<Trait>(builder)`. Only executed registrations through
its original delegated Build count. Dependency registrations do not set the
consumer's policy, and changes to that policy change accepted review meaning.
