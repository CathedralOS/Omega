# Compound index guard

The live fact `0 <= b * 20 + a < 200` permits storing `7` at
`grid[b * 20 + a]`.

The root and nine place projects bind `ProgramEntry` and declare an `Expected exit: 0`
runtime oracle. Each place verifies that the guarded store executes and that the write
lands (`grid[75] == 7`), exiting 0 natively and under the Terminal interpreter.

Each place repeats the same fact sentence in its source. The index remains a
two-variable `i32 in Wrapping` computation, the grid has 200 elements, and the
operation remains an indexed store. With `a = 15` and `b = 3`, `b * 20 + a == 75`,
which satisfies `0 <= 75 < 200`.

| Place | Evidence carried to the store |
| --- | --- |
| `local` | Copied coordinates and their compound guard cross the transition. |
| `field` | The guard bounds the same field computation at the store. |
| `parameter` | Explicit coordinate parameters carry the compound requirement. |
| `call_result` | A checked coordinate result equals the guarded coordinate. |
| `match_join` | Both arms preserve the guarded coordinate. |
| `const_name` | Named stride 20 and ceiling 200 preserve the same computation. |
| `other_state` | The guarded field computation survives arrival in `put`. |
| `other_machine` | A separate store machine requires the same compound bound. |
| `target_body` | Every hosted target body checks the same guard and store. |

Each `invalid/` twin removes only the strict upper-bound proposition, including
its corresponding parameter requirement where present. The lower bound stays;
these controls must reject for the store's missing index upper bound.

```sh
omega run --both samples/cli/facts/compound_index_guard/<place>/main.omg
omega --check samples/cli/facts/compound_index_guard/<place>/invalid/main.omg
```

The baseline call-result place rejects with `cannot prove index
main.b * 20 + a is within length 200 in Main::main::put` (diagnostic quoting
omitted here). The checked coordinate equality must survive in the compound
wrapping observation.

Execution acceptance covers root and all nine places across both native and
Terminal engines, and all nine reject twins reject under `--check`.

```sh
python3 tools/sample_native_gate.py --cli <omega> --facts --filter compound_index_guard
```

The sample gate reports ten passing programs (root plus nine places), nine
intended-invalid rejections, and zero unscored.
