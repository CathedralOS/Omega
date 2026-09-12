# Build evaluation

`src/lib.rs` admits and executes one selected companion build machine. Its
configuration, declarations, target/root selection, optimization, observation,
and replay modules own their respective results and checks.

## Root-binding implementation boundary

`builder.roots.bind(Target::Slot, Product::entry);` is retained as a dedicated
Psi statement with separate product-context operand paths. It is not an ordinary
machine invocation. The host resolver does not resolve those operands through
host imports; selected-target/product admission still owns that resolution.

The current implementation statically harvests bindings from the authoritative
companion build machine and supports direct uses of its compiler-issued
`&mut Build` parameter. Computed receivers, aliases, and helper declarations
reject explicitly instead of being silently ignored. Parameter spelling has no
authority; checking uses its resolved identity and the toolchain Build owner.

These are implementation limits, not language restrictions.
[Evaluated build work](../../../../wiki/spec/build/declarations.md#evaluated-build-work)
permits helpers to borrow the root Build value for binding. General evaluated
binding and restricted product-reference handoff remain under
`BUILD-PRODUCT-REFERENCES` in `TASKS.md`; this static projection does not implement
them or grant product authority from a path spelling.
