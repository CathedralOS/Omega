# Host support

[Bounded processes](../../foundation/bounded-process/README.md) owns child-process execution,
output capture, termination and cleanup. [Platform custody](platform-custody/src/lib.rs)
owns filesystem identity, access observations, temporary directories and atomic
file publication. Neither package compiles a program or selects its providers.

Compiler representations carry program identities and plans. Products select
output destinations and invoke these host mechanisms after compilation.
