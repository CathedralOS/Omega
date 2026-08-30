# Omega product toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta](rungs/delta.md) |
[Compiler source profile](compiler_source_profile.md)

Omega is the product-language endpoint. Psi remains an internal target-neutral
product compiler boundary; it is not a bootstrap rung.

```text
Gamma-written Delta compiler ─▶ delta_compiler.tape

Delta-written Omega D
  └─ delta_compiler.tape ─────▶ omega₀.tape

Omega-written Omega C
  └─ omega₀.tape ─────────────▶ omega.tape
```

`omega₀` and `omega` are full Omega compilers represented canonically as Alpha
tapes. `omega₀` may itself be unoptimized, but the optimizer implemented by its
Delta-written source can run while compiling `C`, producing a better `omega`.

## Source ownership

- `source/delta/compiler/` owns the Gamma-written compiler that accepts Delta.
- the Delta-written first Omega compiler source closure `D` belongs under
  `source/omega/`, even though its files end in `.delta`;
- `source/{psi,omega}/` owns the Omega-written self-hosting closure `C` and the
  product target-neutral/target-specific split; and
- `source/omega-rust/{psi,omega}/` remains the working Rust implementation and
  comparator without bootstrap authority.

The former restricted Delta-written Darwin compiler prototype was deleted
rather than relabeled as `D`. It lacked the complete Omega frontend, Psi
pipeline, optimizer, and product target model; the real closure `D` is being
authored under `source/omega/` from the full contract.

## The source profile used by `C`

`C` deliberately uses only a compositional subset of ordinary Omega. It has no
private syntax or altered semantics, and the subset is not a named dialect.
`omega₀` must accept the complete Omega language. The conservative profile
constrains only how `C` is authored; it does not narrow the language `omega₀`
implements. The compiler built from `C` implements that same complete language.

The source manifest demonstrates closure under general compositional forms; it
must not become a file allowlist or a collection of hard-coded AST shapes.

## Product targets versus bootstrap target

The compiler programs themselves remain Alpha tapes. Omega may emit native user
artifacts for ARM64, x86-64, UEFI, or other targets and may attach PCC evidence
to those artifacts. That product target machinery does not require Beta, Gamma,
or Delta to emit native code.

An optional general Alpha AOT realization can accelerate execution of
`omega₀.tape` or `omega.tape`, but it is checked against Alpha semantics and may
not specialize recognized compiler functions.

## Assurance

Both Omega tapes owe direct source-to-Alpha refinement. The first uses Delta
semantics for `D`; the second uses Omega semantics for `C`. Their different
source languages and expected different bytes are explicit. Neither a
self-build nor agreement with Rust substitutes for either proposition.

The exact execution order lives in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
