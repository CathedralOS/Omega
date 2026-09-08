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
is described in the [evaluation reference](../../pre_migration/design_briefs/build_time_evaluation.md).
Effectful [build execution](../build/execution.md) is a separate operation.

Evaluation does not make an already-typed value anonymous. Anonymous numeric
initializers retain exact arithmetic, including rational division, until their
landing boundary. Integer landing requires an integral in-range value; floating
landing rounds to the selected format. The destination does not select integer
division inside an anonymous initializer. See [numeric values](numeric_values.md).

A target-semantic observation is a canonical constant input. It can appear in
ordinary constant positions, including array lengths and const-generic arguments.
A target-neutral product retains the symbolic dependency; target closure binds
the observation and selected realization. Folding must preserve those exact
dependencies in public signatures, artifact identity, and diagnostic provenance.
Adding or changing such a public-signature dependency is a breaking semantic API change.
Constants cannot add fields, cases, multiplicity, or declarations.

## Materialization

Evaluation and runtime representation are separate judgments. A value used only
by proofs, layouts, or further evaluation need not occupy runtime bytes.
Materialization requires a selected layout determining every observable bit of
the realized active value, recursively through its active fields and case.
Failure identifies the offending component and producer origin. Semantically
unobservable padding is emitted as zero.

Addressable immutable image storage would require a separate storage contract;
it is not implied by `const`. Likewise, there is no ambient mutable `static`.
Long-lived entry state is [one explicitly borrowed, provisioned occurrence](../build/entry_roots.md#entry-shape-and-arrival-bridge),
not a globally nameable object.
