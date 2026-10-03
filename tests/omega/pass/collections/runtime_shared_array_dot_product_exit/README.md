# Two shared arrays through a cyclic reduction

`main.omg` is byte-identical to
`samples/cli/arithmetic/dot_product/main.omg`. Keep the full multiply-accumulate
loop, both caller-owned arrays, console output and EOF/Enter interaction.

```sh
python3 tools/corpus_gate.py --both --filter collections/runtime_shared_array_dot_product_exit
```

Native and checked-interpreter execution must both exit 32 and match all three
stdout lines. Neither array is copied into scratch fields. This uses repository
fixture admission; the sample's public command and separate package review are
documented in its README. The program already ran on base `e56af86474b`; this
fixture pins that support rather than claiming a new compiler repair.
