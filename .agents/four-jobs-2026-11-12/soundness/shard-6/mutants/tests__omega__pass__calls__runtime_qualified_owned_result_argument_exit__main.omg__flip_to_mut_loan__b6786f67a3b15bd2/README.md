Run with `omega run --both tests/omega/pass/calls/runtime_qualified_owned_result_argument_exit/main.omg`.

An affine record returned by a machine retains its member's declared range
when moved into another machine. The consuming call must not require literal
construction syntax or erase the range to obtain an executable operand.
