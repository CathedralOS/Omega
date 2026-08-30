# Rung: Gamma — safe definitional computation

[Lattice overview](../bootstrap_lattice.md) | Prev: [Beta](beta.md) | Next:
[Delta](delta.md)

Gamma is the first safe definitional rung: enough algebraic data, pattern
matching, typing, and recursion to implement the Delta compiler without making
Beta understand Delta.

## Adds

- algebraic data types and pattern matching;
- checked signed integers and compact immutable bytes;
- pure functions, recursion, and proper tail calls;
- a small monomorphic static type system; and
- explicit, bounded evaluation resources.

Gamma has one typed executable contract: `data* def*`, exhaustive matches,
nominal immutable data, and no trailing untyped expression. The required
compiler artifact is written in Beta and emits Alpha tape for arbitrary source
accepted by that contract. An interpreter may remain a bounded oracle, but the
canonical edge yields a standalone tape for the Gamma-written Delta compiler
without an external Beta compiler or host transformation.

## Direct responsibility

```text
Beta-written Gamma compiler source
  └─ beta_compiler.tape ─▶ gamma_compiler_bytecode.tape

Gamma-written Delta compiler source
  └─ gamma_compiler_bytecode.tape ─▶ delta_compiler_bytecode.tape
```

Gamma implements the Delta compiler. It does not merely provide an evaluator
for a Beta-written translator that already parsed Delta.

D19 makes the generated application adapter a sealed compilation input rather
than Gamma syntax. `ConformanceBytesV1` selects pure
`main : Bytes -> Bytes`; `DeltaCompilerV1` selects the source-owned pure
`main : Bytes -> DeltaCompileOutcome`. The latter sum contains only
`Complete(Bytes)` and a structured Delta rejection, and its profile-owned
reason-code table is checked as a complete bijection over the exact resolved
constructors before emission. A generated Alpha adapter owns sealed byte I/O,
private exhaustion, internal failure, and the selected external observation
contract. Gamma source receives no general I/O primitive and matching names do
not select `DCOUT`.

D20 fixes the resolver beneath those profiles. Types, constructors, functions,
and local values occupy four grammar-selected namespaces. Globals are unique
within their own namespace; local bindings cannot shadow an active binding but
may reuse names in disjoint scopes. Collection precedes mutually visible type
resolution, and duplicates reject at the exact later declaration or binder.

D21 requires every valid `Bytes` logical length to fit nonnegative `Int`.
Concatenation checks the operands' stored logical lengths before allocation and
traps on an exact sum above `INT64_MAX`; physical storage exhaustion remains
`Incomplete`. D19 profiles validate their sealed-input maxima against the same
bound before adapter emission.

## Current migration

`source/gamma/compiler/gamma_compiler.beta` now owns the moved strict frontend,
direct Alpha payload/fixup substrate, executed heap/stack and checked-`Int`
helpers, resolved expression lowering, and profile-neutral whole-function
label/body emission. It also validates both exact D19 source schemas and the
26-code Delta rejection bijection without declaration-order authority. Q2's
physical profile realization remains open; the source is incomplete and has no
published tape. Its
251,142-byte historical fixed gate exhausted the former V1 Alpha ceiling before
those later slices and the D19 adapters. D23 therefore selects the coherent
`AlphaBootstrapV2` profile—a one-MiB stamped hole and 1,048,572-byte raw-tape
maximum across seeds, compilers, checker, and exact gates—rather than another
Gamma-specific density gate or private execution path.
`source/gamma/interp.beta` remains a bounded semantic oracle; it does not define
an alternate Gamma language or a serialized-AST runtime. Their
now-hardened historical omission of match exhaustiveness remains the warning
that differential agreement cannot establish a rule both sides omit. The former
Beta-written Delta-to-Gamma route was outside Gamma ownership and is deleted
rather than retained as the Delta edge or a compatibility layer.

## Must not contain

No mutable host memory, hardware boundary, package manager, product optimizer,
or Delta parser hidden in Beta. Proof checking is not a Gamma language feature;
the universal checker remains Alpha-owned and outside the language rung.

## Implementation frontiers

- retain D23's coherent `AlphaBootstrapV2` profile and the consolidated adjacent
  conformance gate through publication;
- resolve Q2's exact physical profiles, complete the two D19-selected adapters
  and remaining lowering in the exact Gamma compiler source, then publish its
  artifact closure;
- reuse the interpreter only as a specification or isolated algorithm source
  without turning runtime interpretation into a permanent dependency;
- emit exact Alpha tapes and checked source-to-tape certificates; and
- escalate on terrible compiler performance, Alpha verbosity, or proof
  explosion rather than adding special Gamma accelerators.
