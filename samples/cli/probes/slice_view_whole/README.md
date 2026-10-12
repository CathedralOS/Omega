# slice_view_whole

Whole-array reborrows lend the array's contiguous storage through ordinary
`&mut [i32]` and `&[i32]` views. The `cells_valid` predicate establishes byte
bounds without changing the i32 carrier or giving a slice a fixed layout.
Writes through the view remain visible through `main.bag.cells[2]`.

The original calculation is `(3 + 5 + 7 + 9) + 7 == 31`. The hosted wrapper
exits zero only when that calculation succeeds. After normal package review:

```sh
omega update --project samples/cli/probes/slice_view_whole --target macos_arm64
omega run --both samples/cli/probes/slice_view_whole/main.omg
```

The build also binds Linux ARM64/x86-64 and Windows x86-64 hosted entries;
the command above demonstrates native and Terminal execution on macOS ARM64.

## Historical scalar positions

The five `[0..=255]` positions at `25b435334dd` retain their obligations:

| Position | Canonical obligation |
| --- | --- |
| `Bag.cells` element | `where cells_valid(&cells) == true` |
| `fill.cells` element | Predicate arrival requirement and checked handback guarantee |
| `fill.value` | Ordinary `requires value >= 0 && value <= 255` |
| `tally.cells` element | Predicate arrival requirement and checked selected-cell citations |
| `Main::main.cells` local element | Borrowed owner predicate and initializer/call facts |

`tally` additionally publishes the derived sum bounds `0..=1020`. The predicate
body proves each cited element bound; `fill` must prove the predicate again
for its actual updated contents, not assume arbitrary predicates survive stores.

## Learnings

- Parenthesize indexed elements and binary expressions used directly as transition results; this example retains its original named `total` and `observed` calculations.
- A `&mut [T]`/`&[T]` view carries unknown length: indexed element access needs `requires at < cells.len` (or `cells.len == 4`) on the callee, even though the `[..]` reborrowed array has statically-known extent.
- A multi-clause `transition` subject with `&&` at top level must sit on continuation lines before `{`.
