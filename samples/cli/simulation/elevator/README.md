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
omega --target windows_x64 --build-dir build samples/cli/simulation/elevator/main.omg
./build/omega-program.exe   # exit 70
```

Exercises: case-payload dispatch with payload binding, guard-gated
conditional self-writes, `[copy]` aggregate, multi-layer
state transitions within a dispatched machine.
