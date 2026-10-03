# Borrowed scalar-field encoding

After ordinary package review, run from the repository root:

```sh
omega run --both tests/omega/pass/wire/runtime_wire_encode_borrowed_scalar_slice_exit/main.omg
```

The unchanged program must exit 70. Its record holds a three-element view of
four-element backing storage, containing `[-1, 64, -65]`. Encoding must write
exactly `[0, 0, 5, 1, 128, 1, 129, 1]` and report eight bytes: discriminator,
field identity, one packed-byte-length prefix, then zigzag LEB128 values.
An element-count prefix or inclusion of the backing array's fourth element is
incorrect. Package-review refusal is not execution evidence. The reviewed
corpus route is:

```sh
python3 tools/corpus_gate.py --both --filter wire/runtime_wire_encode_borrowed_scalar_slice_exit
```
