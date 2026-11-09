# Carry an empty-slot check across calls

The pool and both index values pass through nested calls before writing the selected slot. This variation uses `next + key` for the same 8192-slot operation. The separate `key <= 8191 - next` requirement proves addition safety.

Both index terms are nonzero: the successful calls use 1 + 1 and 2 + 1, and the occupied fallback revisits the first slot.

The guarded call receives a payload from an effectful helper that is not an input to the selector. The program checks distinct 47/59 payloads, the 83 occupied fallback, two untouched Empty neighbors and 91/97 surrounding canaries. Its counter must be two: the occupied fallback must skip the helper.
