# machine_row_satisfaction

A signature-free nominal machine binder resolved through its authored
satisfaction row: `machine chosen(value) -> i32 satisfies Handler::call`
declares the satisfaction directly on the machine; `register<machine
Selected>` requires `requires machine Selected satisfies Handler::call` and
invokes `Selected(value)`; `register<chosen>(4)` selects it.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
