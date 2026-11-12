Three sibling loans have different final-use boundaries. A grandchild closes
the second branch through its immediate parent; the whole view is observed
only after every branch closes. This exercises a child roster, not a pair-only
exception or a linear-chain recognizer.

# Runtime acceptance

`omega run --both tests/omega/pass/borrows/disjoint_branch_closure_with_grandchild/main.omg`
must exit 0 in both engines. Three sibling loans and a grandchild mutate the
same backing before the restored parent is observed. This challenges a pair-only
or linear-lineage implementation without changing the authored borrow pattern.
