# Bounded byte copies

The [probe](main.omg) copies the five bytes of `"Room "` into a 16-byte array at
offset 0. It then copies the label's two live bytes, `"A1"`, from its four-byte
array into that same destination at offset 5. The destination stores its used
length separately. The entry checks that the live view equals `"Room A1"` and
has length 7, then traps if either check fails.

The [Build file](build.omg) selects an entry for all four hosted targets.

The probe uses Core's `ascii_only` predicate. It removes the displayed domains
before slicing and indexing under the
[domain rules](../../../../wiki/spec/language/domains.md#byte-containers-and-encoding-domains).
The shared prefixes keep the ASCII proof from their backing arrays. Each copy
proves that its writes preserve the destination's ASCII requirement.

Run both engines with:

```sh
mbx run -p omega -- run --both samples/cli/probes/bounded_concat/main.omg < /dev/null
```
