# Guarded fixed-layout byte roundtrip

A byte roundtrip through an explicit fixed layout lends a decoded view only after its payload bound is established, preserving every field and neighboring storage.

The local program initializes a sparse 40-byte `LayoutStorage<Payload, Spread>`
inside five words of a seven-word backing array, copies its complete byte region,
and lends the decoded view only after proving `second >= 42`. It checks all three
payload fields and both source/destination canaries. The intended reject twin
changes only that guard to `true`; it must not publish an unestablished valid view.

The [typed interior-region prerequisite](../fixed_region_byte_roundtrip/README.md)
is landed. This guarded decoder is not yet qualified: typed recast establishment,
source restoration and borrowed result transport remain under repair. The other
eight transport places and other hosts remain open.
