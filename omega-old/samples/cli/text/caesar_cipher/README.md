# Caesar cipher

Reads each byte of `HELLO`, shifts it by three with wrapping `u8` arithmetic,
stores the result in a UTF-8 buffer, and prints `KHOOR`.

After completing the sample's package review, run from the repository root:

```sh
omega run --both samples/cli/text/caesar_cipher/main.omg < /dev/null
```

In PowerShell, run the same command without the redirection and press Enter at
the final prompt. The named entry and its state arguments run natively and in
the checked interpreter on macOS ARM64, with both engines exiting 0. Expected
application stdout is exactly:

```text
caesar_cipher: shift each byte of HELLO by 3 -- expecting KHOOR
KHOOR
PASS: ciphertext starts with 'K' -- shift applied correctly (exit 0)
[press Enter to close]
```

The introductory banner also contains `KHOOR`; correct execution requires the
standalone ciphertext line as well. The CLI reports `native exit 0` and
`interp exit: 0` separately on stderr.

The compiler must reconstruct the byte-read range from the unchanged literal
contents (`69..79`) before proving that the shifted result preserves the output
buffer's invariant. The index bound remains a separate obligation. Intervening
overlapping writes or mutable calls invalidate the literal-content premise;
source checking alone is not portable evidence for the byte-store proof.
