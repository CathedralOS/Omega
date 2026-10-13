# arm_field_placement

`pack(swap, left, right)` places linear inputs in different record
fields on its two arms; the caller discharges every member through the
binding-backed sink regardless of which arm produced it.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
