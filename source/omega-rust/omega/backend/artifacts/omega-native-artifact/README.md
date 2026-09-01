# Native artifact

This crate owns the authority-free, replayable join from one canonical
Terminal-Psi artifact to its target object and final executable image. It does
not grant publication, installation, provider, or runtime authority.

## Source map

- `src/lib.rs` is the crate entrance: artifact construction, selected-provider
  projections, top-level replay, and artifact identity.
- `src/physical/mod.rs` is the D32 entrance.
- `src/physical/model.rs` owns replayable optimization-projection, D41 parent,
  physical-child, span, and evidence carriers.
- `src/physical/derivation.rs` independently derives and validates the exact
  physical relation from Terminal semantics, object/image custody, and strong
  selected-plan digests.

The current physical lane is deliberately narrow: an unoptimized handoff, an
exact empty checked D29 boundary-operator roster, and Linux ELF
`CompilerBuiltin(LinuxExitGroupI32)` on x86-64 or AArch64. The scope is bound
into artifact identity. Explicit optimization, nonempty D29 demand, normalized
foreign calls, and other executable evidence roles yield no D32 evidence while
the underlying native artifact remains usable. Consumers requiring
final-realization evidence must reject that absence.
