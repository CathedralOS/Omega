# Stopwatch

A stopwatch driven by `Tick`, `Lap`, and `Split` case-payload commands.
Stresses: case members with scalar payloads (`Tick(n: i32)`), payload
binding in transition arms, multiple scalar field writes within dispatch
states, and `[copy]` aggregate property.

Scripted sequence (7 commands):

```
Tick(10)  elapsed=10, total=10
Split     last_split=10
Tick(20)  elapsed=30, total=30
Lap       last_lap=30, elapsed=0
Tick(15)  elapsed=15, total=45
Tick(25)  elapsed=40, total=70
Split     last_split=40
```

Final state: `total=70`, `last_split=40`, `last_lap=30`, `elapsed=40`.
Exits **70** when all four values are correct.

```
omega --target windows_x64 --build-dir build samples/stopwatch/main.omg
./build/omega-program.exe   # exit 70
```

Exercises: case-payload dispatch (`Tick`/`Lap`/`Split`), multiple scalar
self-writes inside dispatched states, elapsed reset on lap.
