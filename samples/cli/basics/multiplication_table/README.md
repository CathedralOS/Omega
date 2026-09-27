# Multiplication table

From the repository root, run:

```text
mbx run -p omega -- run samples/cli/basics/multiplication_table/main.omg
```

Press Enter at the closing prompt. The native program must print nine rows of
nine products: row `i`, column `j` contains `i * j` for `1 <= i,j <= 9`. The
last row is `9 18 27 36 45 54 63 72 81`; output uses space-padded columns. The
program must exit with `0`.

For a fresh checkout, prepare and review the project with the ordinary
[package commands](../../../../omega-rust/omega/omega/README.md#command-surface).
