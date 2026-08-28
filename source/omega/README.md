# Omega product compiler source

This root owns the Omega-written product compiler together with
[`../psi/`](../psi/). Psi owns target-neutral source processing, checking, and
Terminal Psi; Omega owns optimization, target realization, artifact emission,
and the product entrypoint.

The product compiler is authored once as the exact source closure `C`, using
ordinary Omega constrained to the compositional `Ωself` authoring profile.

```text
published Delta-produced compiler + C → omega₀
omega₀ + the same C                    → omega
```

`omega₀` may be conservatively generated and slow. It already implements the
full product language because the optimizer and advanced lowering live in `C`.
The second build improves the compiler executable; it does not add language
functionality or close a missing bootstrap dependency.

## Ownership

- [`../psi/`](../psi/) — target-neutral source, proof, and terminal semantics;
- this root — target realization, optimization, artifact emission, and product
  command source;
- [`../omega-rust/`](../omega-rust/) — temporary Rust implementation and
  differential comparator, never bootstrap authority;
- [`../delta/`](../delta/) — final lower-rung compiler and direct first-build
  producer.

`Ωself` constrains forms used to author `C`; it does not restrict programs the
resulting compiler accepts. Standalone viewers, interpreters, REPLs, and proof
explorers remain outside `C` unless the compiler executable imports them.

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md); bootstrap
closure is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
