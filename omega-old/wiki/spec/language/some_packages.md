# Per-value type packages with some

`some S` in a data declaration lets each value choose a concrete type while
preserving the relationships between fields that use it. Unlike an ordinary
generic argument, that choice is not part of the external data type. Different
choices can therefore inhabit one array or growable container.

Ratified 2026-11-06 with [runtime machine values](machine_values.md). This replaces
captured `dyn` and its compiler-derived global roster. The spelling `some` is a
data binder, not visibility, an inference hole, a universal type, or a runtime
type object.

## Declaration and construction

```omega
data Reader<'a, some S> {
    state: &'a mut S;
    read: machine(state: &mut S, output: &mut [u8]) -> u64
        ensures result <= output.len
        reaches none
        crashes none;
}
```

`Reader<'a>` is the external type; callers supply ordinary type, lifetime and
const arguments, not the `some` arguments. `Reader<File>` is not the spelling
for choosing this declaration's packaged `S`. An ordinary `Reader<S>` declaration
without `some` still produces different types for different `S` arguments.
Aliases preserve this distinction rather than introducing another binder.
Ordinary lifetime elision remains available where already permitted; packaging
does not invent a lifetime bound. Multiple `some` binders choose separate
witnesses, with any relationships stated and checked in the declaration.
A [nested data declaration](data_and_literals.md#records-sums-and-nested-declarations)
does not inherit a `some` binder; a field passes the witness to a parameter the
nested type declares.

An ordinary record literal constructs the package:

```omega
let reader: Reader = Reader { state: &mut file, read: read_file };
```

Construction infers one concrete witness for each `some` binder from the field
initializers and checks all related fields against that same choice, including
callable contracts and the data invariant. Unresolved or inconsistent witnesses
reject. A machine accepting `Socket` cannot accompany `&mut File`. Witnesses
remain checking evidence, not runtime source-level type values. A generic helper
may construct a package using its ordinary static type argument as the witness,
and an [opened abstract witness](#owned-state-through-uniform-carriers) may do
so within its scope.

V1 permits sized concrete witnesses. A field depending on `S` must have a
uniform representation independent of the witness: references to sized
referents, callable descriptors over those references, and
[uniform carriers](#owned-state-through-uniform-carriers) whose custody index
names the witness are the basic forms; ordinary compositions of admitted
uniform fields are allowed. Inline owned `S`, witness-sized arrays,
witness-dependent field offsets and unsized referents are not admitted by this
rule. A callable cannot require moving an unknown-layout `S` by value.
Unrelated concrete owned fields remain ordinary data with their own cleanup.

## Opening by stable subject

Projecting or decomposing a package opens its witness to a fresh abstract type
tied to that exact subject and storage version. All dependent projections in
the same stable opening share that type. The rule is independent of expression
spelling; it does not recognize a special `reader.read(reader.state)` idiom.

```omega
machine read_one(reader: &mut Reader, output: &mut [u8]) -> u64
ensures result <= output.len
{
    (reader.read)(reader.state, output)
}
```

Here the callable and state have matching abstract types under the same opening.
Invocation obtains an ordinary fresh reborrow of the stored mutable reference;
it does not duplicate or consume that reference just to make a reusable call.
The callable's complete contract governs preconditions, results, effects,
suspension and custody. Opening alone supplies neither those premises nor
private facts about the concrete witness.

Use canonical places, storage versions and captured index values to identify
openings. Repeated indexing shares an opening only when the checker establishes
the same stable element and relevant index value. It must not equate two textual
`items[i]` occurrences across a change to `i`, a collection relocation, or an
overlapping write. Borrowing one element into a local gives an ordinary way to
make the stable subject explicit.

Reassignment of the whole package establishes a new witness association. Live
dependent projections prevent invalid replacement under the ordinary loan rules;
otherwise the old opening and its facts expire. Updating a field in an open
package must preserve the existing witness and invariant. Changing the witness
requires constructing and replacing the complete package coherently, not mixing
fields from old and new packages inside an invariant window.

Moves transfer the package's witness association and custody. Permitted copies
retain evidence of their shared witness origin; unrelated packages do not gain
type equality because they happen to contain the same concrete type. Joins keep
correlated fields together while forgetting unproved concrete choices. An
explicit checked type equality may justify transport; equal sizes, erased
addresses or equal runtime tags cannot manufacture that evidence.
No executable cross-package type comparison or synthesized structural comparison
is introduced merely by having a `some` binder.

## Scope, escape and borrowing

An abstract witness is usable within its stable opening, including by local
bindings, checks and calls whose published operations support it. It supplies no
concrete field namespace, size-based allocation or arbitrary executable generic
specialization. Generic code needs its ordinary operations/evidence; erasure does
not synthesize missing implementations or runtime dictionaries.

A result or retained field whose type contains the opened witness cannot escape
its scope. A helper may return a complete `Reader` package, or another properly
constructed package, but cannot expose `reader.state` as an unbound `&S`. A
concrete result such as `&[u8]` may escape under its checked input-relative lifetime
and custody contract. Borrowing, packing and opening never extend that lifetime.
Existing access policies and reflection authority still apply; `some` adds no
private fields, privileged consumer or dynamic downcast route.

Shared/exclusive access, suspension, movement, linear obligations and cleanup
follow ordinary data rules. Dropping a borrowed package releases its loans when
permitted; it does not destroy the borrowed state. Moving it into a container
retains those loans. Removing it only transfers them to the removed value;
consumption releases access only when no dependent copy or result loan remains.

Runtime branches and generic helpers may append packages with different witness
types to the same `Vec<Reader>`. Neither the number, order nor type choices of
runtime entries require compile-time prediction. Each construction and every
use must be well-typed and obey the container's capacity/allocation contract.
A reference to a helper's local cannot survive that local through packaging.

## Owned state through uniform carriers

Ratified 2026-11-08; [owning some packages](../../drafts/designs/owning_some_packages.md)
records the migration. A field depending on a `some` witness may own that state
when its type is uniform: the witness occurs only in custody or evidence
indices, such as `Extent in Granted & Resident<P, S>`, or behind admitted
references and callable descriptors, never in a position that determines
layout. The judgment is structural over the field's type and over every
parameter and result of every stored callable. A parameter or result of type
`Owner<S>` is admitted where `Owner<S>` is uniform; bare `S` by value stays
rejected. Equal sizes and authored assertions of uniformity establish nothing.

The carrier is ordinary library data, not a privileged name. The qualified
extent already owns one complete live `S` under
[resident custody](../resources/access.md#establishment-and-retirement) and
returns to its provider; an allocator family wraps it with its return handle
where one is needed, and two owners of the same payload from different providers
keep distinct return paths. Packing allocates nothing: it moves an established
owner into the record. Stored operations take the owner, by reference or by
value, rather than `&mut S`, so the consumer never specializes over the hidden
type. Opening pairs `state` with its operations as above, while
`&mut package.state` remains an ordinary field borrow.

Each stored operation keeps its own
[callable contract](machine_values.md#callable-types-and-construction), and
construction checks the selected machine against it; one operation's reach or
suspension does not attach to another because they share a record. Consumption
is any stored operation taking the owner by value. There is no terminal-consumer
spelling, and a package may offer several, such as `run` and `cancel`. Owning
packages are `[linear]`. Packing preserves the hidden state's resident custody,
contained loans, zero-sized obligations, storage claims, carry demands and code
dependencies, and storing a consuming operation admits no automatic cleanup by
itself. The package's owner may select an ordinary cleanup machine with
`dropped by` that takes the package apart with a consuming pattern, opens its
witness and calls a stored consuming operation on the hidden owner; its eligibility is checked from that operation's callable
contract, and the package stays `[linear]`. A
pending or failure outcome returns the remaining ownership as a package exposing
only the operations valid in that state, under
[consumers and cleanup](ownership.md#consumers-and-cleanup).

An abstract witness introduced by a valid opening may construct another package
within its scope under the ordinary construction, lifetime, custody and contract
checks. This permits repacking after an incomplete settlement. It grants no
equality with another package's witness and extends no contained loan.

## Representation, costs and replacement

Uniform borrowed or owning fields plus indirect callable descriptors give one
external layout without a generated sum of all possible witnesses. No global roster,
tag assignment, or type/layout identity changes when another construction uses
a new sized witness. There is no implicit box or runtime witness equality test
for a source-checked pairing. Descriptor representation and dispatch still have
costs; loan/effect checking and external artifact verification remain required.

Type openings and pairings must be represented explicitly in checked evidence
and carried through [Terminal dispatch](../terminal-psi/dynamic_dispatch.md).
Lowering must consume the established plan rather than re-deriving witness
identity from source expressions. External structural checks preserve these
associations under [Psi admission](../terminal-psi/product.md#publication-and-separate-consumption).
Trusted producer judgments may be reused; interpretation still checks actual
pairings, custody and call access. An untrusted asserted pairing grants no authority.

This does not authorize arbitrary new code to enter a closed product. Build
admission, reachability, code retention and replacement contracts still apply.
No Fused-only global-roster mechanism is required, but a package containing local
machine descriptors, borrows or owned state whose layout a component defines
cannot cross an Independent boundary unchanged.
Use its admitted boundary representation or a local proxy. Quiescence does not
convert old state or release stored code dependencies: retain the old era, drain
the packages, or consume and reconstruct them around explicit state migration.
