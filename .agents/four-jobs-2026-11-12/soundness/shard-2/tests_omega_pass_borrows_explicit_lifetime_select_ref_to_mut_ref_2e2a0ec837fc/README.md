# explicit_lifetime_select

`data View<'buf> { body: &'buf mut i32 }` + `select<'left,'right> -> View<'left>` — erased lifetime args select which borrow the result retains; the other stays free to store.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
