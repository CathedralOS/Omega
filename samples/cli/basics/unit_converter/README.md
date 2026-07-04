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

All checks pass via guard ladders and the program exits 70.

## What it tests

- f64 arithmetic in free machine bodies (parameter arithmetic)
- f64 field read/write in guard comparisons
- Chained machine calls storing results in a field
- Multi-step f64 computation (`shifted * 0.555...` for F→C)
- Guard ladder pattern for float value verification
