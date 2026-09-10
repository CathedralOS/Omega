# Chapter 21: Semantic Reflection

Reflection lets an inspector or serializer reuse a type's declared structure
without writing every field access by hand. The compiler supplies descriptions
and checked typed access; ordinary library machines decide what to display,
encode, or edit. There is no universal object header or serializer built into
the language.

The [reflection contract](../spec/language/reflection.md) owns the rules.
This chapter teaches those rules, not current compiler completeness. The code
below illustrates API roles: sink declarations, exact callable requirements,
and library encoding definitions are omitted, so these are not runnable projects.

## Describing A Type

`reflect::schema<Player>()` produces an owned `reflect::TypeSchema` graph for
the exact static application `Player`. Descriptions include field and case
identities, names, field types, relevance, and declaration order. Graph-local
indices connect records; they are not persistent identities or compiler pointers.
Recursive references use graph edges with substitutions, not endless expansion.

An ordinary machine can walk these homogeneous description records to calculate
labels, select a display order, or check coverage. Descriptions include erased
fields too. A type key identifies a fully static type application under ordinary
type equality; runtime-dependent applications instead retain symbolic subject
relationships. Neither is an offset, a cast, or permission to access fields.

The schema answers semantic questions: which member belongs to which exact
type? [Layout](chapter_20_memory_layout_abi.md) separately answers where that
member is represented. Geometry requires a validated selected layout. A layout
planner cannot observe its own unfinished result.

## Inspecting Actual Values

Consider an owner-authored inspector for this record:

```omega
data Player {
    health: u32;
    speed: f32;
}

machine show_player_field<Value>(
    output: &mut InspectorOutput,
    field: FieldInfo,
    value: &Value
)
where Value == u32 || Value == f32;
{
    transition {
        Value == u32 -> output.show_u32(field.name, value)
        Value == f32 -> output.show_f32(field.name, value)
    }
}

machine inspect_player(player: &Player, output: &mut InspectorOutput) {
    reflect::visit_runtime_fields<Player, show_player_field>(player, output);
}
```

The compiler selects `health` with `Value = u32`, then `speed` with `Value = f32`.
It checks the corresponding callback applications and ordinary borrows. At
runtime those calls use the actual player's values and the same output
context. Compilation does not read the player or write to the output.
Adding an unsupported field rejects at that member; reflection does not skip it.

An ordinary runtime loop over descriptions cannot give its loop variable a
different static type on each iteration. Typed visitation supplies that
per-member specialization. The callback is an ordinary named machine and the
output context is an ordinary argument. No lambda or capture syntax is needed.

The complete record visit covers each immediate non-erased field once in
declaration order, including fields with zero runtime size. Erased fields have
no runtime borrow. An empty record makes no field calls. Recursive descent is
an explicit consumer operation, not an automatic consequence of visiting a field.

## Selecting An Encoder

A serializer uses the same checked member access with different callbacks.
User-defined types need explicitly selected encoding conformances, not a fixed
compiler list of encodable types. Selection and encoding happen at different
times:

```text
During compilation, for each included field:
    instantiate the authored policy at the field's exact Value type
    pass its description and a scoped EncodingSelection<Value> receiver
    select the exact encoding conformance through authored control flow
    check the resulting typed encoder call

At runtime:
    borrow the actual field
    invoke that selected encoder with the ordinary output context
```

The policy can provide type defaults and member-specific overrides. For example,
select the ordinary `u32` encoding by default, but select a different checked
`u32` encoding for the exact `Player::health` member. Resolve the member by its
declaration identity, not a display-name hash. The named encodings in this
example are library choices, not automatically supplied conformances.

The selection operation has the form `selection.select<HealthEncoding>()`,
where `HealthEncoding` names an exact conformance application, not an encoder
type from which the compiler searches for one. The evaluation root owns the
selection receiver and freezes its choices into an owned symbolic result for
checking. The policy never mutates compiler memory or returns a reference to it.

Each successful included-field policy completion selects exactly one encoding.
Selecting none or selecting two rejects; exclusion must be explicit in a policy
whose coverage is partial. There is no conformance discovery or implicit
"choose whichever encoder is available" behavior. A type default is ordinary
authored policy, not a compiler-maintained registry.

The selected `Value` is the full field type, including its qualifications and
dependent constraints. A deliberately selected adapter may use ordinary
weakening to encode an underlying value. Reflection cannot silently strip those
constraints to find an easier encoder. The policy runs without reading a future
runtime object; the selected encoder performs the actual runtime work.

## Access And Borrowing Still Apply

Generate projections in the owner's scope, or another scope already authorized
to select those fields. A reusable serializer package does not inherit the
application's private access or acquire a dependency on the application simply
because its generic parameter becomes `Player`.

Instead, the owner supplies a checked visitor or explicit conformance to the
generic serializer. The library invokes that operation. Its callback may carry
the received field borrow and use supplied operations; it gains no right to
enumerate a private nested type. Recursive inspection uses that type's selected
owner-authored adapter. Public structural data already publishes its fields;
reflection does not introduce individually private fields within that model.

Each callback invocation reborrows the context and field under fresh
invocation lifetimes. A nonretaining callback cannot save that field borrow in
a longer-lived context. Retention requires an expressible checked relationship
that remains compatible with subsequent calls. Borrowing never copies a linear
resource or grants mutation.

Choose an explicit callback contract covering effects, failures, progress, and
resources. Reach propagates through static callback calls under the ordinary
[reach rules](../spec/language/effects.md#static-callback-reach-dependencies);
suspension/blocking remain fixed by the requirement. A finite list of fields
does not prove that their encoders terminate or roll back partial output.

## Cases, Recursion, And Storage

A sum visitor reports the active case even when that case has no payload.
Otherwise two different nullary cases could serialize identically merely
because both contain zero fields. It handles common fields once and only
projects the active payload after runtime case selection. All reachable case
applications are checked during compilation; inactive payloads are not read.

Recursive derivation reuses the pending exact encoder application instead of
instantiating an infinite sequence. That reuse does not discharge its typing,
coverage, or proof obligations: the completed recursive application must check.
Runtime traversal is a separate problem. Use tail-only traversal where suitable,
or explicitly provision work storage for pending branches. Shared and cyclic
object graphs additionally need an authored reference-identity and visit policy.

Ordinary RAM uses ordinary borrows. Packed fields without addressable references
need an independently specified copied-read route; an inspector cannot fabricate
a temporary reference with a false lifetime. Placed inspection uses the exact
authorized accessor operations. Descriptions perform no device read, and a
destructive MMIO Take is not an ordinary shared observation.

## Runtime Descriptions And Decoding

Request runtime descriptions explicitly with `reflect::metadata<Player>()`.
They are owned ordinary data, not references into compiler memory. A runtime
editor can retain selected checked adapters alongside descriptions; runtime
lookup chooses among those operations, not a new type argument or arbitrary
address. Retaining names does not automatically retain setters or constructors.

A decoded document is still a document. Migration, defaults, validation,
construction, and resource acquisition use owner-defined operations before a
valid `Player` exists. A live patch likewise needs the owner's validation and
failure contract. Metadata cannot mint qualifications, duplicate linear fields,
or bypass a constructor merely because the document carries the right type key.

Use reflection to remove repetitive member handling. Keep encoding formats,
logical collection adapters, editor policy, object lifecycle, and failure
behavior in ordinary library code.
