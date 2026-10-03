# Interior scalar-slice encoding

After ordinary package review, run from the repository root:

```sh
omega run --both tests/omega/pass/wire/runtime_wire_encode_interior_scalar_slice_exit/main.omg
```

The unchanged acceptance is native and interpreted exit 70. The record borrows
`source[1..4]` from `[99, -1, 64, -65]`; encoding must skip the sentinel and write
exactly `[0, 0, 5, 1, 128, 1, 129, 1]`, with written count eight. Both the retained
start and the three-element extent must survive record capture and codec calls.

The reviewed corpus route is:

```sh
python3 tools/corpus_gate.py --both --filter wire/runtime_wire_encode_interior_scalar_slice_exit
```
