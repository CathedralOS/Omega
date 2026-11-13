# Nested local sequence extents

After ordinary package review, run:

```sh
omega run --both tests/omega/pass/borrows/local_record_nested_sequence_extent_exit/main.omg
```

Both engines must exit 70. Nested byte-field observations retain their complete
local projection and observe the two-byte live length, not its capacity of eight.
A raw array field keeps its declared length and ordinary indexed scalar read.
