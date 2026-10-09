# Runtime scalar-array observations under selected payloads

Run `omega run --both tests/omega/pass/calls/runtime_case_payload_scalar_array_read_exit/main.omg`.
Both engines must exit 0. Arrays held directly by a selected case and arrays
inside two sibling record payloads retain their declaration-local storage paths
after a named-state transfer. Different indices and sibling values distinguish
the selected storage from an incorrectly reused payload or array element.
Empty cases and the guarded out-of-range route perform no payload read.
Borrowed record arrays also select nested fields beside drop-bearing siblings;
repeated observations leave the caller's owner available for the next call.

The observation reads the original array backing without copying its containing
owner. Case authority and index bounds still belong to the ordinary
checked dispatch and read operations; this is not another case-specific index
grammar.
