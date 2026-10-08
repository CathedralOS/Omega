# Encoded scalar reference at a nonzero offset

```sh
mbx run -p omega -- run --both tests/omega/pass/layouts/runtime_offset_encoded_reference_exit/main.omg
```

A returned scalar loan reads an unsigned 16-bit field as 32768, then a forwarded
call writes 65535 into its original two bytes. The layout places that field at
offset 1 inside a four-byte region borrowed from array element 1. All four
padding and neighboring bytes stay unchanged. This checks that projection
retains the codec and applies each offset once.
