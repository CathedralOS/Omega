# Array preparation inside a terminal call arm

Run `omega run --both main.omg`; both engines must exit 0.

The selected terminal call takes a scalar argument, a fixed array, and another
scalar argument. Each scalar and each array element increments the same counter.
The callee must observe 1, then array elements 2 and 3, then 4. The skipped arm
must leave the counter unchanged. This checks that ordinary argument preparation
runs inside the selected arm in authored order.
