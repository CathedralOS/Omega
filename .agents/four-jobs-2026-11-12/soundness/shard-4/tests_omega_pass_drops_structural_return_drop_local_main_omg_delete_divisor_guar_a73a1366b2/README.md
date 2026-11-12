# structural_return_drop_local

A body that returns a structural value disposes of every local it does
not return. `exercise` only borrows its `token`, so the return runs the
nominal drop; `relay` moves its token into `sink` before returning — the
callee drops it and the return must not drop it again.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
