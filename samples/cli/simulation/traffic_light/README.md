# Traffic Light

A traffic-light state machine that cycles through Red → Green → Yellow → Red.
Showcases `case` member types with `[copy]` property, case-value
dispatch in transitions, and accumulated scalar state across multiple machine
calls. Runs to exit **70**.

```
omega --target windows_x64 --build-dir build samples/cli/simulation/traffic_light/main.omg
./build/omega-program.exe   # exit 70
```

The machine starts in `Red` (zero-initialized) and calls `self.advance()` four
times. After Red→Green→Yellow→Red→Green the advance count is 4 and the phase
is `Green`. A two-level guard ladder checks both conditions before exiting 70.

Features exercised:
- `data Phase [copy] { case Red; case Green; case Yellow; }`
- `data Light [copy]` holding a `Phase` field
- `transition self.light.phase { Phase::Green -> ... _ -> ... }`
- Repeated `&mut self` method calls accumulating state
