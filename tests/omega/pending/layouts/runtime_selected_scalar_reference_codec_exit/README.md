A runtime Boolean selects either a canonical `&mut i64` or a signed-byte-backed `&mut i64` at the same match join. One forwarding helper writes through the selected reference on both invocations. The canonical scalar becomes -128, byte 0 becomes 127, and neighboring byte 1 remains 85. Both execution engines must exit 0.

The checker currently rejects reference-valued match results because their
branch custody joins are not implemented. This fixture preserves that source
frontier; it must not be rewritten into compile-time selection or specialized
helpers to satisfy encoded-storage transport.
