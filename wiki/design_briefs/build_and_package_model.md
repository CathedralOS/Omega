# Design Brief: Build And Package Model

Current as of 2026-07-18. `build.omg` is ordinary Omega code interpreted in a
restricted build-time context. It produces inspectable build data and may stage
assets through explicitly supplied services; it is not a second configuration
language.

## Build entry

Each package may define:

```omega
machine build(
    b: &mut Build,
    fs: &mut Filesystem
)
    effects Filesystem + Console
{
    ...
}
```

The tool invokes the machine with a zero-initialized `Build` and scoped standard
providers. Filesystem access is rooted to the package/build directories in the
current implementation direction. Console logging is a declared service call,
not output silently intercepted by the interpreter.

The exact surface may omit unused providers through ordinary requirement/
provider machinery, but no service or authority is ambient.

## Code, not config grammar

`build.omg` uses normal data, calls, control flow, domains, and contracts. It
does not introduce `depends {}`, `target {}`, or another block dialect.

```omega
machine build(b: &mut Build, fs: &mut Filesystem) {
    b.depend("uefi", Source::Path("../../contracts/uefi"));
    b.subsystem = Subsystem::EfiApplication;
    b.freestanding = true;
    b.entry = Main::run;
    b.stack = 128 * KiB;
    fs.copy("assets/font.bin", b.output("font.bin"));
}
```

Library values such as `Source::Path`, `KiB`, and `Subsystem` carry the
vocabulary. Adding a target option normally extends `Build`/library data rather
than the parser.

## Current `Build` core

The first schema contains only pipeline-consumed facts:

```omega
data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}

data Build {
    subsystem: Subsystem;
    freestanding: bool;
    // dependency aliases, targets, entry, stack, and output data grow here
}
```

The zero case is the ordinary console/hosted default. `freestanding` remains an
orthogonal fact rather than being fused into the EFI subsystem name. In-source
`target ... {}` blocks are transitional syntax to remove; image facts belong in
`Build`.

The platform's launch calling plan is checked against the exported entry
machine for each built target. `build.omg` selects the entry; it does not repeat
the target's register/stack arrival contract.

## Provider selection

Selecting a target package also selects that package's ordinary default
provider types. A build, test harness, or component manager that owns a service
slot may override that slot with another admitted provider type. This is scoped
dependency injection, not row construction: conformances and `via` bindings
declare the provider, the toolchain derives its normalized `ProviderPlan`, and
configuration selects the already-declared candidate.

The exact `Build` library method names remain ordinary API design. Conceptually
the operations are target-profile selection plus type-per-slot override; users
do not repeat every default and cannot append or mutate derived plan rows.

## Build-time authority and execution split

`build.omg` may perform authorized package-local staging itself. It does not
directly fetch arbitrary network dependencies, invoke an unchecked compiler, or
link the image unless those services are explicitly admitted in a future build
contract.

```text
tool-in-hand
  -> interprets build.omg with pinned Filesystem/Console slots
  -> receives augmented Build data and staged package-local assets
  -> resolves/fetches pinned dependencies
  -> compiles, links, emits, and records artifacts
```

Build-time admissibility uses the complete normalized machine contract, not
only an empty effect row. Provider trust, authority roots, resource bounds,
failure, and termination must fit the build evaluator's contract floor.

## Dependencies and the lock artifact

Code imports package-local aliases. `build.omg` binds each alias to a source:
content hash, local path, or exact repository revision. Fully qualified paths do
not bypass the declared alias/reach set.

The unified lock artifact records the resolved closure:

- content/package identities;
- toolchain identity;
- mutable-reference resolutions, if permitted;
- boundary/provider trust receipts;
- accepted proof/grant identities; and
- component/build contract identities needed for reproducibility.

Exact pins in source reduce resolution, but do not eliminate the value of this
machine-produced audit artifact. The lockfile is generated/checked state, not a
second hand-authored dependency language.

## Package reach boundary

The package is the dependency-reach boundary:

- `pub` says what the package offers;
- `build.omg` says what packages/services it may reach;
- undeclared aliases are not nameable; and
- a subsystem requiring a meaningfully different reach set is a separate
  package rather than a hidden nested manifest.

Packages normally compose statically and may optimize across package edges.
They are not ABI or replacement boundaries merely because they are packages.
A build may select a provider realization for independent deployment; the
component is that realization plus its compiler-validated owned closure.

The first implementation may accept only closures coinciding with one package.
That is an implementation restriction, not the semantic definition of
component. A concrete-machine call crossing a selected replaceable closure
rejects; a replaceable crossing names an ordinary requirement. The same
requirement may be statically selected and inlined in another build. No
hot-swap call syntax or `slot` keyword is implied.

## Authority evidence and admission

Runtime authority uses ordinary data layout plus domain evidence. The artifact
records each owner-authorized boundary establishment, checked resource
transformation, provider/backing requirement, admitted claim, and reachable
authority. Public
data shape and domain trust policy enter contract/component compatibility
identity; private implementation bodies and proof evidence affect content
identity while remaining outside public contract identity.

Package policy admits the transitive reachable-authority set of the final
resolved artifact. It does not approve dependencies one edge at a time. A new
root-memory, DMA/IOMMU, executable-installation, interrupt-publication, or
equivalent reach blocks unless deployment policy explicitly grants it,
regardless of which transitive package introduced the change.

The complete manifest remains machine-readable. Human diffs are
severity-ranked: checked local tokens collapse to a short summary, while new
admitted providers, boundary-evidence permissions, provider-owned backing,
generation/revocation machinery, or system authority are elevated with their
dependency path. Package policy decides who may enter with power; checked
contracts still constrain behavior after admission.

## Workspace composition

A workspace build composes member `Build` values with ordinary Omega code.
Shared pins and ceilings may be passed into members and members may only narrow
them. Source code never searches parent directories for ambient imports; only
the build tool discovers the nearest enclosing workspace/build entry.

## Current engineering delta

The interpreter already has real/virtual filesystem modes and a scoped
filesystem-backed build evaluator. Remaining work:

- adopt the `build(b, fs)` entry and standard provider injection;
- replace the retired empty-effect gate with decision-22 normalized ceiling
  checks;
- converge Console/platform entries onto boundary traits;
- finish the `Build` dependency/target/entry schema;
- expose target-profile defaults and type-per-slot provider overrides through
  ordinary `Build` library machines;
- make name resolution consult the declared dependency aliases;
- generate/check the unified lock and trust artifact; and
- remove target-block and hand-written BuildLog compatibility paths.

## Still open

- final `Build` schema and ergonomic library calls;
- mutable dependency references and update policy;
- workspace inheritance/ceiling details;
- which additional services a build may request beyond Filesystem/Console; and
- exact build-entry discovery and default-entry behavior.
