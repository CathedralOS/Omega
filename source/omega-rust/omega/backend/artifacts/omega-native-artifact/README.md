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

The current physical lane is deliberately narrow: an unoptimized handoff,
exact D29 custody for supported compiler-intrinsic and checked-body operator
applications, and supported Linux ELF compiler-builtin settlements on x86-64
or AArch64. Each surviving physical occurrence has one replayable child bound
to its D29 or D41 parent, and that scope is bound into artifact identity.
Explicit optimization, unsupported D29 roles, normalized foreign calls, and
other executable evidence roles yield no D32 evidence while the underlying
native artifact remains usable. Consumers requiring final-realization evidence
must reject that absence.
