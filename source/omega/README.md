# Omega product compiler source

This package is the product root and Terminal-Psi consumer. The
sibling [`../psi/`](../psi/) package owns target-neutral source processing,
checking, and Terminal Psi; this package owns optimization, target realization,
artifact emission, and the product entrypoint.

The product compiler has two exact source implementations. `D` is written in
Delta; `C` is written in Omega using a deliberately conservative,
compositional subset of ordinary Omega.

```text
delta compiler + Delta source D → omega₀
omega₀ + Omega source C          → omega
```

`omega₀` may be conservatively generated and slow. It is already a full Omega
compiler because `D` implements the product language. The second build closes
the self-hosting edge and may improve the compiler executable; it does not add
language functionality.

Both compiler outputs are platform-independent Alpha tapes. Native target
realization belongs to this product phase only for user-program artifacts; it
does not turn any compiler rung into a native bootstrap artifact.

## Ownership

- [`../psi/`](../psi/) — target-neutral source, proof, and terminal semantics;
- this root — target realization, optimization, artifact emission, the
  Delta-written source closure `D`, and Omega-written source closure `C`;
- [`../omega-rust/`](../omega-rust/) — maintained Rust implementation and
  differential comparator, never bootstrap authority;
- [`../delta/`](../delta/) — final lower-rung compiler and direct first-build
  producer.

That source choice does not define a dialect or restrict programs the resulting
compiler accepts. Standalone viewers, interpreters, REPLs, and proof
explorers remain outside `C` unless the compiler executable imports them.

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md); bootstrap
closure is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `build.omg`, `main.omg` | Current roots of Omega-written compiler closure `C`; the closure is incomplete but is extended in place. | Delete or replace only when an exact package-root ruling changes `C`; do not preserve alternate hosted roots. |
| `omega_compiler.delta` | Incomplete Delta-written compiler closure `D`; currently owns strict source-view UTF-8 framing, the final exact Alpha tape encoder, bind-once label/fixup ownership, structural replay before sealing, and no invented invocation boundary. | Extend in place as `D`; replace a completed component only atomically with an equally complete final Delta implementation. |

The four empty target declarations in `build.omg` are temporary compatibility
scaffolding, not product architecture. Delete them as soon as immutable target
activation/reach closure lands, and normalize `windows_x64` to
`windows_x86_64` in that same migration.

`omega_compiler.delta` (`D`) now exists but is intentionally incomplete, and
both descriptive compiler tapes remain absent. The canonical sealed
package-closure request for the standalone Omega compiler is owner-blocked; no
raw-single-file stdin convention may stand in for it. Boundary-independent
final internals may be authored in `D` before that ruling, but no placeholder,
generated source closure, viewer, or standalone bootstrap owner is retained
while the artifacts are absent.
