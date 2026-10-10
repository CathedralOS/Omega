# atomic_bitwise_rmw

Bitwise read-modify-write on both atomic widths: `fetch_or`, `fetch_and`,
`fetch_xor` each return the committed prior on `AtomicU32` and `AtomicU64`;
`NoOrdering` is the weakest ordering atom beside `Publish`, `ReceivePublish`,
`GlobalOrder`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
