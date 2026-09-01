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
`main : Bytes -> DeltaCompileOutcome`. The latter sum contains success, a
structured Delta rejection, and D31/D34's attributed/aggregate bounded-witness
application-static-storage refusals. Its profile-owned reason-code table is
checked as a complete bijection over the exact resolved constructors before
emission. A generated
Alpha adapter owns sealed byte I/O, validates the sole source-authored
Incomplete resource, and owns every private exhaustion, internal failure, and
selected external observation. Gamma source receives no general I/O primitive
and matching names do not select `DCOUT`.

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
26-code Delta rejection bijection without declaration-order authority. D30
fixes the physical `GCREQ`, profile limits, generated-runtime observations, and
`GCOUT`/`DCOUT` tables. D33 fixes bounded length admission before body exact-end
work, total schema-category priority, absence coordinates, and request-profile
code availability. The adapters remain incomplete and there is no published
tape. Its
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

D58 settles how the complete Gamma compiler revises the Beta compiler's private
resource profile. The current incomplete source's 965 calls, 739 states, and 586
edges are a baseline, not a final projection. A roomy noncanonical Beta compiler
stages the complete source; final procedures, calls, states, edges, derived
initialization storage, fixups, tape, and maximum work are then measured
conjunctively. Each independently provisioned authored-structure count receives
the least power-of-two provision with at most 75 percent occupancy; derived
guards remain bound to their owners and tape capacity remains D23-owned. Changed
tables move above the fixup table in the same atomic publication as the rebuilt
Beta tape and admission subject.

## Must not contain

No mutable host memory, hardware boundary, package manager, product optimizer,
or Delta parser hidden in Beta. Proof checking is not a Gamma language feature;
the universal checker remains Alpha-owned and outside the language rung.

## Implementation frontiers

- retain D23's coherent `AlphaBootstrapV2` profile and the consolidated adjacent
  conformance gate until D58 atomically publishes its measured private-table
  revision;
- implement D30's exact physical profiles, complete the two D19-selected
  adapters and remaining lowering in the exact Gamma compiler source, then
  publish its artifact closure;
- reuse the interpreter only as a specification or isolated algorithm source
  without turning runtime interpretation into a permanent dependency;
- emit exact Alpha tapes and checked source-to-tape certificates; and
- escalate on terrible compiler performance, Alpha verbosity, or proof
  explosion rather than adding special Gamma accelerators.
