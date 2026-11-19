# Exclusive typed field visitation

Fact: authorized typed mutation lends each non-erased field exclusively to the selected callback once and restores the caller's access afterward.

The callback adds one to each field. The caller checks all three results,
including that the maximum wraps to zero, and counts exactly three calls.
The match example runs both choices and checks that the selected record
changes while the other keeps all its original values. Checking the two
records afterward also exercises the return of access to their owner.
The constant example copies its subject into mutable local storage before
visiting its fields.

Each of the nine examples binds the four hosted targets and expects exit 0
in native code and Terminal Psi. Each reject twin accepts only a `u32`
callback type, while the visited fields require `u64`.
