# Rewritten selector retires a saved indexed IEEE observation

The saved Boolean `accepted` proves finite bounds for `reading.values[slot]`.
Before the conversion, `slot` itself is reassigned, so the later read names a
different element whose value the guard never observed. The fact must not
survive: it belongs to the element the old selector reached, not to the
spelling `reading.values[slot]`.

This twin differs from
[the positive](../../../pass/float/runtime_ieee_collection_disjoint_exit/main.omg)
only by reading through a mutable `slot` copy of the index and rewriting it
between the check and the conversion. Its sibling
[collection-write twin](../ieee_collection_write_retires_guard/main.omg) writes
the observed element instead.
