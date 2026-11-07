# Rotor byte rows

The bombe stores each rotor as its own `[i32; 26]` field and re-bounds every
table read with a transition before that read may shift a live-wire mask.
This program keeps rotors I, II, and III in one `[[u8; 26]; 3]` field.
`wiring_at` returns the selected byte under `ensures result < 26`. The caller
widens that byte and shifts `1 << image`, and also uses the same byte as a
column into another rotor row.

The lookup is observed three ways: the call result shifted directly, the same
call stored in a field and then shifted, and the call result used as the
column index. Rotor I at A is 4, so the mask is 16. Rotor III at column 4 is
9. The program prints `16` and exits 0 when both hold.

`invalid/` indexes column 26 of a 26-byte row and must be rejected
(`cannot prove index `26` is within length 26`).

```sh
omega run samples/cli/simulation/rotor_byte_rows/main.omg
```
