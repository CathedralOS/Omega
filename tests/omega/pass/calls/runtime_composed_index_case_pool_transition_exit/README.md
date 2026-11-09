# Carry an empty-slot check across calls

The pool and both index values pass through nested calls before writing the selected slot. This variation uses `next + key` for the same 8192-slot operation. The separate `key <= 8191 - next` premise proves addition safety.

The guarded call receives a payload from an effectful helper that is not an input to the selector. The program checks distinct 47/59 payloads, the 83 occupied fallback, an Empty neighbor and 91/97 surrounding canaries. Its counter must be two: the occupied fallback must skip the helper.
