# Grid walk

From the repository root, run `omega --check samples/cli/simulation/grid_walk/main.omg`,
then `omega run --keep samples/cli/simulation/grid_walk/main.omg` after ordinary
package review. Press Enter at the prompt; EOF also completes the sample.

The unchanged five-step walk (east three times, south twice) prints its result
message and exits with **32**, encoding `x * 10 + y`. A nonzero exit is intentional.
The build declares Windows x86-64, Linux x86-64/ARM64, and macOS ARM64 entrypoints;
execute on the matching host to verify its native result.
