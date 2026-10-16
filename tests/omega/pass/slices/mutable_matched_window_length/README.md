# Mutable matched window length

A match of two mutable windows publishes their shared length. The caller
binds that call, then uses the length from the local, from a field of the
record that stores the window, and from the parameter that writes through
it. The source is a lent local array, the other match arm, a record field,
or a length-3 window of a six-byte row.

Native and Terminal execution both exit 0.
