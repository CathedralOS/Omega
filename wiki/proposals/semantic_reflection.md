# RFC: Semantic reflection and typed member expansion

Status: open design proposal. None of the `reflect` API, `expand` syntax,
member selectors, or runtime descriptor projection below is approved or
implemented by this RFC. Examples are candidate source, not executable tests.
Existing generic/type-equality decisions remain settled; this proposes the
separate reflection facility rather than silently extending them.

## Customer and recommendation

An application wants a read-only property inspector over records and active sum
payloads without manually repeating their fields. A serializer is a second
customer for the same field traversal, but has independent encoding and schema
evolution obligations. Neither customer requires runtime type construction,
arbitrary invocation by name, or inspection of the compiler's private IR.

Start with typed member expansion over an explicitly named type. Reuse exact
declaration identity and ordinary access checking. Let eligible ordinary machines
compute metadata and layout/access plans at compile time, and materialize only
explicitly requested metadata at runtime. Keep data geometry, access authority,
and reflective discovery separate.

The simpler baseline is handwritten inspectors and the existing fixed core
trait synthesis. Retaining that baseline is viable if a general expansion
mechanism adds more complexity than it removes. A runtime-only descriptor loop
is another option, but needs erased typed operations even when callers want
only static serialization. This proposal favors one checked expansion mechanism
over independent compiler generators for every serializer, inspector, and codec.

## Existing boundaries

| Owner | Already defines | This proposal must not infer |
| --- | --- | --- |
| [Generics](../spec/language/generics.md) | Type equality, structural binding, static versus runtime indices. | Arbitrary field enumeration or type-as-runtime-value operations. |
| [Semantic evaluation](../spec/language/evaluation.md) | Eligible ordinary machines compute pure values and plans; no separate comptime language. | Ambient compiler, host, or source access. |
| [Layout](../spec/layouts/plans.md) | A compiler-provided schema feeds an authored `plan(schema) -> Plan`. | Backing, readable content, or permission to access fields. |
| [Placed](../spec/resources/placed_access.md) | Validated layout/access demand plus admitted storage, provenance, loans, and operation-specific accessors. | A raw reference merely because a field has an offset. |
| [Recasts](../spec/layouts/recasts.md) | Representation-compatible ordinary views under proof and loan rules. | Bypassing a Placed access policy. |
| [Dynamic dispatch](../spec/terminal-psi/dynamic_dispatch.md) | Exact selected conformance, instance, callable rows, and ownership. | A universal `invoke(name, arguments)` or arbitrary boxed values. |

The current [core layout schema](../../source/library/core/layout.omg) exposes
field keys, sizes, alignments, kinds, identities, cases, and tombstones. Its
private fixed capacities are implementation limits, not the proposed reflection
model. It does not expose arbitrary predicates or field types as runtime objects.

The existing [generator direction](../spec/language/evaluation.md#trait-bodies-and-generators)
restricts reflection to conforming Self and leaves unroll syntax unsettled.
Allowing generic inspectors of an explicit T is a proposed extension to that
scope, not a claim that current ordinary trait defaults already have this access.

## Candidate source surface

Use a core module named `reflect` for compiler-supplied semantic queries. Exact
compiler identities select these operations; a same-named user module gains no
reflection privilege. Each query still obeys source dependency and visibility.

| Candidate form | Role |
| --- | --- |
| `reflect::fields<T>()` | Static sequence of direct common/record fields in authored declaration order. Includes erased fields as descriptions. |
| `reflect::cases<T>()` | Static sequence of sum cases in authored order, not wire-ID order. |
| `expand Field in sequence { body }` | Check a typed body instance for each static member selection. No runtime loop over type objects. |
| `Field::Type` | The selected field's exact type expression, retaining qualifications and index bindings. |
| `Field::name()` / `Field::identity()` | Descriptive name bytes / optional authored schema number, never lookup authority. |
| `Field::borrow(value)` | Checked shared projection of an addressable ordinary field from the exact owner. |
| `Field::key(schema)` | Rejoin this field to the matching supplied layout/access schema and return its compiler-issued key. |
| `Case::is_active(value)` / `Case::fields()` | Runtime discriminator observation with selected-case evidence / static payload field sequence. |
| `reflect::reject(message)` | Reject an admitted expansion alternative during compilation, not a runtime trap or an assertion that can waive obligations. |

`Field` and `Case` are scoped static declaration selections, neither runtime
variables nor freely constructible integer handles. `Field::Type` is a type
position, not a first-class Type object. These are new compiler mechanisms.
The list returned by fields/cases is not an ordinary runtime collection whose
elements happen to be different types. Its membership is fixed before expansion.
General first-class member parameters and arbitrary sequence transformations
are not required for the first slice.

This separates homogeneous metadata processing from heterogeneous code
elaboration. A machine may calculate sizes or copy names from ordinary schema
data. It cannot turn a runtime field-description record into a new type argument.
The expansion binds the field's type before checking each body instance; that is
the capability an ordinary loop over metadata cannot replace.

### Read-only record inspector

The output adapter and its domain-specific methods are ordinary library code.
This example assumes it accepts shared u32/f32 values and has no external reach;
an effectful output interface would expose its ordinary complete envelope.

```omega
use omega::core::reflect;

data Player {
    health: u32;
    speed: f32;
}

machine inspect_record<T>(value: &T, output: &mut InspectorOutput) {
    expand Field in reflect::fields<T>() {
        if Field::Type == u32 {
            output.show_u32(Field::name(), Field::borrow(value));
        } else if Field::Type == f32 {
            output.show_f32(Field::name(), Field::borrow(value));
        } else {
            reflect::reject("field needs an explicitly selected inspector");
        }
    }
}
```

For Player this elaborates to two ordinary calls over its exact fields. It does
not snapshot or move the whole Player. Each shared subloan has the call's
lifetime and ends under normal borrow rules. A zero-field record emits no calls;
this particular inspector rejects sums rather than silently visiting only their
common fields. Describe-only traversal may include erased fields, but borrowing
one for a runtime inspector rejects. The author must explicitly handle or waive
unsupported fields rather than accidentally claim full coverage.

The type branches are static. Each admitted alternative must check under its
exact field type. This does not permit invalid operations in an alternative
that some other consumer could select. There is no source-text substitution,
token concatenation, capture by variable spelling, or hidden macro language.
Generated operations retain the expansion and member coordinates for diagnostics.

For a reusable serializer, named codec conformances remain explicit. A generator
cannot search for a unique visible codec merely because it can see the field.
The bounded first version can branch on a closed set of primitive types and
delegate composite types through explicitly supplied machines/conformances.

### Active sum payloads

```text
expand Case in reflect::cases<T>() {
    if Case::is_active(value) {
        expand Field in Case::fields() {
            visit_supported_field(Field::Type, Field::borrow(value), output)
        }
    }
}
```

This algorithm sketch reuses the record visitor's typed branches; it introduces
no first-class Type argument to `visit_supported_field`. The outer expansion
generates discriminator control flow, not unconditional reads of every payload.
The selected branch establishes the exact case evidence needed by its field
projections. Common fields are traversed once with fields<T>, separately from
payloads. Mutations that can change the active case invalidate that evidence;
the borrowed read-only receiver prevents conflicting ordinary mutations.

All source cases must be represented or explicitly waived. Wire IDs and retired
IDs are metadata, not live runtime cases. Mixed records/sums and zero cases keep
their existing validity and ZII semantics.

## How Layout and Placed fit

The reflected field selection names the semantic declaration. The Layout schema
provides target-resolved geometry for that declaration. Both must rejoin the
same owner, static application, field path, and semantic revision; similar names
or offsets are not sufficient. Do not create a second reflection-owned layout
engine or expose a bare offset as permission.

A candidate integration point for a policy specialized to an explicit T is:

```text
machine access_plan<T>(schema: Schema) -> AccessPlan {
    let mut plan = AccessPlan::inaccessible(schema);
    expand Field in reflect::fields<T>() {
        let key = Field::key(schema);
        plan = plan.with(key, selected_access_for(Field, schema));
    }
    plan
}
```

`selected_access_for` denotes explicit policy logic, not automatic permission
inference; this is an algorithm sketch, not another approved callable form.
The caller must supply the compiler's schema for the same T; a forged/mismatched
schema cannot acquire valid keys. Erased fields have no physical key. The existing
Access policy still receives its validated layout and checks operation demand;
the sketch shows only the declaration-key join. Layout planning cannot query
its own unfinished plan through reflection and create a recursive dependency.
Declaration inspection is available after the relevant type structure is formed;
geometry observations wait for layout closure. Dependency cycles reject.

Ordinary owned RAM stays ordinary T with normal projection. Reflection should
not require converting it to Placed. Packed/fragmented geometry may prevent a
direct shared reference even when reading a copied scalar would be valid; the
initial `borrow` operation rejects a non-addressable field rather than inventing
a pointer or temporary reference with the wrong lifetime.

For `Placed<P, T>`, propose a distinct selection operation inside a field
expansion where Field is already bound:

```text
accessor = reflect::accessor<P, T>(view, Field)
value = accessor.read()
```

The static Field selector must name an accessor exposed by that exact placement.
Selecting it performs no device read. Calling `read` is the ordinary accessor
operation, available only when the actual access family and receiver support it.
The view carries the admitted supply, residency/loan, range, and binding evidence;
Field metadata supplies none. This new selector reuses existing accessor
generation rather than creating a reflection-specific read primitive.

A generic read-only inspector must not silently invoke External Take, atomic
loads, or even repeatable MMIO reads. Those can consume data, observe devices,
or require ordering and authority. Authors can explicitly select a device-aware
inspection contract, including its read operations and effects. Inaccessible or
BindingPrivate fields remain unavailable to unauthorized consumers. Ordinary
recasts cannot bypass this distinction.

## Runtime metadata without mandatory RTTI

An inert projection may be explicitly retained:

```omega
const PLAYER_DESCRIPTION = reflect::record_metadata<Player>();
```

Propose that this returns ordinary owned, copy-eligible data: selected member
names, schema numbers, kind tags, and normalized descriptive facts. Its finite
storage must satisfy existing constant materialization rules; it cannot return
a slice pointing into compiler memory. Names are literal data deliberately
retained by this request, not required per-object metadata. The compiler's
static member selections and type binders themselves do not become runtime
values. Static dependencies whose descriptions are not requested need not emit
runtime metadata, but generated code and retained tables still have measurable
code/data and build-time costs.

A runtime editor can display this data and invoke an explicitly selected typed
visitor such as `inspect_record<Player>`. A runtime-selected object can instead
use an ordinary declared Inspectable conformance and borrowed dynamic interface.
That conformance binds the matching description and visitor; it does not accept
an arbitrary address plus a claimed type ID. A future per-property table would
need checked getter/setter adapters and exact instance/member binding. This RFC
does not assume those tables can be materialized as raw function addresses in
ordinary constants.

Runtime TypeId/downcast is a separate optional interface. Exact local type
identity is not a stable wire ID, an object lifetime guarantee, or evidence of
representation validity. Cross-component schema compatibility follows its own
version/admission contract. A descriptive runtime record is not the compiler's
authoritative type object and cannot override static `T == U`.

## Visibility, ownership, and verification

Inspection must use the requesting scope's normal declaration access. Metadata
can describe only what that scope may inspect, or an explicitly owner-published
description. The default exact fields query rejects incomplete access rather
than silently hiding fields and allowing a purported full serializer to omit
them. Public structural fields keep their normal visibility. Private issuer
identities retained for verification do not become selectable reflectively.

A library template does not inherit arbitrary private access from its caller.
An owner may write a wrapper/conformance in its authorized scope that deliberately
exports a description or checked operation. Defining the authority rule for
reusable owner-scoped expansion is a decision before expanding beyond the public
structural first slice; there is no implicit friend privilege.

Borrowing a linear field lends it, never duplicates or consumes it. Readability
is not copyability. General mutable reflection is deferred: arbitrary setters
can break dependent invariants, destroy required custody, or invalidate siblings'
loans. Initial editing should call owner-authored setters with explicit outcomes.
Likewise method discovery does not authorize invocation: requires, argument
ownership, reach, suspension, blocking, crashes, and progress remain binding.

Expansion is compiler elaboration followed by ordinary checking. It cannot mint
domains, waive preconditions, assume a false admission, or skip the selected
operation's laws. Numeric endpoints can be described as static values or symbolic
dependencies without asking a solver for their tightest range. Recursive type
graphs must retain handles/edges rather than infinitely expand nested types;
recursive visitors remain ordinary machines with their own progress contracts.

Member identities are handle-first inside compilation. Stable authored schema
numbers, display names, compact coordinates, and canonical artifact commitments
retain their distinct purposes. Query and expansion results depend on exact
declarations, selected rules, visibility context, and any target-layout inputs.
Those dependencies invalidate caches and derived artifacts when relevant fields
change. Immutable descriptors do not substitute for source-to-generated-operation
correspondence or independent verification of the generated body.

## Design lab and alternatives

No experiment has run. These cases distinguish the candidate from superficial
metadata or unsafe offset-based reflection:

| Case | Acceptance target |
| --- | --- |
| Record with u32/f32 fields | Two exact typed visitor calls, no per-object header or copied record. |
| Added unsupported field | Explicit diagnostic/waiver, not a silently incomplete serializer. |
| Sum with an owned payload | Only the active payload is borrowed; no duplicate linear custody. |
| Erased field | Describable under visibility, but no executable field borrow or physical key. |
| Wrong schema passed to Field::key | Reject before a layout/access plan can claim correspondence. |
| Packed or fragmented field | No forged addressable reference; selected legal projection or rejection. |
| Placed MMIO FIFO | Metadata inspection causes no transfer; Take is not silently treated as read. |
| BindingPrivate accessor | No new consumer authority from field enumeration. |
| Runtime endpoint in a range | Retain symbolic dependency, never invent a const value. |
| Runtime metadata omitted | No metadata retention merely from compiling a typed visitor. |
| Two equal-layout nominal types | Remain different identities; descriptors cannot cross-cast them. |

Alternatives to compare before approval:

- Compiler-provided field visitor invocation instead of `expand`: fewer source
  constructs, but needs a precisely typed polymorphic visitor contract and may
    obscure the per-field checking scope. Compare candidate source examples on the
    same inspector and serializer before choosing; no implementation experiment
    is authorized here.
- Existing Self-only trait generators: smaller visibility extension, but less
  convenient for reusable generic inspection and owner-generated adapters.
- Runtime descriptor loop only: adequate for a property editor, with retained
  metadata and erased adapters; needlessly indirect for static serialization.
- Exposing compiler AST/IR: couples libraries to private implementation stages
  and is not required by these customers. Prefer a closed semantic query surface.
- Automatic ambient codec discovery or arbitrary `get(name) -> pointer`: neither
  preserves explicit conformance choice, field validity, nor access authority.

## Decisions before implementation

Agree on `expand` versus an explicit compiler-mediated visitor, the initial query
and member-selector inventory, and the visibility scope for generic expansion.
Specify static query dependency order and the owned runtime metadata schema.
Decide whether placed accessor selection belongs in the first implementation
or follows an ordinary-record inspector; it must preserve its independent access
rules either way. General field mutation, callable-by-name adapters, runtime
downcasting, and arbitrary declaration/body generation can remain later designs.

An initial prototype should cover public record inspection, active sum payloads,
and one explicit metadata projection; inspect generated operations and compare
them with handwritten equivalents. No implementation task or change to normative
reflection restrictions follows merely from adding this RFC.