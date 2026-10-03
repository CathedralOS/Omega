# Field preconditions at recast formation

Complete the package review with `omega update --project .` and
`omega update --project . --resume`, then run `omega run --both main.omg`.
Both engines must exit 70.

The callee's entry precondition bounds a mutable field used as the byte offset
of a record view. The view captures that offset before the field changes and
continues to observe the original descriptor. The caller also checks that the
field mutation occurred. The precondition is an arrival fact, not a permanent
constraint on the field.

The `field_precondition_invalidated_before_capture` and
`field_precondition_call_invalidated_before_capture` negative fixtures write
past the region before view formation, directly and through a mutable call.
An expired entry bound cannot authorize either view.
