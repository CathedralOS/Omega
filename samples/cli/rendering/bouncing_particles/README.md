# Bouncing particles

Four particles bounce in a 20 by 10 grid for 60 frames. The source transports an
explicit `Main` subject between states; guards on compound row-major indices
remain valid until a write changes one of their operands.

After ordinary local package review, run:

```sh
omega run --both samples/cli/rendering/bouncing_particles/main.omg < /dev/null
```

On Linux x86_64, native execution exits 0. The checked interpreter currently
declines `LinuxClockWait::poll` because it has no selected Clock provider.
Native stdout contains 60
cursor-home frames, each followed by ten 20-character grid rows containing only
spaces and `*`, then the completion message and the Enter prompt. EOF completes
the prompt. Each frame retains between one and four visible particle cells;
coincident particles share a cell. Clock sleeps preserve the authored pacing.
