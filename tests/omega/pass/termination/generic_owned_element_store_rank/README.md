# generic_owned_element_store_rank

A `terminates by` machine over a generic `Box<T>` moves an owned `T` local
out of element storage and stores it back before its decreasing recursive
call. The state's assignment prefix walk meets the unresolved type
parameter `T`; it must keep a conservative opaque-region origin for `T`
instead of returning no inventory — otherwise the whole write frame is
opaque and the entry-relative rank premise cannot be shown preserved.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
