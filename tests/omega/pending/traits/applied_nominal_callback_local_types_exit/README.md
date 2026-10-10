The applied visitor has both a type binder and a callback binder. Each selected
callback must retain its Item/Other carrier and its invocation-local type. Both
calls return true; the intended native and Terminal exit is 0.

Run: `omega run --both main.omg`.

Current first failure on macOS ARM64: Stage 03b cannot derive the complete
specialization tuple for `item_visit` and `other_visit` through the selected
nominal schema. Applied declaration/callback checking precedes this failure.
Owned by PSI-TARGET-FAMILIES; repair positional application binding rather than
changing the program or accepting an incomplete tuple.
