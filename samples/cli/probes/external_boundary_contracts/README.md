# External boundary contracts

Calls libc `abs` through three requirements of one public boundary trait,
each bound to `_abs` by a `Build::bind` row in `build.omg`. The program exits
0 only when all three results match, including a field/local operand and
state forwarding.

`omega run main.omg` must prepare package review and execute on macOS ARM64.
Each call carries its requirement's `requires value > -2147483648` contract;
no per-symbol realization machine stands between caller and binding. It does
not claim a Terminal interpreter implementation of libc.
