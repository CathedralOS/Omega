# literal_field_order_snapshot

Record literal fields evaluate in order: `Packet { samples: context.samples,
stamp: stamp(context) }` snapshots `context.samples` before `stamp` mutates
it — the packet keeps `[30, 40]` while the source lands at 90.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
