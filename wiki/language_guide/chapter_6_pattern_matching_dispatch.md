# Chapter 6: Pattern Matching And Dispatch

Patterns select an arm and contribute facts about the saved subject. Expression
matching returns a value; transition matching chooses a control edge. The
[pattern specification](../spec/language/patterns.md) owns their shared rules.

## Match Expressions

A value-position match selects one result:

```omega
let category: i32 = match code {
    2 -> 10
    13 -> 20
    _ -> 0
};
```

The intended semantics evaluate the subject once, test arms in order, and
evaluate only the selected arm. Arm values need compatible types and the
dispatch must be exhaustive.

The Rust parser's current arithmetic expansion does not implement general
selective evaluation. Do not treat an accepted arithmetic-shaped example as
coverage for effects, owned values, or general result types. The
[source processing note](../../omega-rust/psi/pipeline/README.md#lexing-and-parsing)
records the gap.

## Transition Dispatch

A transition chooses the next state:

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

Only the chosen edge runs. Its pattern facts help establish the target state's
requirements, but do not waive its ordinary type, ownership, or authority checks.

## Tuple Patterns

Tuple dispatch combines conditions:

```omega
transition (player_defeated, enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

The first arm deliberately wins when both conditions hold. A component wildcard
ignores that component without asserting anything about its value.

## Named Facts Before Dispatch

Name complicated conditions before branching:

```omega
let empty: bool = items.len == 0;
transition empty {
    true -> no_items()
    false -> inspect_first()
}
```

The false arm establishes nonempty input. A target using that fact must receive
the corresponding view or value through its state parameters. Naming a fact
does not make the source binding ambient in the target state.

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

## Tail Dispatch

A transition ends the current straight-line segment. Its target is a state in
the same machine, not a machine call disguised by call-shaped arguments.
[Chapter 4](chapter_4_states_transitions.md) explains entry transitions,
state frontiers, and terminal values.

## Domain Patterns

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
