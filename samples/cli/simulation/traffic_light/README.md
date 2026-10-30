# Traffic Light

A traffic-light state machine that cycles through Red → Green → Yellow → Red.
Showcases `case` member types with `[copy]` property, case-value
dispatch in transitions, and accumulated scalar state across multiple machine
calls. Runs to exit **70**.

```sh
omega run --both --input samples/cli/simulation/traffic_light/input.txt samples/cli/simulation/traffic_light/main.omg
```

The machine starts in `Red` (zero-initialized) and calls `Main::advance(main)` four
times. After Red→Green→Yellow→Red→Green the advance count is 4 and the phase
is `Green`. A two-level guard ladder checks both conditions before exiting 70.

Features exercised:
- `data Phase [copy] { case Red; case Green; case Yellow; }`
- `data Light [copy]` holding a `Phase` field
- `transition main.light.phase { Phase::Green -> ... _ -> ... }`
- Repeated `Main::advance(main)` calls mutate the explicit `main: &mut Main` context

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Light.advances`: `0 <= self.advances && self.advances <= 100`.

The original endpoints, guards and operations remain; 4 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
142-byte transcript; native and Terminal execution must both produce it and
exit 70. Other hosted targets retain their entry bindings and need
matching-host execution.

The original `(counter + 1) % 101` expression replaces the redundant inner
remainder. Its pre-store 0..=100 owner invariant proves the exact intermediate
is 1..=101, and the stored remainder is again 0..=100. The invariant is the
premise for forming the addition; the desired result is not its own premise.
