# entry_mut_scalar_param

`machine Main::main(out: &mut i32)` — the entry machine binds a borrowed
scalar context, not `&mut Main`: stores through `out` land in the
caller's storage. Refines the earlier note that entry `&mut` params
were refused — a plain `&mut i32` compiles fine; the earlier refusal
was about `Binding<T>` contexts, not borrows.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
