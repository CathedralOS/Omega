# Libc descriptor read through a Build binding

This Linux runtime-admission control reads one byte from descriptor 0 through
the libc `read` import. It checks that an import bound only through
`Build::bind<Probe::read>` reaches the real supplied input rather than a
fabricated stream, so that each engine either reads the supplied `A` or traps on
exhaustion.

The program declares `Probe::read` as an ordinary one-member boundary trait.
`build.omg` binds it to the exact versioned glibc `read` on each Linux target;
the source carries no target-prefixed leaf or `via` clause.

Observed 2026-11-09 on `linux_x86_64` with `omega run --both`: with `input.txt`
on stdin both engines exit 0; with empty stdin both trap (`native signal 4`,
`interp crash: Trap`). The engines agree on both inputs.
