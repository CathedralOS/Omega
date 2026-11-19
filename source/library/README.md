# Source libraries and their consumers

`source/library` holds reusable Omega packages. They have ordinary Build
 declarations and contracts, with the same authority, review and trust rules as
other dependencies. See [using source packages](../../wiki/language_guide/packages.md).
A folder or catalog label neither admits a package nor claims it works on every
target. The specification defines the required behavior; owning tasks record
unfinished implementation.

Find and run consumers with existing tooling:

```text
mbx xtask sample-gate --filter <consumer>
mbx xtask qualify --samples <consumer-path> --output <new-directory>
```

Consumers stay at their paths. `samples/cli/scientific/w100`, for example,
contains separate build roots and rejection controls; its directory is not one
program. [Sample discovery](../../samples/README.md#project-roles-and-discovery)
defines those roles. Focused programs read by Rust tests live in `tests/fixtures/`.

Move a reusable sample helper into a library when real use establishes its
contract. Move the implementation and migrate consumers together, preserving
inputs, expected results and rejection reasons. Do not leave two permanent
implementations or claim a consumer is validated without running its checks.

## Core integer and float APIs

Carrier queries live in `omega::language::core::integer`, and floating properties
and representations in `omega::language::core::float`:

```omega
integer::min<u32>()
integer::max<u32>()
float::min<f32>()
float::max<f32>()
float::epsilon<f32>()
float::min_positive_normal<f32>()
float::min_positive_subnormal<f32>()
```

[Numeric values](../../wiki/spec/language/numeric_values.md#core-integer-and-float-apis)
owns the query, classification and bit-preservation contracts. Float `min` and
`max` are finite endpoints, not smallest-positive values. These are ordinary
checked declarations over authoritative carrier metadata, not compiler-recognized
names, proof shortcuts or implicit provider searches.

[Width-indexed integers](../../wiki/spec/language/numeric_values.md#width-indexed-integers)
include positive static widths beyond machine registers: `u32` abbreviates
`u<32>`. That accepted surface is distinct from implementation coverage. It grants
neither general aggregate bit casts nor a foreign ABI for every width. NaN
constructors taking a payload remain library conveniences.

Numeric implementation and qualification stay with the corresponding tasks in
[TASKS.md](../../TASKS.md). An API declaration is not evidence that every operation
executes on each host.

## Allocation and growing collections

An allocator strategy manages actual backing; a selected allocation service
provides operations. Selecting a provider does not construct or capture a mutable
runtime allocator. Its state, backing, access and lifecycle must still be
established. Explicit arena/pool instances remain ordinary library choices.

A generic `Vec<T>` owns a live typed run and adjacent vacant spare backing.
Within-capacity operations need not acquire fresh backing or borrow an allocator
instance. Growth accounts for its selected service, failure outcome, live elements
and old/replacement backing. Automatic reclamation must satisfy the ordinary
infallible, terminating, nonblocking and nonsuspending cleanup contract;
otherwise release is explicit.

The concrete `alloc::vec` is a `u32` customer, not the completed generic container.
Reconcile the core placeholder and library implementation rather than creating
two meanings of `Vec`. `EXTENT-LIBRARY-TRANSFORMS` in [TASKS.md](../../TASKS.md)
owns this and the selected `Allocator` service; provisional `Arena` source is
not a second settled API.

[Allocation](../../wiki/spec/resources/allocation.md) requires linear elements,
zero-sized elements with cleanup, contained loans, failed growth and rejected
initialization. Complete borrowing, restoration and backing return as one
lifecycle. A scalar push demonstration is not generic acceptance.

## Core indexing contract migration

[Core indexing](../../wiki/spec/language/expressions.md#contract-gated-core-operations)
uses sealed primitives with ordinary invocation contracts. `CORE-INDEXING-CONTRACTS`
in [TASKS.md](../../TASKS.md) owns generic catalog admission, borrowed access,
mutable operations and ranges.

The checked `core/slice.omg` helpers use builtin brackets; they are not another
compiler indexing vocabulary. Importing a helper must not demand a provider
because an obsolete boundary declaration remains nearby. Preserve its backing
loan and consumer behavior while completing the migration.

## Core slice iteration

`use omega::language::core::iteration;` supplies an ordinary checked traversal:

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

`Command` is the consumer's record. `for_each<Element, Step>` visits elements
front to back and lends each exclusively to the statically selected callback.
An empty view calls nothing. `Element` need not be copyable. The callback must
satisfy the helper's termination, blocking, suspension and crash requirements.
The helper provides no captured context, early stop or runtime dispatch.

The callback's omitted reach is a
[deferred requirement row](../../wiki/spec/language/effects.md#deferred-requirement-reach),
instantiated from the supplied implementation. A service-free requirement writes
`reaches none`, not a bare `reaches` clause. Broader callback behavior follows
[published operational dependencies](../../wiki/spec/language/effects.md#static-callback-operational-dependencies).

Hollow's [input routing](../../samples/gui/dungeon_crawler/engine/control/routing.omg)
is a consumer, not proof that every callback combination runs. Existing callback
and iteration tasks own remaining coverage. Wrappers may use the
[forwarded callback rule](../../wiki/spec/language/generics.md#forwarded-callback-requirements)
rather than repeating the selected helper's complete contract.

## Hosted service reach

A GUI interface selects operations, not their effects. Window management,
presentation, dispatch, global input observation, clocks and execution delay
remain distinct identities. Building an image buffer does not itself present a
surface; polling physical keys can observe global input even after a focus check.

Opaque native bindings retain their admitted behavior. Narrow OS queries and
unrestricted native messages or callbacks need not have the same reach. A row
does not prove that opaque code obeys it or grant physical authority.

A checked body's omitted reach is inferred. A written row is a ceiling and
`reaches none` excludes services. Boundary requirements explicitly state their
row. `Build::audit_service<Trait>(builder)` requests review under the consumer's
own delegated authority; a dependency cannot set that consumer's acceptance.
[Effects](../../wiki/spec/language/effects.md) owns these rules.

## Static interrupt-table validation

The kernel supplies the validator run before table publication. Its
`crashes none` contract makes refusal a Boolean satisfying the selected policy.
A justified `false` prevents publication and returns control; the caller decides
whether refusal is fatal. `true` does not replace materialization or root
admission. The [validation contract](../../wiki/spec/build/hardware_materialization.md#interrupt-table-validation)
keeps the scope's separate crash routes and the validator's termination promise
distinct.

## C record storage

`CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>` is an ordinary transparent alias
in `omega::language::core::c_layout`. The policy computes offsets, alignment and
trailing padding. Geometry grants no storage, foreign, atomic or access authority.

The scoped consumers are the
[scalar record](../../samples/cli/facts/c_layout_scalar_record/README.md) and
[explicit nested representation](../../samples/cli/facts/c_layout_nested_representation/README.md)
examples. Use their actual execution evidence rather than treating the alias
as implementation support for every record or target. The full
[representation contract](../../wiki/spec/layouts/plans.md#representation-applications)
owns the accepted surface.
