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
defines those harness roles. Small compiler behavior witnesses are programs
Rust tests read, kept in `tests/fixtures`, rather than another library or sample
hierarchy.

A kernel can start private in a sample and become a library when actual use
establishes a reusable contract. Stable contracts preserve observable behavior
and obligations while implementations evolve. One integration owner extracts
the kernel and migrates its consumers together in a bounded outcome, preserving
their authored inputs, frozen results, negative coverage and host qualifications.
Do not leave competing permanent implementations or call an unchanged consumer
validated against a new library without affected checks. A proposal to extract
a kernel is not authorization for a broad migration or new package acceptance.

## Core integer and float APIs

The ratified core surface groups integer carrier queries in
`omega::language::core::integer` and floating properties/representations in
`omega::language::core::float`:

```omega
integer::min<u32>()
integer::max<u32>()
float::min<f32>()
float::max<f32>()
float::epsilon<f32>()
float::min_positive_normal<f32>()
float::min_positive_subnormal<f32>()
```

These are accepted API contracts, not a claim that the modules are implemented.
[Numeric values](../../wiki/spec/language/numeric_values.md#core-integer-and-float-apis)
defines the complete query table, classification, exact bit wrappers, canonical
NaN values and explicit quieting/canonicalization. Float extrema are finite
endpoints; smallest-positive values have explicit names. Core implementation is
ordinary checked code over exact type refinement and semantic carrier metadata,
not a keyword, primitive-member namespace, implicit provider, or spelling-based
proof rule. General aggregate value bit casts and arbitrary-width integer
families are not introduced. Parameterized NaN constructors are convenience
code whose need may change with future unsigned narrow payload types.

`CORE-INTEGER-FLOAT-APIS` in [TASKS.md](../../TASKS.md) retains library/compiler
acceptance, nine-place facts and reject controls, including old-spelling
removal. Existing numeric examples using primitive-member limits are migration
inputs, not an implemented alternate API.

## Core indexing contract migration

The ratified [core indexing architecture](../../wiki/spec/language/expressions.md#contract-gated-core-operations)
selects token-bearing sealed primitives directly and proves their complete
contracts through ordinary call checking. Generic catalog admission, borrowed
access, mutable operations and ranges are tracked by **CORE-INDEXING-CONTRACTS**
in [TASKS.md](../../TASKS.md) and its [migration plan](../../wiki/drafts/designs/core_indexing_contracts.md).

The current `core/slice.omg` helpers `slice_index` and `slice_range` are used
named APIs whose checked bodies delegate through `slice_views` to builtin
brackets. They are not the compiler's authoritative indexing declarations.
The unused `SliceAccess` requirements are pending removal in this migration;
no provider must be selected merely to import the slice helpers. Named helper
compatibility and backing-loan preservation remain required during migration.

## Core slice iteration

`use omega::language::core::iteration;` exposes ordinary checked traversal:

```omega
machine clear_one(command: &mut Command)
satisfies ElementStep<Command>::apply {
	command.direction = -1;
	command.attack = false;
}

machine clear_commands(commands: &mut [Command]) terminates {
	for_each<Command, clear_one>(commands);
}
```

`for_each<Element, Step>` visits a mutable slice from front to back, calling
the exact statically selected machine once per element. Empty slices call
nothing. `Element` needs no `copy` property; the callback receives a temporary
exclusive element loan. The callback must explicitly satisfy
`ElementStep<Element>::apply`, terminate, and have no blocking, suspension or
crash behavior. Its omitted reach defers to the selected callback under
[deferred requirement reach](../../wiki/spec/language/effects.md#deferred-requirement-reach).
The traversal propagates that row, including application-defined services.
This initial API has no captured
context, result propagation, early stop or runtime dispatch. It is library
code, not loop syntax or a compiler intrinsic.

Hollow's [input routing](../../samples/gui/dungeon_crawler/engine/control/routing.omg)
uses the helper. The executable fixture `pass/generics/core_slice_for_each` of
the former corpus (`tests/README.md`) covered mutable records, non-copy elements, a forwarded view and empty input;
separate reject controls enforce callback and access obligations.

The effectful-callback behavior requires the
[callback reach migration](../../wiki/drafts/designs/deferred_callback_reach.md);
the existing fixture is not evidence that this extension is implemented.
`ElementStep::apply` keeps its omitted clause. Requirements elsewhere that rely
on service-free behavior need explicit bare `reaches` in that migration.

The language's [forwarded callback rule](../../wiki/spec/language/generics.md#forwarded-callback-requirements)
lets a wrapper with an explicit `machine Step` binder inherit this exact
requirement from `for_each<Element, Step>` instead of repeating its clause.
This does not widen the core helper's originating non-suspending, nonblocking
contract. Libraries that admit broader callbacks use
[operational dependency summaries](../../wiki/spec/language/effects.md#static-callback-operational-dependencies)
so a synchronous selection does not acquire suspension solely from that bound.

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
remaining reach and carrier-qualified helper migrations are tracked with
`STATIC-MACHINE-MIGRATION` in [TASKS.md](../../TASKS.md); publishing this contract
does not claim those source migrations are complete.

The consuming root can make an exact boundary identity mandatory in review
with `Build::audit_service<Trait>(builder)`. Only executed registrations through
its original delegated Build count. Dependency registrations do not set the
consumer's policy, and changes to that policy change accepted review meaning.

## C record storage

`use omega::language::core::c_layout;` exposes ordinary transparent
`CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>`. The policy computes aligned
member placement and trailing padding from the target-resolved schema.
Complete nonempty, unnumbered records compose scalar fields, scalar arrays and
explicitly selected child representations, including arrays of those children.
Bare nested aggregates, references, numbered fields, empty records and sums
reject instead of inheriting an unrelated native child layout. Geometry alone
grants no storage, foreign, atomic or access authority.

The [focused fact](../../samples/cli/facts/c_layout_scalar_record/README.md)
records scalar execution scope. [Explicit nested representation](../../samples/cli/facts/c_layout_nested_representation/README.md)
records composition and its remaining transport scope. The
[layout contract](../../wiki/spec/layouts/plans.md#representation-applications)
defines the broader C-layout family and explicit nested composition; the library
subset does not establish complete platform ABI migration.
