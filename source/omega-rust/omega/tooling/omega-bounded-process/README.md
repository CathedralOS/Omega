# Omega Bounded Process

This crate owns neutral native child-process preparation, concrete resource
limits, bounded standard-stream transfer, wall-clock deadlines, and explicit
process-container cleanup.

```text
src/
├── lib.rs          crate responsibility map
├── preparation.rs  opaque command and resource-limit vocabulary
├── lifecycle/      process group or Windows Job ownership and completion
└── capture/        bounded duplex transfer and deadline coordination
```

Windows uses a kill-on-close Job Object with process, memory, and CPU ceilings.
Unix applies inherited resource-limit intersections, launches a process group,
and kills that group during cleanup. A Unix descendant can deliberately detach
from its process group. Neither platform path claims filesystem, executable,
credential, or network isolation. Callers requiring a security sandbox must
provide and verify that separate boundary.
