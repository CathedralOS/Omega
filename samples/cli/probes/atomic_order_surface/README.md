# atomic_order_surface

Normalized atomic orderings as checked source data: `AtomicU32/AtomicU64/AtomicBool` locals + `.store(v, GlobalOrder)`, `.load(GlobalOrder|Receive)`, `.fetch_add(5, ReceivePublish)`, `.swap(20, GlobalOrder)`, `.compare_exchange(20, 30, GlobalOrder, Receive)` — success/failure orderings spelled separately, results land `u32 in Wrapping`. Learnings: AtomicBool admits only load/store (no swap — returns numeric policy type); `u32 in Wrapping -> i32` is still an exact-cast obligation.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
