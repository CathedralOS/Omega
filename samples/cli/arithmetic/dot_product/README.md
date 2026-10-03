# Dot product

```sh
omega run --both samples/cli/arithmetic/dot_product/main.omg
```

The complete program computes `[1,2,3] · [4,5,6] = 32`, prints its three
messages and exits 32 after Enter or EOF. Both arrays remain local caller-owned
storage borrowed across the reduction's state cycle.

On a fresh checkout, the public command first needs the package review for
the selected host. Inspect and decide the generated review before resuming
`omega update`; tests do not authorize sample package decisions.

The byte-identical body is pinned by:

```sh
python3 tools/corpus_gate.py --both --filter collections/runtime_shared_array_dot_product_exit
```

At `e56af86474b` on macOS ARM64, the repository fixture builds and both engines
exit 32 with exact stdout. This is compiler execution evidence using fixture
admission, not evidence that the public sample's package review was accepted.
The public command on that revision exited 200 with pending package-review
decisions; no decisions were accepted for this measurement.
