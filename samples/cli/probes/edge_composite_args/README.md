# edge composite args

Compile-scope probe. No standard-library dependency.

- Field paths, record literals, machine-call results, affine call
  results and `&` views all ride a transition edge; only the
  selected edge evaluates its call operand (`main.made` stays 0 —
  the losing arm's `make` never ran).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/edge_composite_args/main.omg
mbx run -p omega -- update --project samples/cli/probes/edge_composite_args
```
