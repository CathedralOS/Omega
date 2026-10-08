# Captured encoded reference survives container replacement

```sh
mbx run -p omega -- run --both tests/omega/pass/layouts/runtime_captured_encoded_reference_replacement_exit/main.omg
```

A helper moves a container of two reference leaves into a selecting call,
retains its returned encoded-byte loan, and replaces the moved container with
two other references. Writes through the returned loan still reach the original
byte. A second selecting call writes to the replacement's fourth field;
the original ordinary backing, third field, and neighboring byte stay unchanged.

This is unfinished execution acceptance, not a passing example. Source fit
retains the original signed-byte backing (the paired overflow control rejects),
but lowering stops at `local data: structural value: reference record loans`.
The initializer loan weakens at `LocalReassigned`; that after-RHS boundary is
not an ordinary before-statement release. The owning task is
`ENCODED-STORAGE-REFERENCE-TRANSPORT`.
