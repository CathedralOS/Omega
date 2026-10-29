# Owned array field replacement

Replacing the owned array field with [7, 4] establishes its current [7, 4] contents while the earlier owned copy remains [3, 1].

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, whole-value record-field and owned-array predicate handback with current storage versions. This is distinct from the saved-observation indexed-store production: the current element must change from 3 to 7 and acquire a new postcondition, while the copied earlier array retains both 3 and 1.

The field replacement takes an owned incoming array constrained by the ordinary checked `installed` predicate. `consume` requires the complete current predicate and both old copied elements. No caller guard supplies these facts. Runtime checks follow consumption and distinguish omitted stores, partial/wrong-offset copies, old/new aliasing, wrong selected match arms, and corrupted record neighbors. Every reject twin changes only the installed array's first element to 6 and must reject install's own Ensures.

Fixed discovery base: `f6f6bbb07c0ae50e280fe64a209a54e60745e03e`. The local witness is frozen before implementation. Baseline observation, the other transport places and pinned corpus controls are pending. The row is not complete.

Canonical requirements: [collection invariants](../../../../wiki/spec/language/dependent_values.md#collection-invariants), [structural replacement](../../../../wiki/spec/terminal-psi/structural_access.md), [normal returns](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use), and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md). Earlier values remain immutable; new storage contents owe their own predicate. Carrier custody, exact layout/type, writable access, formation and every normal exit remain independent obligations. Descriptor/reference rebinding is a separate operation.
