# Recursive buffer fill

Ranked tail recursion fills a record's fixed array, a borrowed fixed array,
and a mutable slice. Each call returns the number of filled elements; the
caller checks those counts and the first and last bytes, exiting 0 only when
all observations match. The field loop uses `Nat::IncreasingTo(cells.bytes.length)`
and must prove that its element writes preserve the bound. The slice
loop bounds its remaining distance by `0..=bytes.length` on each invocation.

From the repository root with a built `omega` on `PATH`:

```sh
omega run --both samples/cli/probes/recursive_buffer_fill/build.omg
```

The original failure was rank formation for `cells.bytes.length`, followed by
call-site rejection of fixed-array length bounds after an element write.
The original probe used `Nat::BoundedDistance` for all three fills and exited 0
in native and Psi execution on Linux x86-64 and macOS ARM64. The
`termination/runtime_projected_extent_fill_exit` regression also exercises
nested fields and mixed ranged/unranged mutual recursion, checking every byte.
