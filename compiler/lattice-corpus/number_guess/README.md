# number_guess

Scripted binary-search guessing game (no stdin — target is hardcoded).

## What it demonstrates

- Integer division (`/`) in a `let`-binding: `mid = (lo + hi) / 2`
- `[copy]` data type holding the search state (`lo`, `hi`, `mid`, `guesses`)
- Sequential calls to the same sub-machine (`step`) that each mutate `lo` or `hi`
- Reading fields updated by the PREVIOUS sub-machine call at the top of the CURRENT call
- Three-arm guard dispatch inside `step`: `== target`, `< target`, `_ (> target)`
- Guard ladder in `main` verifying `found == 1` and `guesses == 7`

## Binary search trace

Secret number: **42**, range: **[1, 100]**

| Guess | mid | Result     | lo | hi  |
|-------|-----|------------|----|-----|
| 1     | 50  | > 42, hi-- | 1  | 49  |
| 2     | 25  | < 42, lo++ | 26 | 49  |
| 3     | 37  | < 42, lo++ | 38 | 49  |
| 4     | 43  | > 42, hi-- | 38 | 42  |
| 5     | 40  | < 42, lo++ | 41 | 42  |
| 6     | 41  | < 42, lo++ | 42 | 42  |
| 7     | 42  | == 42, ✓   | —  | —   |

7 guesses to find 42.

## Expected exit code

**70** — `found == 1` (target found) and `guesses == 7` both verify correct.

## Building

```
cargo run -p omega-cli -- --build-dir samples/number_guess/build --target windows_x64 samples/number_guess/main.omg
./samples/number_guess/build/omega-program.exe
echo $?   # 70
```
