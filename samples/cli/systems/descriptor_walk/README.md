# Descriptor walk

Walk three firmware-style records stored in a field of caller-owned storage.
The runtime `descriptor_size` determines each step; the shared `Desc` view
reads the original byte buffer, and kinds 4 and 5 contribute 20 + 30 pages.
`omega run --both main.omg` must report native and Terminal exit 50.

Each nested helper receives its mutable storage loan and offset explicitly.
Its own contract establishes the recast footprint; the loop transfers control
with constant stack usage. Numeric conversion helpers called from fused states
remain ordinary dependencies of their authored state owners.

This exercises the projected-storage requirement of
`STATE-LOCAL-VALUE-FRONTIER`. The bounds control under
`tests/omega/fail/borrows/projected_record_region_exceeds_backing` reduces the
backing extent by one byte and must reject the same 16-byte view at offset 32.
Development runs use the normal target-specific `omega update` package workflow
before accepting the local standard library's program-entry binding.
