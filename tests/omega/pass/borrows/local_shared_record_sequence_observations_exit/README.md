# Observe original storage through a shared record reference

After ordinary package review:

```sh
omega run --both tests/omega/pass/borrows/local_shared_record_sequence_observations_exit/main.omg
```

Both engines exit 70. The shared reference observes a nested UTF-8 field's
live two-byte length and contents, not its eight-byte capacity or the pointer
slot's bytes. Raw-array length and element selection retain their own geometry.
