# Smallest prime factor

Trial division finds 13 as the smallest prime factor of 169. The named entry
passes its context through the states; the compiler proves the square,
remainder and increment safe without wrapping arithmetic.

From the repository root:

```sh
mbx run -p omega -- run --both samples/cli/arithmetic/smallest_prime_factor/main.omg
```

Use Cargo if `mbx` is unavailable. Complete any requested local package review
with `omega update --target macos_arm64 --project samples/cli/arithmetic/smallest_prime_factor`,
edit the generated review decisions, then resume with `omega update --resume
--project samples/cli/arithmetic/smallest_prime_factor`. Select your host target
for update; leave `--target` off `run`.

Both engines exit **13** on macOS ARM64. The native stdout is exactly:

```text
smallest_prime_factor: trial division finds the smallest factor of 169
PASS: smallest prime factor found, 13 for n=169 (the exit code)
[press Enter to close]
```

Press Enter to finish, or provide a newline on stdin for automation.
