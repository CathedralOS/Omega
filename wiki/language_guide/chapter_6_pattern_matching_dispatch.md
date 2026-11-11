# Chapter 6: Pattern matching and dispatch

`match` picks a value and `transition` picks what runs next. Both test their
arms in order, run only the first that matches, and must cover every case.

## Match

```omega
let category: i32 = match code {
    2 -> 10
    13 -> 20
    _ -> 0
};
```

The subject is evaluated once. Each arm produces the same type.

## Transition

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

The chosen arm is the last thing the machine does
([Chapter 4](chapter_4_nested_machines_transitions.md)).

## Data and case patterns

A case pattern can bind the payload:

```omega
data Command {
    command_id: u64;

    op: data Operation {
        case GetUser(user_id: u64);
        case CountUsers;
    };
}

match command.op {
    Command::Operation::GetUser { user_id } -> lookup(user_id)
    Command::Operation::CountUsers -> count()
}
```

A record pattern binds or tests fields:

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

| In a pattern | Means |
| --- | --- |
| `field` | Bind the field to a local of the same name. |
| `field as name` | Bind it under another name. |
| `field as _` | Ignore it. |
| `field: value` | Test that it equals `value`. |
| `..` | Ignore the fields not mentioned. |

Patterns are one level deep: a record pattern does not contain a case
pattern. When a record holds a sum, match the field, as above.

## Testing two things at once

Put them in a tuple:

```omega
match (command.command_id, command.op) {
    (0, _) -> reject()
    (_, Command::Operation::GetUser { user_id }) -> lookup(user_id)
    (_, Command::Operation::CountUsers) -> count()
}

transition (player_defeated, enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

Arms are tried top to bottom, so `(true, _)` wins when both are true.

## Irrefutable bindings

When a pattern cannot fail, bind with `let` instead of a one-arm match:

```omega
let Command { command_id as _, op as operation } = command;
let (_, operation) = command;        // the same, by position
```

A `let` pattern that could fail is rejected; use `match` for that.

## Arms carry facts

Inside an arm, the compiler knows what the pattern established:

```omega
let empty: bool = items.len == 0;
transition empty {
    true -> no_items()
    false -> inspect_first(items)     // here: items.len != 0
}
```

Matching a case also brings in what the type promises about that case. For
the `Interval` and `Value<T>` types of
[Chapter 1](chapter_1_data_values_literals.md#constraints-on-individual-cases),
the `Range` arm knows `lo <= hi` and the `Integer` arm knows `T == i32`. A fact
belongs to its arm and is lost if the subject is modified.

## Exhaustiveness

```omega
transition command {
    Command::Look -> look()
    Command::Quit -> finished()
    Command::Invalid -> invalid_command()
}
```

Add a case to `Command` and this stops compiling until the case is handled.
A `_` arm opts out of that check. A case the types make impossible, such as
`Value::Boolean` when the subject is a `Value<i32>`, does not need an arm.

## Domain patterns

A pattern can test membership in a [domain](chapter_8_domains.md) that has a
runtime test:

```omega
transition player {
    Player::Dead -> game_over()
    _ -> continue_game()
}
```

## Where next

[Chapter 7](chapter_7_types_constraints_invariants.md) explains the facts that
arms and calls establish. The complete rules are in
[patterns](../spec/language/patterns.md).
