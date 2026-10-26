# Recoverable replacement at a completed selector's exact element

Fact: two consuming transforms repair the same captured element while retaining
a linear sibling and disjoint scalar storage.

Parent: **BORROWED-STORAGE-RESTORATION** in [TASKS.md](../../../../TASKS.md).
The canonical laws are [source windows](../../../../wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
and [Terminal restoration](../../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).

`first_slot()` completes with the checked postcondition `result == 0`. Its call
still executes; the compiler must retain the completed value's exact coordinate
rather than replay the initializer or assume unrelated calls have equal results.
An exclusive alias extracts `holder.tokens[first_slot()]`, repairs through the
alias, extracts again, and repairs through the original indexed owner path.
Failure returns the removed token; success disposes it and constructs its
replacement. Each path owes the same captured element. There is no rollback.
The caller runs both outcomes and observes token 44, sibling token 7, and scalar
sibling 19 before disposing both linear tokens exactly once.

The nine transports are local, field, parameter, call_result, match_join,
const_name, other_state, other_machine and target_body. Each invalid twin
changes only the final repair store to element 1. Returning a value at the
wrong element cannot settle element 0's restoration debt.

## Requirements through the compiler

1. **Establishment:** ordinary constructors establish two distinct linear tokens;
   the completed Outcome producer returns the replacement's owned custody.
2. **Identity and lifetime:** the completed selector owns its scalar evidence.
   The retained alias names its original element across both extractions.
3. **Composition:** success/failure sums, alias/owner spellings and all nine
   transports retain the same original storage and caller-visible updates.
4. **Rejection:** wrong-element repair leaves element 0 absent. Unknown or changed
   selector values cannot acquire equality from their spelling or producer body.
5. **Psi 05–07:** actual selector evaluation and the pre-RHS destination survive
   lowering; the move's exact claim path agrees with that captured operand.
6. **Omega 00–02:** structural owned arguments and borrowed address joins preserve
   the original owner's claim frontier and exact selected geometry.
7. **Omega 03–06:** native selection transports the evaluated coordinate without
   copying caller storage or changing borrowed backing into an owned descriptor.
8. **Omega 07–09:** native and Terminal executions must both observe the authored
   zero-exit oracle. Checking another target alone is not host execution.

## Validation status

At frozen main `036605466510d2361fd579cf9fd97b722e070beb` and landed PR #164's
source `b1b9aaa57a30a2c3601c1fc8ae0e42212837d0fb`, the local indexed probe rejects
because the absent element must be restored as a whole. The literal-index
control already executes in both engines on both compilers. The repaired optimized-dev compiler now observes native 0 and Terminal 0 for
all nine positives on Linux x86-64; all nine twins reject the original
`holder.tokens[index]` restoration debt. The original match/alias matrix also
retains all eighteen positive/reject observations.

Additional corpus witnesses select element 1 and use an effectful selector
whose checked singleton result remains 1 while incrementing a disjoint counter.
The latter observes six actual selector invocations in both engines; the frozen
baseline source-checks it but refuses executable call ownership. A consumed
child cannot be borrowed through its whole parent, and missing result evidence,
changing non-singleton calls, wrong-element repair and shared mutation reject.

Exact final qualification/review is in progress. These Linux observations do not
claim other-host execution, physical suspension/cancellation, or effectful range
endpoint coverage. The parent TASKS.md item remains open.
