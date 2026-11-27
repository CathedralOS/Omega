# Libc descriptor read through a Build binding

This Linux runtime-admission control reads one byte from descriptor 0 through
the libc `read` import. It checks that an import bound only through
`Build::bind_native::<Probe::read>` reaches the real supplied input rather than a
fabricated stream. Both the published executable and the interpreted program
read the supplied `A` or trap on exhaustion.

The program declares `Probe::read` as an ordinary one-member boundary trait.
`build.omg` binds it to the exact versioned glibc `read` on each Linux target;
the source carries no target-prefixed leaf or `via` clause.

On 2026-11-09 on `linux_x86_64`, both the executable and the interpreter exited
0 with `input.txt` on stdin. With empty stdin, the executable received signal 4
and the interpreter reported `Trap`.
