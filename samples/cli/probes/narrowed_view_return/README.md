# narrowed_view_return

`holder.bytes[1..2]` returns as `-> &[u8]` — a member narrowed sub-view keeps the holder loan alive in the returned view. `Choice::Return -> observed` is the bare-param arm result.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
