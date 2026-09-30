# unit_converter

Unit conversion library covering temperature, distance, and mass.

Conversion checks verified at runtime:

| Input | Output | Expected |
|-------|--------|----------|
| 100°C | Fahrenheit | 212.0 |
| 212°F | Celsius | 100.0 |
| 373.15 K | Celsius | 100.0 |
| 10 km | miles | ~6.21 |
| 5 kg | lbs | ~11.02 |

The guard ladders require every conversion to fall within its authored tolerance;
success exits 70 and a failed check exits 71–80.

Run the complete program after reviewing its package inputs:

```sh
omega run samples/cli/basics/unit_converter/main.omg
```

Native compilation currently stops at nested boundary-call composition in
`celsius_to_fahrenheit`. Ordinary float arithmetic also needs native builtin
installation; a successful checked comparison alone does not close this sample.

## What it tests

- f64 arithmetic in free machine bodies (parameter arithmetic)
- Static entry and explicit context arguments between states
- Conversion results held in locals and passed only to the next tolerance check
- Local input buffers allocated only by the terminal states that use them
- Multi-step f64 computation (`shifted * 0.555...` for F→C)
- Guard ladder pattern for float value verification
