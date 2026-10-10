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
