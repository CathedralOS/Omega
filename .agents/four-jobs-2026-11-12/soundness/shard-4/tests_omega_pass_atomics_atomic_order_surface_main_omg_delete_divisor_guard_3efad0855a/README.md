# atomic_order_surface

Atomic ops carry ordering as checked data: `AtomicU32/U64/Bool` with store/load/fetch_add/fetch_sub/swap/compare_exchange under GlobalOrder/ReceivePublish/Publish/Receive/NoOrdering; AtomicBool admits only load/store.

Compile-checked only: run a check target, not native.
