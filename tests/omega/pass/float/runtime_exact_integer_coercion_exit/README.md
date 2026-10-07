# Exact integer coercion

Run from the repository root:

```sh
mbx run -p omega -- run tests/omega/pass/float/runtime_exact_integer_coercion_exit/main.omg
```

Use Cargo if mbx is unavailable. Native execution exits 0. Optional `--both`
compares the produced Terminal artifact, not checked trees; that comparison
currently declines because the exact `I32::from_f64` host adapter is missing.
The helper result's source range must reach conversion checking, and the
converted integer bound must reach its caller. A live IEEE equality proves
integrality; a binary64 interval above unit spacing contains only integers.
The named conversion deliberately truncates 3.75 to 3: its weaker guarantee
must not replace the authored Exact coercion obligation.
