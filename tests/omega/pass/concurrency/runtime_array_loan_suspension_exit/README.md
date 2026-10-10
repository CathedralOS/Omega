Run `omega run --both tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg`.

The possibly-suspending selected helper borrows an array containing two shared
loans. Its crossing must retain the descriptor array and both original backing
values. The caller observes both loans again after the helper returns and exits
0. Inline completion is the execution control, not evidence of park/resume or
runtime preservation.

The same array then crosses an ordinary call with a runtime selector. Both
reads must follow the stored references; their pointer bytes are not integers.
