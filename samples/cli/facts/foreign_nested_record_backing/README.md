# Retained original nested record backing

Parent: **NORMALIZED-ABI-LOWERING** in `TASKS.md`.

A mutable foreign registration retains the exact original nested Buffer backing until completion consumes Pending; completion writes its nested cell to 73 while marker 91 and an earlier copy 37 remain unchanged.

`Buffer` is an ordinary finite record containing marker `91` and a nested `Cell`. The C provider uses the corresponding 16-byte nested C struct; static assertions pin the outer size and field offsets. Registration writes nested value `41` and retains the original pointer. Completion writes `73` and verifies marker `91` before consuming the registration. The original copied value stays `37`.

The nine places preserve the same operation through local, field, parameter, call result, match join, const name, nested machine, other machine and selected target body. Every entry binds all four hosted targets. Each invalid twin inserts one nested scalar read before its linear Pending completes.

This row measures the missing recursive original-backing codec. No passing nine-place or four-host qualification is claimed until actual execution records establish it. The existing capacity and Pending content contracts and exact normal `Buffer::Owned` guarantee are preserved. Nested record shape adds no qualification or pointer-authority exception.

## Required behavior

- Establishment: the boundary takes a qualified exact mutable Buffer loan and returns linear Pending custody; `ensures buffer in Buffer::Owned` is the same explicit normal guarantee as the flat retained row. The nested `Cell` is an ordinary unqualified record.
- Identity and lifetime: both C writes reach the original nested scalar through the retained outer pointer, and completion consumes Pending before Omega reads the original again. Marker 91 and the copied scalar 37 are independent observations.
- Composition: the nine places transport Pending custody through local binding, an enclosing field, a parameter, call result, join, const-named construction, nested machine, another machine and selected target body.
- Soundness: each twin inserts one read of `buffer.cell.value` while retained exclusive custody remains live. Qualification and content projection are unchanged; nested shape grants no extra access.
- Psi 05–07: checking retains exact lifetime and access; lowering carries the original structural path through the boundary and Terminal representation. Terminal must execute the same nested storage semantics, including normal restoration coverage.
- Omega 00–02: the public foreign binding selects the same C ABI pointer lane, with the finite outer/nested layout joined to its original semantic scalar path. C asserts size 16, marker offset 0, cell offset 8 and nested scalar offset 0.
- Omega 03–06: materialized original storage and its nested scalar remain stable across registration, custody transport and completion; separate calls cannot replace that backing with independent snapshots.
- Omega 07–09: actual matching-host native execution and Terminal execution must both exit 0 for every positive. Source reject twins and the provider's real lifecycle traps retain separate oracles. Four authored target bindings alone do not establish four-host execution.

The authority is the existing [foreign storage contract](../../../../wiki/spec/build/foreign_storage.md), [original structural access](../../../../wiki/spec/terminal-psi/structural_access.md), and [loan restoration](../../../../wiki/spec/terminal-psi/loans.md). This row adds a finite nested record, rather than increasing a flat scalar count. Recursive pointer graphs, arrays, variants, dynamic views, independently qualified descendants and owned aggregate result publication remain separate acceptance.

## Measured baseline

Optimized macOS ARM64 producer `a96bc885ffebee4ebd1a35845700c05c6fe0c668` runs every positive natively with exit 0, while Terminal declines each with `native import needs a materialized non-scalar field codec`. Every reject twin fails for the intended read of nested original storage while its Pending remains live. The actual CLI returns 0 for the native-success/Terminal-decline pair; acceptance therefore requires the explicit Terminal exit-0 observation as well.

The real C fixture self-test exits 0 and all nine lifecycle controls reach their intended deliberate trap on macOS ARM64. These observations establish the baseline gap and control validity, not completed compiler acceptance or other-host execution.

## Canonical nested declarations

Every completion branch is a nested `machine`, and the main body is the free machine `run_main`. The bad branch publishes its own `crashes Trap` contract; nesting does not inherit runtime contracts. The `other_state` place declares a lexical `forward<'a>` helper inside non-generic `run_main`. Its lifetime and Pending argument are explicit, so it captures no enclosing static telescope or runtime state. This is the same retained-token transport and original-backing operation.

The historical baseline with legacy state declarations is preserved separately. Canonical focused checking and native execution establish the same missing Terminal codec. Enclosing generic/static telescope capture remains an existing unrelated implementation limitation rather than a feature this record-layout row claims to establish.
