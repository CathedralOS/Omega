# Chapter 6: Pattern Matching And Dispatch

Pattern matching inspects values and binds facts.

Transitions use matching to choose control edges. Ordinary expression-level
matching may exist later, but the same fact rules should apply.

## Transition Dispatch

Conditional transitions name the value being inspected.

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

## Guarded Jumps

A guarded jump is still a transition.

```omega
state read_command(&mut self) {
    self.console.read_line(&mut self.input);

    -> finished() when self.input == "";

    transition self.input {
        "quit" -> finished()
        "look" -> look()
        _ -> invalid_command()
    }
}
```

If the guard is true, the current path ends and the target state starts.

## Domain Patterns

Domains may participate in matching when the classifier is proof-visible.

```omega
transition player {
    Player::Alive -> continue_game()
    Player::Dead -> game_over()
}
```

The selected arm contributes the matched domain fact to the target edge.
