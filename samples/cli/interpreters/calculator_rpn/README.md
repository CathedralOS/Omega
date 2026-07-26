# calculator_rpn

Integer RPN (reverse-Polish notation) calculator over a fixed-capacity `[i32; 4]` stack.

## What it demonstrates

- `[i32; 4]` fixed-array stack with `[copy]` property
- Const-dispatch indexed write via `write_slot(idx, value)` chain (proven pattern)
- `sp` (stack pointer) as an integer field driving dispatch
- Binary arithmetic in a leaf state with const-indexed reads and no intermediate
  sub-machine calls (avoids the stale-static-fold risk)
- Sequential sub-machine calls (`push`, `add`, `mul`, `sub`) that each mutate `self`
- Guard-ladder verification of the final computed result

## RPN expression evaluated

`32 4 + 2 * 2 -` = ((32 + 4) × 2) - 2 = 70

| Step     | Stack contents | sp |
|----------|----------------|----|
| push 32  | [32]           | 1  |
| push 4   | [32, 4]        | 2  |
| add      | [36]           | 1  |
| push 2   | [36, 2]        | 2  |
| mul      | [72]           | 1  |
| push 2   | [72, 2]        | 2  |
| sub      | [70]           | 1  |

## Design note: binary-op implementation

Binary ops (`add`, `sub`, `mul`) dispatch on `sp` and read both operands from
const-indexed slots within a single leaf state. This avoids calling sub-machines
between the reads and the write, which is the defensive pattern against the
stale-static-fold miscompile shape (field reads inside a sub-machine body can
return the initial value if a prior sub-machine call wrote the field but the
static tracking was not refreshed).

## Expected exit code

**70** — `result == 70` and `sp == 0` both verify correct.

## Building

```
cargo run -p omega-cli -- --build-dir samples/calculator_rpn/build --target windows_x64 samples/calculator_rpn/main.omg
./samples/calculator_rpn/build/omega-program.exe
echo $?   # 70
```
