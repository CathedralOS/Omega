# Masked byte scan

Parse adjacent hexadecimal byte pairs with `?` wildcard nibbles into ordinary
value/mask records. Scan every candidate start, including overlapping matches,
without treating an incomplete tail as a match. Output capacity bounds writes.
The parser accepts upper/lowercase hex and rejects invalid digits, odd lengths,
and insufficient output space; it may leave a successfully decoded prefix on
failure. Empty input parses to zero bytes.

`omega run --both samples/cli/text/masked_byte_scan/main.omg` must exit 0 on
both engines. The program checks parsed values/masks, match positions, unchanged
output sentinels, capacity truncation, empty input and parser failures. This is a
language pressure program for signature scanning, not a Squalr port substitute.
