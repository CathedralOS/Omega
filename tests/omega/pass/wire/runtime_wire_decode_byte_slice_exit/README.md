# Borrowed byte-slice decoding

After ordinary package review, run from the repository root:

```sh
omega run --both tests/omega/pass/wire/runtime_wire_decode_byte_slice_exit/main.omg
```

Native and interpreted execution must exit 70. The program encodes `[72, 105]`,
decodes a borrowed view into that buffer, and re-encodes the replaced record
field. It checks the Sound verdict, five consumed bytes, five produced bytes,
and the two payload bytes. The record initializer's original loan and the
decoder call's replacement loan belong to distinct statement occurrences.

The reviewed corpus route is:

```sh
python3 tools/corpus_gate.py --both --filter wire/runtime_wire_decode_byte_slice_exit
```
