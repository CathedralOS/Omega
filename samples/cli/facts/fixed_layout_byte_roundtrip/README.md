# Guarded fixed-layout byte roundtrip

A byte roundtrip through an explicit fixed layout lends a decoded view only after its payload bound is established, preserving every field and neighboring storage.

This row belongs to [LIBRARY-REPRESENTATION-FAMILIES](../../../../TASKS.md).
The [local program](local/main.omg) initializes a sparse 40-byte
`LayoutStorage<Payload, Spread>` inside five words of a seven-word array, copies
the complete byte region, and lends the decoded view after proving `second >= 42`.
Its consumer checks all three fields and both source/destination canaries.
The [static reject twin](local/invalid/main.omg) changes only the guard to `true`;
it rejects the attempt to publish an unestablished valid view.

The [rejected-payload control](local/rejected_payload/main.omg) writes 41 with the
same bound, layout and byte operation. It must take `None` and exit 0; the
original program must take `Some` and exit 0. Together they detect a missing or
inverted guard, while the field values and canaries detect incorrect storage
selection. An exclusive initial view exposes independently valid fields without
assuming its record predicate; escaping mutable authority also owes the original
occupied storage, including padding.

The local pair and rejected-payload control are qualified on Linux x86_64 with
native/Terminal agreement. [Run 41387515883](https://github.com/CathedralOS/Omega/actions/runs/41387515883)
also checks seven intended rejection controls and reloads the retained Terminal
artifact using the matching optimized Rust producer at `70ba9ee572b4`.
The [typed interior-region prerequisite](../fixed_region_byte_roundtrip/README.md)
is landed. The other eight transport places and other hosted targets remain open.
