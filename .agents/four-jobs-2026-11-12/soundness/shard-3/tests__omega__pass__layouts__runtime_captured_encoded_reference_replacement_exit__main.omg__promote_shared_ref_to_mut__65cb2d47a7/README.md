# Captured encoded reference survives container replacement

```sh
mbx run -p omega -- run --both tests/omega/pass/layouts/runtime_captured_encoded_reference_replacement_exit/main.omg
```

A helper moves a container of two reference leaves into a selecting call,
retains its returned encoded-byte loan, and replaces the moved container with
two other references. Writes through the returned loan still reach the original
byte. A second selecting call writes to the replacement's fourth field;
the original ordinary backing, third field, and neighboring byte stay unchanged.

Exit 0 requires the original byte to become 127 and the replacement's fourth
field to become 42, while the neighboring byte remains 85. The paired overflow
control rejects 128 against the original signed-byte backing, not the new
ordinary storage. Replacement ends the old container occurrence after its RHS;
it neither retargets the captured loan nor revives the consumed initializer.
