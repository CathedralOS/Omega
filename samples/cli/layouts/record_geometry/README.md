# Record storage geometry

Run `omega run --both samples/cli/layouts/record_geometry/main.omg`.
Expected exit: 0 in native and Terminal execution. Any wrong geometry traps.

The record occupies 16 bytes with alignment 8 on the four authored hosted
targets. The program computes 24-byte extents and an 8-byte difference through
locals, copied records and fields, array elements, ordinary arguments and
results, casts, dispatch, and state transfers. A helper also returns a query
or a literal from separate states. Multiplication computes a 32-byte extent.
Overwriting the original field must not change the saved scalar's value.

The compiler retains the observations' bounds until target closure. The sample
uses Exact arithmetic throughout; wrapping arithmetic and hard-coded query
results would hide the proof obligations this program exercises.
