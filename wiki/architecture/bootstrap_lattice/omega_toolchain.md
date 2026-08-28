# Psi/Omega toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta](rungs/delta.md) |
[Compiler source profile](compiler_source_profile.md)

Omega is the product-language endpoint, not another small bootstrap language.
Psi owns source processing through terminal portable IR. Omega consumes that IR
and owns target closure, optimization, and native emission.

```text
language capability: Alpha → Beta → Gamma → Delta → Omega

Delta compiler source
  └─ lower-rung publication ─▶ Delta-produced compiler

exact ordinary-Omega product source C
  └─ Delta-produced compiler ─▶ omega₀

the same C
  └─ omega₀ ─────────────────▶ omega
```

There is no additional compiler role between Delta and `omega₀`. The first
product artifact may be conservative and slow. It must nevertheless implement
the accepted source with exact Omega semantics. The rebuild changes artifact
quality and closes self-hosting; it does not introduce a new language or source
generation.

## Source ownership

- `source/omega/psi/` owns the target-neutral product compiler half.
- the rest of `source/omega/` consumes terminal Psi and owns target
  realization and the product entrypoint.
- `source/omega-rust/{psi,omega}/` is the current working Rust implementation
  and comparator. It is useful during migration but grants no authority.
- `source/delta/` owns the compiler that performs the first product build and
  the lower-rung meaning used to publish it.

## The source subset used by `C`

`C` deliberately uses only a compositional subset of ordinary Omega. It has no
private syntax or altered semantics, and the subset is not a named language or
dialect. The Delta-produced compiler may reject Omega programs using forms not
needed by `C`, but every admitted program retains normal Omega meaning.

The product compiler built from `C` implements the complete Omega language for
users. A feature may therefore be absent from the compiler's own implementation
source while still being parsed, checked, and lowered by that compiler.

The source manifest demonstrates closure under general compositional forms;
it must not become a file allowlist or a collection of hard-coded AST shapes.

## Assurance

Hosting does not prove compiler correctness. Every edge binds exact source and
artifact subjects to canonical semantics, an observation profile, target
semantics, reconstructed obligations, certificates, and disclosed admissions.
The proof kernel checks derivations independently of the producer.

Terminal Psi remains the boundary between product halves, not a mandatory IR
for the Delta implementation itself. The Delta compiler may lower the
ordinary-Omega source used by `C` conservatively by any checked route. It
merely cannot redefine the accepted source meaning.

The exact execution order and unfinished edges live in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
