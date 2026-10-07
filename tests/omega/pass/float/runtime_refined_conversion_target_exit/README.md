# Refined conversion targets

Run from the repository root:

```sh
mbx run -p omega -- run tests/omega/pass/float/runtime_refined_conversion_target_exit/main.omg
```

Use Cargo if mbx is unavailable. Native execution exits 0. Both conversions
must preserve the helper's value and prove their authored target ranges;
selecting a conversion requirement must not discard those predicates.
