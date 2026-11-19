# Dutch flag

Dijkstra's three-pointer partition turns `[White, Blue, Red]` into
`[Red, White, Blue]`, checks all three results, and exits 70.

This customer of `CANONICALIZE-SCALAR-RANGE-CONTRACTS` preserves four field
intervals from `7a9a7b8287e6`: `r`, `w` and `b` stay at most three, while
`bp` stays at most two. Their `u64` carriers supply the zero floors. The data
predicate establishes these storage restrictions independently of the existing
index guards. In particular, `b <= 3` and `b > 0` justify storing `b - 1`
into `bp` before its later guard. The algorithm and guards remain unchanged;
28 terminal calls acknowledge the existing blocking-input path.

After the ordinary package review, run from the repository root:

```sh
omega run --both --input samples/cli/algorithms/dutch_flag/input.txt samples/cli/algorithms/dutch_flag/main.omg
```

The input supplies one newline and then EOF. `expected_stdout.txt` pins the
complete 141-byte transcript, including the success message and final prompt.
Native and Terminal execution must both produce that transcript and exit 70.
The expected output was independently derived before execution. Other hosted
targets retain their entry bindings and need matching-host validation.
