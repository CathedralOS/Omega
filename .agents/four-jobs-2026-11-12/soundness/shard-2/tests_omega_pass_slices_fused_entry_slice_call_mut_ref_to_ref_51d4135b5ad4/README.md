# fused_entry_slice_call

A `[u64; 3]` field on the entry receiver coerces to a checked callee's
`&mut [u64]` through the borrowed-view presentation; the Fused entry
receiver keeps its Terminal attachment while it lends the field —
the establishment rejoin survives the call.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
