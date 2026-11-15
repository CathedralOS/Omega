# Bounded byte copies

The [probe](main.omg) copies the five bytes of `"Room "` into a 16-byte array at
offset 0. It then copies the label's two live bytes, `"A1"`, from its four-byte
array into that same destination at offset 5. The destination stores its used
length separately. The entry checks that the live view equals `"Room A1"` and
has length 7, then traps if either check fails.

The [Build file](build.omg) selects an entry for all four hosted targets.

The source still calls undeclared `ascii_only`. It needs an ordinary checked
ASCII predicate, explicit qualification under the current
[domain rules](../../../../wiki/spec/language/domains.md#byte-containers-and-encoding-domains),
and checker support for its fixed-array and qualified copy/view contracts.
[LOWERED-FORM-CHECKER](../../../../TASKS.md#priority-human-set-only) tracks that
remaining work. The source and its oracle remain in place; no successful check
or native/Psi execution is claimed.

Reproduce the outstanding check and execution requirements with:

```sh
mbx run -p omega -- --check samples/cli/probes/bounded_concat/main.omg < /dev/null
mbx run -p omega -- run --both samples/cli/probes/bounded_concat/main.omg < /dev/null
```
