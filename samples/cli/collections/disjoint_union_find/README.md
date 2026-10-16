# Disjoint-set union-find

Disjoint-set union over 64 elements in fixed arrays (`parent: [u64 [0..=63]; 64]`
and `rank: [u64 in Wrapping; 64]`) with union-by-rank and iterative two-pass
path compression.

The program initializes 64 singleton sets, executes four waves of unions (pairs,
quads, octets, and cross-group unequal-rank and redundant unions), then queries
representatives across all 64 elements and self-verifies:

- representative agreement across merged components and separation across distinct components,
- exact component count (`8`), and
- representative table checksum (`12229357013220123104`).

On agreement it prints `PASS` and exits `0`.

An intended-invalid control in `controls/find_out_of_range` invokes
`DisjointSet::find` with an out-of-range element (`64`, outside `0..=63`) and is
rejected at compile time.

After ordinary package review on macOS arm64:

```sh
python3 tools/sample_native_gate.py --cli build/dev-target/debug/omega --target macos_arm64 --filter collections/disjoint_union_find --jobs 1
build/dev-target/debug/omega run --both samples/cli/collections/disjoint_union_find/main.omg < /dev/null
```
