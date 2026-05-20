# Chapter 5: Return Value Compatibility

Return value compatibility is a graph check for a typed machine.

If a machine declares `-> T`, every reachable terminal completion in that
machine's internal graph must produce a compatible `T`.

```omega
machine Parser::resolve(
    &mut self,
    line: &ConsoleLine
) -> Command {
    transition line.text {
        "quit" -> Command::Quit
        "look" -> Command::Look
        _ -> Command::Invalid
    }
}
```

`Parser::resolve` promises `Command`, so each dispatch arm must produce a
`Command`.

## Graph Compatibility

Internal states participate in the machine's return-value graph.

```omega
machine CombatSystem::fight(
    &mut self,
    player: &mut Player,
    enemy: &mut Enemy
) -> CombatRound {
    player.health = player.health - enemy.attack;

    transition player.health == 0 {
        true -> defeated(enemy)
        false -> survived(enemy)
    }

    state defeated(enemy: &mut Enemy) {
        CombatRound {
            player_defeated: true,
            enemy_defeated: false
        }
    }

    state survived(enemy: &mut Enemy) {
        CombatRound {
            player_defeated: false,
            enemy_defeated: enemy.health == 0
        }
    }
}
```

`defeated` and `survived` are not called from outside. They are transition
targets inside `CombatSystem::fight`, and their terminal values must satisfy
the machine's `CombatRound` result.

The compatibility checks are:

- The transition target state exists in the current machine graph.
- State-transition arguments match the target state's parameters.
- Terminal values satisfy the active machine's return type.
- Every reachable terminal path in a typed machine graph produces the declared
  return type.
- Transition dispatch arms add proof assumptions for the target edge, but they
  do not create caller frames.

## Terminal Value Syntax

A final expression can complete the machine invocation.

```omega
state shutdown(&mut self) {
    0
}
```

The state name is a jump target. The `0` is a terminal value.

When a branch should move control, use a transition target with parentheses:

```omega
transition {
    _ -> shutdown()
}
```

The parentheses matter because they distinguish a state jump from a bare value.

## Machines Are Call Boundaries

Transitions to machines are rejected in the current model.

```omega
machine Helper::value(&mut self) -> i32 {
    1
}

machine main(&mut self) -> i32 {
    transition {
        _ -> value() // illegal unless value is a state in this machine
    }
}
```

Normal call syntax is the way to enter another machine:

```omega
let value: i32 = self.helper.value();
```

If the language later needs tail calls into machines, they should get an
explicit spelling. They should not masquerade as ordinary state transitions.
