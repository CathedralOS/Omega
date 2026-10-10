# Case-qualified reference storage

`omega run --both tests/omega/pass/generics/runtime_case_qualified_reference_storage_exit/main.omg`
must exit 0 through native and Terminal execution.

The program reads an ordinary `End` and follows an active `More` to a growing
generic application's array payload. A descriptor retains its finite referent
identity without recursively storing its pointee. The mixed `Selection` variant
adds a common field and identically named descriptor fields in distinct cases,
then transports both cases with distinct referents beneath a fixed array.
Reading 17 and 19 distinguishes their actual captured pointers and indices.
Observing each selected
descriptor exercises branch-local shared copies without assigning concurrent
permissions to inactive payloads.

The explicit `[copy]` shared case joins `Left`, `Right` and `Empty` before
copying its descriptor storage. An empty payload contributes no descriptor;
the copied active payload still reads its original referent. Carrier disposal
multiplicity does not determine whether Psi admits that shared copy.
Repeating one unrestricted reference-bearing record in an array and copying
the array preserves both destination descriptors and the readable source.

The copyable mixed case also returns from a selector and passes through a
forwarder as a named result, nested call result and direct constructor. Its
common stamp and active descriptor survive array construction; the original
named result remains readable after forwarding. The affine `Selection` and
exclusive payload controls remain separate in the same program.
Projecting the copyable case from an enclosing record forwards only that
subtree and leaves the original record readable.
