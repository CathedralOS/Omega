# Wrapping array literals

The program adds one to the largest `u32` in a wrapping array and checks that the result is zero. It also reads 1 from a nested array and 2 from an array stored in a record field. An incorrect value traps.

Run the native and Psi checks with `mbx xtask sample-gate --filter samples/cli/probes/wrapping_array_literal`.
