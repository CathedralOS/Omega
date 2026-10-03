# Local reference carrier transfer

The program captures an exclusive loan, stores it in a record, forwards the
record through an ordinary call, and writes through its stored reference. The
original local must observe 70 after the loan ends.

After ordinary local package review:

```sh
omega run --both tests/omega/pass/borrows/local_reference_record_transfer_exit/main.omg
```

Both engines must exit 70. The companion source-custody tests substitute the
supplying holder and access; those substitutions must reject before publication.
