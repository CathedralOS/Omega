# Case construction and field replacement

Run `omega run --both tests/omega/pass/structs/case_field_type_store_shapes/main.omg`
from the repository root after preparing the fixture's package review. Both
executions must exit 70. The authored program checks common fields, scalar and
record payloads, erased members, and direct and local-mediated replacement.
Its helper call must return the declared payload-domain guarantees without
claiming that an old case tag survived mutation.
