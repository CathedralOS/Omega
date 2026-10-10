# encoded_recast_views

First sample instantiating a layout-typed field:
`SignedByte<PortableByte>` inside `RawView` shares a
scalar's original storage; `&mut main.bytes[0] as
&mut RawView` mints the encoded view, and a write through
`view.value.value` lands on the backing byte.

## Gaps found while authoring (filed)

`value as &mut ForeignByte` (a second recast through the
view down to a foreign byte type) rejects "requires mutable
source access" even though the backing borrow is `&mut` —
the corpus sibling may rely on an access plan this sample
doesn't construct; filed for owner triage.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
