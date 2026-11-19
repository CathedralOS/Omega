# Recursive buffer fill

Ranked tail recursion fills a record's fixed array, a borrowed fixed array,
and a mutable slice. Each call returns the number of filled elements; the
caller checks those counts and the first and last bytes, exiting 0 only when
all observations match. Element writes preserve collection extents.

After ordinary package review:

```sh
cargo run -p omega -- run --both samples/cli/collections/recursive_buffer_fill/main.omg
```

The original failure was rank formation for `cells.bytes.len`, followed by
call-site rejection of fixed-array length bounds after an element write.
Native and Terminal execution both produce exit 0 on macOS ARM64. The
`termination/runtime_projected_extent_fill_exit` regression also exercises
nested fields and mixed ranged/unranged mutual recursion, checking every byte.
Other hosts remain unverified.
