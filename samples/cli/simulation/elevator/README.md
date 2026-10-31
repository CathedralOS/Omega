# Elevator Controller

A multi-state elevator FSM driven by a stream of case-payload commands.
Stresses: `case` members with named payloads (`GoTo(floor: i32)`),
payload binding in transition arms, guard-gated state writes (door-closed
check before moving), and scalar self-write accumulation.

Commands: `GoTo(floor)`, `OpenDoor`, `CloseDoor`. The elevator only moves
when the door is closed.

Scripted sequence (6 commands, 3 valid moves):

```
GoTo(2)    -> floor=2, moves=1
OpenDoor   -> door_open=1
CloseDoor  -> door_open=0
GoTo(0)    -> floor=0, moves=2
GoTo(1)    -> floor=1, moves=3
OpenDoor   -> door_open=1
```

Final state: `floor=1`, `moves=3`, `door_open=1`. Exits **70**.

```
omega --target windows_x86_64 --build-dir build samples/cli/simulation/elevator/main.omg
./build/omega-program.exe   # exit 70
```

Exercises: case-payload dispatch with payload binding, guard-gated
conditional self-writes, `[copy]` aggregate, multi-layer
state transitions within a dispatched machine.

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Elevator.moves`: `0 <= self.moves && self.moves <= 100`.

The original endpoints, guards and operations remain; 6 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
168-byte transcript; native and Terminal execution must both produce it and
exit 70. Other hosted targets retain their entry bindings and need
matching-host execution.

The original `(counter + 1) % 101` expression replaces the redundant inner
remainder. Its pre-store 0..=100 owner invariant proves the exact intermediate
is 1..=101, and the stored remainder is again 0..=100. The invariant is the
premise for forming the addition; the desired result is not its own premise.
