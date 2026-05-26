# Chapter 6: Pattern Matching And Dispatch

Pattern matching inspects values and binds facts.

Omega uses the same pattern vocabulary in both expression position and
transition dispatch. The selected arm contributes facts to the arm body or
target edge.

## Match Expressions

Expression-level `match` chooses a value-producing arm.

```omega
let command: Command = match self.input {
    "quit" -> Command::Quit
    "look" -> Command::Look
    _ -> Command::Invalid
};
```

Working rules:

- `match` evaluates one scrutinee expression.
- Arms are checked top-to-bottom.
- The selected arm contributes its matched facts to the arm body.
- Every reachable arm of a value-producing `match` must produce a compatible
  result type.
- Exhaustiveness is checked unless existing facts prove some arms unreachable.

## Transition Dispatch

Transition dispatch uses the same pattern model, but instead of producing a
value it selects the next control edge.

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

Each selected arm adds its matched pattern as proof facts for that edge.

## Tuple Patterns

Tuple scrutinees make multi-fact dispatch explicit.

```omega
transition (round.player_defeated, round.enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

The wildcard `_` ignores a value but still participates in exhaustiveness.

## Named Facts Before Dispatch

When the facts become hard to read, name them first.

```omega
let found: bool = inventory.items[index].kind == kind;
let has_next: bool = index + 1 < item_count;

transition (found, has_next) {
    (true, _) -> found_item(index)
    (false, true) -> find_item_at(next_index)
    (false, false) -> not_found()
}
```

This keeps proof facts visible to humans and tools.

## Exhaustiveness

Dispatch should be exhaustive unless a prior proof fact makes missing arms
unreachable.

```omega
transition command {
    Command::Look -> look()
    Command::Quit -> finished()
    Command::Invalid -> invalid_command()
}
```

If `Command` later gains a variant, this transition should fail until it handles
the new variant or proves that the new variant cannot occur.

## Tail Dispatch

Transitions are expressed with the `transition` keyword. Tail transitions are
the supported control form inside state bodies.

```omega
state read_command(&mut self) {
    self.console.read_line(&mut self.input);

    transition self.input {
        "" -> finished()
        "quit" -> finished()
        "look" -> look()
        _ -> invalid_command()
    }
}
```

The selected arm ends the current state and transfers control to the target
state. Entry code follows the same rule: the machine body executes straight-line
setup first, then reaches one explicit trailing `transition { ... }` before the
state declarations begin. Chapter 4 defines the machine/state control model;
this chapter focuses on how pattern selection feeds that control model.

## Domain Patterns

Domains may participate in matching when the classifier is proof-visible.
Chapter 8 defines domains themselves; this section only defines how domain
patterns participate in matching once those domains exist.

```omega
match player {
    Player::Alive -> continue_game(player)
    Player::Dead -> game_over(player)
}
```

The selected arm contributes the matched domain fact to the selected arm body or
transition target.
