# A newly selected constant dependency has its own initializer job

ROOT is authored before LATER. Its ordinary overloaded choose(1u32) selects the
u32 body, which reads LATER. Once typed selection reveals that edge, the scheduler
must settle LATER's own ordinary overloaded other(37u32) initializer before
resuming ROOT. Both constants must be 37. The unselected u64 bodies return 73
and 91, so their values cannot satisfy the oracle. Both engines must exit 0;
all four hosted targets bind main.

This is one nested scheduler composition beside
[const_overload_selected_dependency_exit](../const_overload_selected_dependency_exit/main.omg).
The existing [selected-body cycle control](../../../fail/compile_time/const_overload_selected_dependency_cycle_rejected/main.omg)
retains the refusal boundary; it does not establish this new positive's outcome.
The parent shared lane owns frozen R26 and R29 observations of these identical
sources. No compiler execution or baseline result is claimed here.
