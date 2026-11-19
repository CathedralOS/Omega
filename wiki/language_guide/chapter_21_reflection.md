# Chapter 21: Reflection

Reflection lets a serializer, inspector or layout policy handle every field
of a type without being written by hand for each one. It runs at compile time
and produces ordinary typed code.

## Visit a record's fields

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

The compiler calls `show_player_field` once per field, with `Value` set to
that field's type. Add a field of a type the callback does not accept and you
get a compile error at that field, so nothing is skipped silently.

The visit covers the type's own fields in declaration order. Recurse into
nested records yourself.

## Describe a type

```omega
reflect::schema<Player>()            // fields, cases, names and types
reflect::field(Player::health)       // one field
reflect::callable(inspect_player)    // one machine
```

These describe declarations. To read a player's health you still write
`player.health`, and to find its byte offset you ask its
[layout](chapter_20_memory_layout_abi.md).

`reflect::parameter(Requirement::formal)` describes one declared formal of a
boundary requirement. For example, a Build-owned function-table binding uses
this descriptor to identify which argument supplies its table. It does not
read that argument at compile time. Parameter, field and callable descriptions
have distinct types; none grants authority. See
[formal-parameter selectors](../spec/language/reflection.md#formal-parameter-selectors).

## Serializers

A serializer combines the field visit with an encoder chosen per field:

```text
Compile time: field type + field description -> selected encoder
Runtime:     selected encoder + field borrow + output -> encoded bytes
```

Encoders are chosen by declaration, not by name string, so renaming a field
cannot select a different one by accident. For a sum, the visitor reports the
active case and reads only that case's payload.

## Reflection grants no extra access

A visitor receives ordinary borrows of fields and can do with them only what a
borrow allows. Reflecting on a type does not get around a domain's access
policy.

## Where next

[Chapter 22](chapter_22_inline_assembly.md) covers inline assembly. The
complete rules, including the encoder-selection API, are in
[reflection](../spec/language/reflection.md).
