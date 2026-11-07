# Generic element amplitude

A smaller witness of the const-generic runtime index walk that
`antenna_array_pattern` hardcodes at four elements. The amplitude is the sum
of absolute weights. One machine reads a record field. Another reads a parameter array and
writes the comparison as `Count > index`. The element read is in the next
state, and the following index is the same parameter plus one.

`omega run main.omg` on Linux x86-64 exits 0 and prints
`generic element amplitude 6 4`.

`invalid/` writes index 4 of a two-element const-generic weight array. Its
rejection must identify `cannot prove index `4` is within length`.
