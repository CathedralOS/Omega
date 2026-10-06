The earlier constructor field captures an issued owned ticket. A later field
replaces the caller's source with an unissued ticket. The observer must receive
the original 41/9 payload and membership, while the caller sees 0/0.

Run `omega run --both tests/omega/pending/calls/runtime_constructor_child_membership_snapshot_exit/main.omg`.
Both engines must exit 0.

Currently source checking rejects `observe`'s membership requirement. Flow
captures the completed constructor as one operand and removes facts about
sources changed during its evaluation. Supporting this program needs retained
completion facts for each constructor child, projected through the constructed
value without substituting a later child's source facts. The fail fixture
`tests/omega/fail/calls/constructor_child_membership_precedes_repair` pins the
opposite case: later issuance cannot validate an earlier unissued child.
