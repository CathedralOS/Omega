# Chapter 21: Reflection

Reflection lets a serializer or inspector handle fields without hand-writing
each projection. Start with a typed visitor when you need actual values; use
a schema when you need descriptions such as field names and types.

## Visit a record's fields

This example assumes an inspector library supplies `InspectorOutput`,
`show_u32`, and `show_f32`. The visitor receives each field with its exact type:

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
requires Value == u32 || Value == f32
{
    transition {
        Value == u32 -> show_u32(output, field.name, value)
        Value == f32 -> show_f32(output, field.name, value)
    }
}

machine inspect_player(player: &Player, output: &mut InspectorOutput) {
    reflect::visit_runtime_fields<Player, show_player_field>(player, output);
}
```

The compiler specializes the callback for `health: u32` and `speed: f32`.
At runtime, each call borrows the corresponding field and writes through the
output context. Adding a field of an unsupported type produces an error at
that field, so the inspector cannot silently omit it.

The visit covers immediate runtime fields in declaration order. Visit nested
records explicitly when you want recursive inspection. Erased proof fields
have no runtime value to pass to the callback.

## Describe a type

`reflect::schema<Player>()` returns an owned `reflect::TypeSchema` describing
fields, cases, names, and types. Recursive types use graph edges in the schema.
Request retained runtime descriptions with `reflect::metadata<Player>()`.

A schema walk can choose labels and display order. Typed visitation supplies
the actual field access; a runtime loop over descriptions cannot choose a new
static field type on each iteration.

Select one declaration directly when you only need one:

```omega
reflect::field(Player::health)       // FieldInfo
reflect::callable(inspect_player)    // CallableInfo
```

These queries describe declarations. To borrow a player's current health, use
`&player.health`. To find its byte offset, use the selected [layout](chapter_20_memory_layout_abi.md).

## Select encoders

A serializer combines field visitation with explicitly chosen encoding
conformances. A compile-time policy can select a default encoding for `u32`
and override it for the particular `Player::health` field.

```text
Compile time: field type + field description -> selected encoder
Runtime:     selected encoder + field borrow + output -> encoded bytes
```

For each included field, the policy selects exactly one conformance through
its `EncodingSelection<Value>` receiver. The choice uses declaration identity,
so renaming a display label cannot accidentally select another field's encoder.
The [reflection reference](../spec/language/reflection.md) gives the policy API
and coverage rules.

The encoder receives the complete field type, including its domains and
constraints. An adapter may deliberately encode an underlying value when its
contract permits that conversion.

## Preserve access and lifetimes

An owner-authorized visitor can pass checked field borrows to a reusable
serializer. The serializer gets the access in those borrows. It cannot inspect
additional restricted fields by reflecting on the type again.

Each callback borrows the field and output context for that invocation. Saving
the field reference into a longer-lived output object requires an explicit
lifetime relationship. Effects and suspension follow the callback contract,
as they do for [other generic traversals](chapter_13_generics.md).

Packed or device fields may require copied reads or planned access operations
instead of an ordinary reference. Use the access route supplied by their layout
and policy.

## Handle cases and recursive data

A sum visitor reports the active case as well as its payload. Even a case with
no fields needs that tag in a serialized format. Only the active case's
payload is read.

For trees, provision work storage for pending branches or use a tail traversal.
For shared or cyclic graphs, define how references are encoded and how already
visited objects are recognized. Reflection supplies the members; the library
chooses the traversal and format.

## Build an editor

An editor can display a detached document, edit it, and submit it to a checked
operation that validates and applies the change. Creating a live value still
requires its constructor, domain proofs, and any resource acquisition.

Do not write through a schema into a paused script's locals. The script may
retain borrows or rely on facts stronger than the type's default invariant.
Use the host's checked API when the object's loans permit the update. See
[embedding](chapter_24_embedding.md#calls-messages-and-editors).
