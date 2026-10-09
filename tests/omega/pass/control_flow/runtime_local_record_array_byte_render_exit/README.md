# Local record arrays and qualified byte rendering across states

`main.omg` is byte-identical to `samples/cli/algorithms/maze_flood/main.omg`.
It exercises an invocation-local record containing fixed arrays, a
scalar-returning helper that mutates that record through explicit state
parameters, and rendering through a qualified mutable bounded byte-owner loan.
The helper must update the caller's actual storage; later shared state loans
must observe those writes. Reentering the rendering state creates a fresh line.

`expected_stdout.txt` independently spells all five shortest-distance rows,
including two trailing spaces per row, the banner and final messages: 235 bytes.
Native and Terminal execution must each produce those exact bytes, empty stderr
and exit 0. EOF completes the closing input without an interactive dependency.
An exit-only comparison would miss incorrect rendering or missing output.

From the repository root:

```text
python3 tools/corpus_gate.py --both --filter control_flow/runtime_local_record_array_byte_render_exit
omega run --both samples/cli/algorithms/maze_flood/main.omg
```

Run the second command with finite input ending at EOF. Build declarations list
all supported hosts; each host still requires its own execution qualification.
