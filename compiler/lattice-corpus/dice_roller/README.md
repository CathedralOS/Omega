# dice_roller

Deterministic dice roller using a linear congruential generator (LCG).

## What it demonstrates

- Integer arithmetic chains (`*`, `+`, `%`) on `let`-bound locals
- Sequential LCG steps computed inline to avoid field-read-after-sub-call patterns
- `[copy]` data properties on a scalar aggregate
- Guard-ladder verification of a precomputed integer result
- Exit-code encoding of a correct computation

## How it works

LCG parameters: multiplier=1103, increment=12345, modulus=65536. All
intermediate products fit comfortably in `i32` (max: 65535 × 1103 ≈ 72M < 2³¹).

Starting from seed 42, five d6 dice rolls are computed inline:

| Step | State  | d6 |
|------|--------|----|
| 1    | 58671  | 4  |
| 2    | 42426  | 1  |
| 3    | 15519  | 4  |
| 4    | 24906  | 1  |
| 5    | 24079  | 2  |

Sum = 12, final state = 24079. Both are checked; exit 70 on match.

## Authoring note

All five LCG steps are unrolled inline in `main` using chained `let`-bindings
rather than a looping sub-machine. Sub-machine calls that read back a field
written by the previous call can encounter a stale-static-fold shape where the
caller's local static values are not updated after the sub-machine write; the
inline form sidesteps this entirely.

## Expected exit code

**70** — both `rng_state == 24079` and `total == 12` verify correct.

## Building

```
cargo run -p omega-cli -- --build-dir samples/dice_roller/build --target windows_x64 samples/dice_roller/main.omg
./samples/dice_roller/build/omega-program.exe
echo $?   # 70
```
