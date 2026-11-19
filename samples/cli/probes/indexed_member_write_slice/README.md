# indexed_member_write_slice

An indexed member write reads back through a slice view:
`main.rooms[index].exit_count = 7` then `&main.rooms[..]` —
`room_slice[index].exit_count`; `&Main` immutable receiver states.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
