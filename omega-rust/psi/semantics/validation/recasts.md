# Recast validation implementation

The [recast contract](../../../../wiki/spec/layouts/recasts.md) defines the
representation judgment. `src/recasts.rs` and its child modules implement the
supported source subset; their restrictions are not new language semantics.

Shared scalar, bounded byte-region, and recursively nested record/array views
are supported. Mutable typed aliases require equal geometry and equivalent
leaf representation sets. Integer range leaves normalize to exact
two's-complement bit sets; same-carrier float ranges use interval inclusion
for shared views and equality for mutable views. Equal-looking cross-carrier
predicates do not establish equivalence.

One direct erased-lifetime shell around an eligible synthesized record may
use its exact instance layout when lifetime arity is exact, no runtime
arguments remain, and stored fields do not recursively carry lifetime
applications. This applies only at the target root; array or nested-record
descent cannot erase more shells into layout authority.

Complete-source and proved interior unsized slices use exact tiling and
compiler-derived element stride, including padding. Runtime multi-byte
offsets need proved congruence. Source recasts are currently validated as direct
reference-typed `let` initializers restating the target type. Other positions
remain fenced; a mismatched bare borrow cannot bypass the recast judgment.

Native and interpreter projection/state forwarding preserve backing identity.
`Placed<P, T>` and its accessors are excluded: representation equivalence
cannot replace the placement access plan. Focused source cases live in
`tests/omega/{pass,fail}/recast/`; compiler integration coverage is in
`omega-rust/omega/compiler/compiler/tests/recast_views.rs`.
