A nonconstant checked index reads a constrained scalar field through a shared call. The selected value, neighboring element and out-of-range branch retain their authored values; native and Terminal execution must exit 0.

Run `omega run --both tests/omega/pass/layouts/runtime_indexed_restricted_field_read/main.omg` after ordinary package preparation.

Runtime-selector joining uses the argument’s retained access and the same access-aware projection as placement and instruction selection. Shared observation exposes the constrained scalar carrier; it does not grant ownership or writable access. The checked selector still requires its exact machine, operation and accepted bounds obligation.
