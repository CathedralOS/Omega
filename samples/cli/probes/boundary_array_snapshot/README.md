# boundary_array_snapshot

A boundary receives an owned primitive array between scalar arguments:
the later scalar operand mutates the source (`stamp` writes index 0 to
99), but the provider observes the array snapshot captured earlier —
the caller still sees the write.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
