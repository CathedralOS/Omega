# Chapter 6: Pattern matching and dispatch

Patterns select an arm and contribute facts about the saved subject. Expression
matching returns a value; transition matching chooses a control edge. The
[pattern specification](../spec/language/patterns.md) owns their shared rules.

## Match expressions

A value-position match selects one result:

```omega
let category: i32 = match code {
    2 -> 10
    13 -> 20
    _ -> 0
};
```

A match evaluates the subject once, tests arms in order, and
evaluates only the selected arm. Arm values need compatible types and the
dispatch must be exhaustive.

## Data and case patterns

A record decomposes into its fields and a sum is matched by case. When a record
holds a sum, match the sum through its field:

```omega
data Command {
    command_id: u64;

    op: data Operation {
        case GetUser(user_id: u64);
        case CountUsers;
    };
}

machine is_lookup(op: &Command::Operation) -> bool {
    match op {
        Command::Operation::GetUser { user_id as _ } -> true
        Command::Operation::CountUsers -> false
    }
}

machine routes_to_user(command: &Command) -> bool {
    is_lookup(&command.op)
}
```

A named record pattern binds fields, `Command { command_id as _, op }`, and the
positional equivalent is `(_, op)`: fields in authored order. Binding names are
arbitrary. Field renaming uses `as`, while `field: value` tests a value.
Patterns account for all components unless `..` explicitly waives the remaining
ones. Nested matches retain outer bindings and still-valid facts, and each
dispatch must be exhaustive. Ignoring a binding does not waive cleanup.

The helper takes an ordinary type. It sees the sum's own contract and has no
way to reach `command_id`, so a fact about the whole command that it needs
becomes one of its `requires` clauses. Matching `command.op` where the whole
`Command` is in hand combines the selected case with the command's invariants.
The [pattern specification](../spec/language/patterns.md#data-and-case-decomposition)
owns the full rule.

## Irrefutable bindings

When a pattern only decomposes the value, a local binding avoids a one-arm outer
match. Inside a machine holding `command: &Command`, bind the fields before
matching:

```omega
let Command { command_id as _, op as operation } = command;
```

The positional equivalent is `let (_, operation) = command;`. Evaluate the
initializer once. The checked type and live facts must prove the pattern cannot
fail; otherwise use ordinary match arms. A binding has no hidden trap, assertion
or fallback. The original nested-match form remains valid, and there is no
special `match expression as Pattern` header.

## Transition dispatch

A transition can choose the next machine to invoke terminally:

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

Only the chosen edge runs. Its pattern facts help establish the target machine's
requirements, but do not waive its ordinary type, ownership, or authority checks.

## Tuple patterns

Tuple dispatch combines explicit conditions:

```omega
transition (player_defeated, enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

The first arm deliberately wins when both conditions hold. A component wildcard
ignores that component without asserting anything about its value.

A component can be a field and another a sum the same record holds, which is
how one arm tests both:

```omega
match (command.command_id, command.op) {
    (0, _) -> reject()
    (_, Command::Operation::GetUser { user_id }) -> lookup(user_id)
    (_, Command::Operation::CountUsers) -> count()
}
```

Patterns are one level deep, so a record pattern does not contain a case
pattern; match the held sum as its own subject or as a component here.

Positional data patterns use the same parenthesized style on a data subject,
but their components come from its declared fields,
not an implicit conversion to unrelated tuple storage. Named and positional data
patterns do not change representation or add runtime fields.

## Named facts before dispatch

Name complicated conditions before branching:

```omega
let empty: bool = items.len == 0;
transition empty {
    true -> no_items()
    false -> inspect_first()
}
```

The false arm establishes nonempty input. A target using that fact must receive
the corresponding view or value through its machine parameters. Naming a fact
does not make the source binding ambient in the target machine.

## Exhaustiveness

A complete case dispatch fails when an added case is not covered:

```omega
transition command {
    Command::Look -> look()
    Command::Quit -> finished()
    Command::Invalid -> invalid_command()
}
```

A wildcard opts into handling other cases. Pure case-union domains can contribute
finite case coverage; arbitrary predicate-domain tests do not establish coverage
merely because every known example matches. Prior facts can exclude impossible
subjects, but a failed proof cannot be treated as an unreachable arm.

A matched [case constraint](chapter_1_data_values_literals.md#constraints-on-individual-cases)
supplies both type equations and payload facts to its arm. For `Value<i32>`, the
Boolean case's `T == bool` contradicts the subject type, so a proved contradiction
can exclude it from coverage. Harder case predicates require established proof,
not a guess: otherwise cover the case or explicitly discharge its dead arm.
An arm's facts do not leak into other arms or survive invalidating mutation.

## Tail dispatch

A transition ends the current straight-line segment. A machine target, nested
or otherwise, is a final call whose result completes the current invocation.
A value arm completes directly. Suspending/blocking calls retain their exact
acknowledgement markers; lexical nesting grants no exemption.
[Chapter 4](chapter_4_nested_machines_transitions.md#final-calls-including-suspension)
explains result forwarding and the retained loan/cleanup obligations.

## Domain patterns

A runtime domain pattern needs a finite, pure executable test and any required
establishment provenance. Proof visibility alone does not make a test executable.

```omega
transition player {
    Player::Dead -> game_over()
    _ -> continue_game()
}
```

The selected domain fact belongs to the saved subject and its current revision.
Mutation can invalidate it. Overlapping domains use ordinary first-match order;
testing predicates cannot mint a routed qualification. See
[domains](chapter_8_domains.md).
