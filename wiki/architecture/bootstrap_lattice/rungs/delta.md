# Rung: Delta — compiler-host systems language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next:
[Omega product toolchain](../omega_toolchain.md)

> **Status: open publication and product edges.** Delta's language, compiler
> experiments, and lower-rung meaning exist. Publishing the canonical compiler
> without an external producer and using it to build the exact Omega compiler
> source closure remain open.

Delta is the terminal small-language rung:

```text
Alpha → Beta → Gamma → Delta
```

It is an independently specified deterministic compiler-host language, not an
Omega subset and not an extra product compiler. Delta v1 is selected from the
complete source needed for its canonical compiler and the direct Omega-source
compilation edge, together with compiler-host safety and maintainability needs.
The old sample corpus or a temporary producer cannot define the language by
accident.

## Direct responsibility

The published Delta compiler is the compiler that performs the first product
build:

```text
Delta compiler source
  └─ Delta→Gamma elaboration + Gamma execution ─▶ Delta compiler artifact

exact ordinary-Omega compiler source C
  └─ Delta compiler artifact ───────────────────▶ omega₀

the same C
  └─ omega₀ ────────────────────────────────────▶ omega
```

No standalone bootstrap compiler exists between these edges. `omega₀` is the
first product compiler artifact. It may be slow or conservatively lowered, but
accepted Omega programs retain their exact Omega meaning.

## Implementation owners

- `source/delta/compiler/main.alp` is the Delta-written compiler
  experiment.
- `source/delta/meaning/delta2gamma.beta` and its encoding tools provide the
  Rust-free lower-rung meaning route used to publish Delta artifacts.
- `source/delta/compiler/artifacts/` is reserved for exact artifacts admitted
  by the lower-rung producer edge; it remains absent until publication closes.
- `source/delta/compiler/validation/` owns publication verification and custody
  adjacent to the compiler artifact those checks admit.
- `source/delta/FEATURE_LEDGER.md` records candidate Delta facilities and the
  evidence still needed to retain them.

The meaning tools are part of Delta's publication proof, not a separate
language or bridge stage.

## Trust boundary

Delta's bootstrap host surface stays narrow: source bytes in; artifact and
diagnostic bytes out; explicit target/configuration input; deterministic exit.
Filesystem, environment, time, network, spawning, MMIO, and arbitrary foreign
calls are absent unless a complete source closure demonstrates an unavoidable
need and the admission is recorded explicitly.

## Closure criteria

Delta closes when:

1. its language contract covers the complete canonical compiler source;
2. the compiler artifact is published through the lower-rung route without
   Rust defining the result;
3. that artifact accepts the compositional ordinary-Omega surface exercised by
   `C`, rejecting unsupported source rather than approximating it;
4. it builds the exact product source closure `C` into `omega₀`; and
5. the resulting `omega₀` rebuilds that same `C` into the product compiler.

The exact work order lives in
[`TASKS_BOOTSTRAP.md`](../../../../TASKS_BOOTSTRAP.md).
