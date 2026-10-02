# Prime sieve

From the repository root, run:

```text
mbx run -p omega -- run --both samples/cli/arithmetic/prime_sieve/main.omg
```

Press Enter at the closing prompt. Native and checked-interpreter execution
both exit `0` on macOS ARM64. The native program must
print this prime sequence, in order:

```text
2 3 5 7 11 13 17 19 23 29 31 37 41 43 47
```

The entry receives an explicit context parameter and forwards it through the
states. Output uses 15 space-padded three-byte columns and three trailing spaces
in a 48-byte row. The full native stdout, including banner, PASS line and closing
prompt, is 175 bytes with LF endings.

For a fresh checkout, prepare and review the
project with the ordinary [package commands](../../../../omega-rust/omega/application/cli/README.md#command-surface).
