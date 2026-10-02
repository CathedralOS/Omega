# Borrowed byte-field encoding

After ordinary package review, run from the repository root:

```sh
omega run --both tests/omega/pass/wire/runtime_wire_encode_byte_slice_exit/main.omg
```

The unchanged program must exit 70. It constructs a record containing a shared
view of `[72, 105]`, encodes it into a separate field of the same owner, and checks
the exact five bytes `[0, 0, 2, 72, 105]` and written count. Copying the descriptor
does not write its backing or end the stored loan. Package-review refusal is not
execution evidence; the reviewed corpus route is:

```sh
python3 tools/corpus_gate.py --both --filter wire/runtime_wire_encode_byte_slice_exit
```
