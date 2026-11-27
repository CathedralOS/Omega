# Record range search

One binary-search operation computes either bound of a key in a sorted record
slice. It uses the ordinary overflow-safe midpoint, `lower + (upper - lower) / 2`,
and keeps its search interval inside the borrowed slice. Sortedness is an
algorithm precondition on the supplied records, not a claimed static proof.

The authored exit-0 oracle covers duplicate keys, a missing key, an empty view,
a shifted subview, zero and the maximum unsigned 64-bit key.

After ordinary package preparation:

```sh
mbx run -p omega -- run samples/cli/algorithms/record_range_search/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/algorithms/record_range_search` from the repository root.

The midpoint uses the same live integer relationships for its Exact addition,
its saved local value and its slice index. Native and Terminal execution both
return 0 on macOS arm64. The record keys deliberately include the full `u64`
maximum; the search endpoints come from the supplied slice, not a fixed clamp.
