# A write through the collection retires a saved indexed IEEE observation

The saved Boolean `accepted` proves finite bounds for `reading.values[index]`.
Before the conversion, the program passes `&mut reading.values` to a call that may write any element. The saved fact about the element
must not survive that write.

This twin differs from
[the positive](../../../pass/float/runtime_ieee_collection_disjoint_exit/main.omg)
only in that write, which replaces the positive's disjoint `reading.other` store.
