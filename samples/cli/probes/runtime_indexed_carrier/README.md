# Runtime-indexed fixed carrier

Ordinary `Limit: u32` binds an exact runtime subject while `value` retains its
fixed u32 representation. The authored input/payload table is in `oracle.json`.
Native and captured Terminal executions must return 70 after checking both
payloads and their carried indices independently. A static index of 9 is the
passing differential control.

The same index travels through a local, an ordinary parameter, and a returned
record. `validation/oracle.py` fixes the authored table independently of compiler
output; `validation/run.py --omega <compiler> --output <new-directory>` captures
native and Terminal streams, checks both exits, and retains the native product.
Prepare the project through the normal `omega update` review before execution.

`tests/omega/{pass,fail}/generics/runtime_index_carrier_*` covers static 9,
named constants, preserved snapshots, aggregate composition, const-only uses,
shadowing, stale or mismatched index subjects, descriptor initialization and
mutation, and scalar loans that would lose index preservation.
