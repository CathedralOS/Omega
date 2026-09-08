# Constants

`const` names an immutable compile-time value, not runtime storage. It may be
package/module-scoped or genuinely type-scoped, outside a `data` body:

```omega
pub const PAGE_SIZE: u64 = 4096;
const EfiStatus::SUCCESS = EfiStatus { code: 0 };
```

Constants are not fields and do not contribute to `sizeof`. Their complete
types recursively permit copying and have no cleanup obligation, shared
ownership, or interior mutability. Fixed arrays, records, and copy-eligible sums
are eligible; selecting an unrestricted active case does not make a structurally
linear sum eligible. A constant has no stable address and grants no authority.

## Evaluation

A constant position requests semantic evaluation. An initializer may call an
ordinary machine only when that concrete invocation is admitted for evaluation;
there is no separate `const machine` species. The evaluator's admission contract
is described in [semantic evaluation](evaluation.md#invocation-admission).
Effectful [build execution](../build/execution.md) is a separate operation.

Evaluation does not make an already-typed value anonymous. Anonymous numeric
initializers retain exact arithmetic, including rational division, until their
landing boundary. Integer landing requires an integral in-range value; floating
landing rounds to the selected format. The destination does not select integer
division inside an anonymous initializer. See [numeric values](numeric_values.md).

[Target-semantic observations](evaluation.md#target-dependent-applications)
are canonical constant inputs, including in array lengths and const-generic
arguments. Their symbolic dependency, exact target closure, and provenance
survive folding. Constants cannot add fields, cases, multiplicity, or declarations.

## Materialization

Evaluation and runtime representation are separate judgments. A value used only
by proofs, layouts, or further evaluation need not occupy runtime bytes.
Materialization requires a selected layout determining every observable bit of
the realized active value, recursively through its active fields and case.
Failure identifies the offending component and producer origin. Semantically
unobservable padding is emitted as zero.

A NaN whose payload is not determined remains usable through `Float::meaning`
in proof and compile-time positions. Runtime materialization requires
canonicalization, explicit bits, or a selected realization publishing the exact
representation. An arbitrary evaluator choice cannot become stable image data.

A quotient retains the exact operational representative selected at construction,
as runtime execution does. Ordinary opaque materialization may emit that carried
representative without canonicalization; equivalent quotient values may have
different bytes. Representative-independent consumers instead require proved
canonical form: stable serialization, public ABI representation, canonical
const-index identity, structural hashing/interning, or reproducible raw bytes.
The distinction does not expose the opaque representative to source code.

Addressable immutable image storage would require a separate storage contract;
it is not implied by `const`. Likewise, there is no ambient mutable `static`.
Long-lived entry state is [one explicitly borrowed, provisioned occurrence](../build/entry_roots.md#entry-shape-and-arrival-bridge),
not a globally nameable object.
