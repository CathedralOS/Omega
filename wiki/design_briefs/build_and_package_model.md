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

## Provider profiles and overrides

Provider policies are ordinary package declarations; `build.omg` owns the
static root's service slots and selects admitted realizations. Most applications
choose one platform profile rather than naming every standard service:

```omega
machine build(b: &mut Build) {
    b.platform = LinuxHosted;
}
```

The platform profile contributes a package-authored default provider bundle.
Exceptional slots may be overridden explicitly:

```omega
b.override_provider(Clock, DeterministicClock);
b.override_provider(Writable, SandboxedFilesystem);
```

Exact library spelling remains provisional. Resolution always prefers an
explicit override, then one applicable profile default, and otherwise fails;
two applicable defaults are ambiguous. The normalized `Build` artifact expands
the profile into complete provider-plan identities, admission receipts, and
slot assignments. This preserves zero-ceremony hosted defaults without making
target selection hidden semantic magic.

Static build selection is one instance of slot-owner authority. Component
managers and test harnesses may own narrower dynamic slots through the same
admission/selection model. See
[`provider_plans.md`](provider_plans.md).

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

Machines remain the behavioral/hot-swap units inside that package. Package and
component identity are related but not conflated.

## Workspace composition

A workspace build composes member `Build` values with ordinary Omega code.
Shared pins and ceilings may be passed into members and members may only narrow
them. Source code never searches parent directories for ambient imports; only
the build tool discovers the nearest enclosing workspace/build entry.

## Current engineering delta

The interpreter already has real/virtual filesystem modes and a scoped
filesystem-backed build evaluator. Remaining work:

- adopt the `build(b, fs)` entry and standard provider injection;
- add platform-profile default bundles, explicit scoped overrides, and
  normalized provider-slot reporting;
- replace the retired empty-effect gate with decision-22 normalized ceiling
  checks;
- converge Console/platform entries onto boundary traits;
- finish the `Build` dependency/target/entry schema;
- make name resolution consult the declared dependency aliases;
- generate/check the unified lock and trust artifact; and
- remove target-block, `provides`, and hand-written BuildLog compatibility
  paths after provider-plan migration.

## Still open

- final `Build` schema and ergonomic library calls;
- mutable dependency references and update policy;
- workspace inheritance/ceiling details;
- which additional services a build may request beyond Filesystem/Console; and
- exact build-entry discovery and default-entry behavior; and
- final profile/override library spelling and dynamic slot-capability scoping.
