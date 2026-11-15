# Source libraries and their consumers

`source/library` holds reusable Omega packages. They have ordinary build
declarations and source contracts, and the same authority, review and trust
requirements as any source dependency. See [using source packages](../../wiki/language_guide/packages.md) for
the rules. A folder or catalog label neither admits a package nor claims it
works on every target.

Find or run the packages with repository tooling, not another list:

```text
mbx xtask sample-gate --filter <consumer>
mbx xtask qualify --samples <consumer-path> --output <new-directory>
```

Consumers stay at their paths. For example,
`samples/cli/scientific/w100` holds separate consumer build roots and separate
programs that must fail; the directory itself is not a program.
The [sample discovery contract](../../samples/README.md#project-roles-and-discovery)
defines those roles. Small programs that pin one compiler behavior are read by
Rust tests and live in `tests/fixtures`.

A sample's private helper can become a library once real use shows a reusable
contract; a stable contract keeps its behavior and obligations as the code
changes. One owner moves the code and migrates all its users together, keeping
their inputs, expected results, failing programs and host qualifications. Do
not leave two permanent implementations, or call an unchanged user validated
against a new library without its affected checks. A proposal to move code does
not authorize a broad migration or a new package acceptance.

## Core integer and float APIs

The ratified core API puts integer type queries in
`omega::language::core::integer` and float properties and representations in
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
defines the full query table, classification, exact bit wrappers, canonical
NaN values and explicit quieting and canonicalization. Float `min` and `max`
are finite; the smallest positive values have their own names. The
code is ordinary checked code over the exact type and its compiler metadata,
not a keyword, a primitive-member namespace, an implicit provider, or a proof
rule keyed on spelling. There are no general aggregate bit casts or
arbitrary-width integer types. NaN constructors taking a payload are a
convenience that may change with future narrow unsigned payload types.

`CORE-INTEGER-FLOAT-APIS` in [TASKS.md](../../TASKS.md) holds the library and
compiler acceptance, facts and failing programs, including old-spelling
removal. Numeric examples that read limits as primitive members are migration
inputs, not a second API.

## Core indexing contract migration

Under the ratified [core indexing architecture](../../wiki/spec/language/expressions.md#contract-gated-core-operations),
the compiler selects sealed primitives that carry a token, and ordinary call
checking proves their full contracts. Generic catalog admission, borrowed
access, mutable operations and ranges are tracked by **CORE-INDEXING-CONTRACTS**
in [TASKS.md](../../TASKS.md) and its [migration plan](../../wiki/drafts/designs/core_indexing_contracts.md).

The used `core/slice.omg` helpers `slice_index` and `slice_range` have checked
bodies that call `slice_views`, which uses the built-in brackets; they are not
the compiler's indexing declarations. This migration
removes the unused `SliceAccess` requirements; importing the slice helpers must
not require choosing a provider. Meanwhile the helpers must keep working and
keep their backing loan.

## Core slice iteration

`use omega::language::core::iteration;` provides an ordinary checked traversal:

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

`for_each<Element, Step>` walks a mutable slice front to back, calling the
statically chosen machine once per element. An empty slice calls nothing.
`Element` needs no `copy` property; the callback gets a temporary exclusive loan
of the element. The callback must state that it satisfies
`ElementStep<Element>::apply`, terminate, and never block, suspend or crash.
Its omitted reach (the services it may use) comes from the callback under
[deferred requirement reach](../../wiki/spec/language/effects.md#deferred-requirement-reach),
and the traversal passes it on, including application-defined services. It has
no captured context, result propagation, early stop or runtime dispatch, and is
library code, not loop syntax or a compiler intrinsic.

Hollow's [input routing](../../samples/gui/dungeon_crawler/engine/control/routing.omg)
uses the helper. The executable fixture `pass/generics/core_slice_for_each` of
the former corpus (`tests/README.md`) covered mutable records, non-copy elements, a forwarded view and empty input;
separate failing programs enforced the callback and access obligations.

Service-using callbacks need the
[callback reach migration](../../wiki/drafts/designs/deferred_callback_reach.md);
the fixture is no evidence that they work. `ElementStep::apply` keeps
its omitted clause. In that migration, other requirements that rely on a
service-free callback must write a bare `reaches`.

The language's [forwarded callback rule](../../wiki/spec/language/generics.md#forwarded-callback-requirements)
lets a wrapper with a `machine Step` parameter inherit this requirement from
`for_each<Element, Step>` instead of repeating it. The core helper itself still
refuses callbacks that suspend or block. Libraries that accept broader callbacks
use [operational dependency summaries](../../wiki/spec/language/effects.md#static-callback-operational-dependencies)
so a synchronous callback choice does not become suspending only through that
bound.

## Hosted service reach

A GUI provider interface chooses operations, not their service effects. Shared
core identities name window management, presentation, event observation and
dispatch, global input observation, clock observation, and execution delay.
Building a Core Graphics image buffer presents no surface by itself. The macOS
physical-key poll is global input observation even when the caller first checks
that its window has focus.

Unrestricted Objective-C messages, autorelease finalizers and native window
callbacks can run native code, so their published reach includes filesystem and
process effects. Narrow Win32
queries and image-buffer operations keep their own contracts. External bindings
still need trust review; a declared reach neither proves what opaque native code
does nor grants physical authority.

Under the 2026-10-22 `optional-reach-ceilings` decision, the compiler may infer
a checked library body's reach; a written clause is an upper limit, and an empty
clause allows no services. Boundary requirements must declare their reach. The
remaining reach and carrier-qualified helper migrations are tracked with
`STATIC-MACHINE-MIGRATION` in [TASKS.md](../../TASKS.md); this contract does not
claim those source migrations are done.

The consuming root can require review of one exact boundary identity with
`Build::audit_service<Trait>(builder)`; only registrations run through its own
delegated Build count. A dependency's registrations do not set
the consumer's policy, and changing that policy changes what an accepted review
means.

## Static interrupt-table validation

A kernel supplies the validator `X86StaticInterruptScope::run` calls before
publishing the table. The validator declares `crashes none` and returns a
Boolean following its table policy. A `false` the policy justifies stops
publication and returns to the caller, which decides whether that is fatal. The
[validation contract](../../wiki/spec/build/hardware_materialization.md#interrupt-table-validation)
leaves the scope's separate trap routes and the validator's termination contract
unchanged.

## C record storage

`use omega::language::core::c_layout;` provides the ordinary transparent alias
`CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>`. The policy computes aligned
member offsets and trailing padding for the target. Complete, nonempty records without numbered fields may hold scalars,
scalar arrays and children with an explicitly selected representation,
including arrays of them. Nested records without one, references, numbered
fields, empty records and sum types are refused rather than inheriting an
unrelated native child layout. Geometry grants no storage, foreign, atomic
or access authority.

The [focused fact](../../samples/cli/facts/c_layout_scalar_record/README.md)
records where scalar records have run. [Explicit nested representation](../../samples/cli/facts/c_layout_nested_representation/README.md)
records nested composition and what it covers so far. The
[layout contract](../../wiki/spec/layouts/plans.md#representation-applications)
defines the wider C-layout family and nested composition; this subset does not
complete migration to every platform ABI.
