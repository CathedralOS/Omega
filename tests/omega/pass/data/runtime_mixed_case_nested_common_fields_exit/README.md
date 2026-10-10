# Mixed-case common storage

Run `cargo run -p omega -- run --both tests/omega/pass/data/runtime_mixed_case_nested_common_fields_exit/main.omg`
from the repository root (`mbx` may replace `cargo`). Both engines must exit 0.

Common record and fixed-array members receive recursive zero initialization
when omitted. Explicit common and payload operands execute in authored order,
once; a skipped short-circuit branch executes none of them. Runtime indexing
reads the selected case's common scalar without confusing its tag or payload.
Erased common and payload declarations occupy no runtime initializer positions.

This fixture does not establish routed-domain authority. Construction still has
to prove each child's declared domains, and transporting licensed memberships
through common case fields remains a separate source-checking frontier.
