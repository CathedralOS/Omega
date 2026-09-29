# Format a number

From the repository root, after the checkout's ordinary package review:

```sh
mbx run -p omega -- run samples/cli/text/format_number/main.omg
```

The program computes the decimal digits of 25, narrows them through the library's
guarded trapping conversion, and stores them in its UTF-8-qualified buffer. It
compares the resulting buffer with `"25"`, prints
`PASS: buffer matches '25' (exit 70)`, and exits 70 after Enter or end of input.
Exit 71 reports a mismatch. Do not pass `--target` when requesting execution;
that selects the CLI's compile-only route.

Verified natively on macOS ARM64. Other hosts still need matching-host runs.
The compiler retains an independently proved conversion-result guarantee across
the helper's continuation blocks and follows live field-store equations back to
the digit computations. Neither the trapping guards nor the UTF-8 requirement
are bypassed.
