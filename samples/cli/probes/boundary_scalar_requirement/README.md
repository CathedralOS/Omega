# boundary_scalar_requirement

`TimedWait::poll` is a one-member boundary trait requirement that reaches `Clock`. The selected checked provider returns 0 for a 20ms wait with no descriptors and 1 otherwise. The entry calls that wait through a later state and the other timeout directly.

Expected exit: 0.
