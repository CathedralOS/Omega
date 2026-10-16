# Fixed-limb unsigned bignum

Fixed-limb unsigned bignum arithmetic over 8 little-endian `u32` limbs
(`limbs: [u32; 8] in Wrapping`) with compare, ripple-carry add, ripple-borrow
subtract, 4x4-into-8-limb schoolbook multiply, divide/modulo by a nonzero
`u32` limb, and decimal rendering via repeated divmod by 10.

The program runs a scripted sequence on carry-heavy inputs
(`A = 2^65 - 1`, `B = 3 * 2^64 + 3 * 2^32 - 1`) and self-verifies:

- comparison results in both directions and equality,
- `A + B`, `B - A`, and the `(A + B) - B` round-trip against hand-computed limbs,
- the schoolbook product `A * B` against hand-computed limbs,
- `(A * B) divmod 10^9` quotient limbs and remainder `440310785`, and
- the 40-byte decimal rendering of `A * B` against the hand-computed digit string.

On agreement it prints `PASS` and exits `0`.

An intended-invalid control in `invalid/` stores a carry limb at index `8` of
the fixed `[u32; 8]` record and is rejected at compile time
(`cannot prove index `8` is within length 8`).

After ordinary package review on macOS arm64:

```sh
python3 tools/sample_native_gate.py --cli build/dev-target/debug/omega --target macos_arm64 --filter arithmetic/fixed_limb_bignum --jobs 1
build/dev-target/debug/omega run --both samples/cli/arithmetic/fixed_limb_bignum/main.omg < /dev/null
```
