# Fixed timestep

A point accelerates at a steady rate while the program replays six uneven
frame durations. The simulation advances in 10 ms ticks and carries leftover
time into the next frame. Each frame prints an interpolated position using
the two most recent simulated positions.

From the repository root, run the sample on both the native and Psi engines
with the repository's exact output check.

```sh
mbx xtask sample-gate --filter samples/cli/games/fixed_timestep
```

The first 4 ms frame runs zero ticks. The next frame adds 16 ms and runs
two ticks. After the 27 ms frame, 7 ms are left over. The point's two latest
positions are 12 and 20, so the displayed position is 70% of the way from
12 to 20, or 17.6. After the first tick, the displayed position is one tick
behind the time the loop has accepted.

The 120 ms frame models a stall. The loop accepts 40 ms of it and drops
80 ms so the simulation does at most four ticks in one frame. The sample
uses a scripted clock and prints to the console so every run observes the
same timing cases.

The loop follows Glenn Fiedler's [Fix Your Timestep!](https://gafferongames.com/post/fix_your_timestep/).
