On macOS AArch64:

```sh
omega run --both main.omg < input.txt
```

Both engines return 0 and print `abc` followed by LF, then `!` and LF. A whole
record borrowed through a helper contains nested fixed-array storage and a
two-byte tail. A short read crosses the one-byte header and three array elements,
preserving the tail and adjacent root fields; EOF changes nothing. Whole-record
output observes those same original fields. No external package is required.

Native placement keeps fixed-array dimensions as compact stride recipes, not
one compiler row per element. Terminal captures only actual scalar leaves.
