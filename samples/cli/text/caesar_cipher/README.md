# Caesar cipher

Reads each byte of `HELLO`, shifts it by three with wrapping `u8` arithmetic,
stores the result in a UTF-8 buffer, and prints `KHOOR`.

After completing the sample's package review, run from the repository root:

```sh
omega run samples/cli/text/caesar_cipher/main.omg < /dev/null
```

In PowerShell, run the same command without the redirection and press Enter at
the final prompt. Successful execution prints a **standalone line** `KHOOR`,
prints the `PASS` message, and exits 0. The introductory banner also contains
`KHOOR`; matching that substring alone does not establish correct execution.

The compiler must reconstruct the byte-read range from the unchanged literal
contents (`69..79`) before proving that the shifted result preserves the output
buffer's invariant. The index bound remains a separate obligation. Intervening
overlapping writes or mutable calls invalidate the literal-content premise;
source checking alone is not portable evidence for the byte-store proof.
