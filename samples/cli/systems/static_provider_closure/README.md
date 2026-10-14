# Static provider closure

Run `omega run samples/cli/systems/static_provider_closure/main.omg`.
Expected exit: 0. The portable service calls a specifically selected raw map,
through explicit requirement selection, implementation qualification, a Unit
statement and a generic helper. Every result must come from that map even
though Build selects a different raw provider for runtime raw bindings.

The raw map covers both reads and mutations. The caller has only the portable
binding; it neither manufactures a raw binding nor searches for an implementation.
The compiler retains the exact requirement, selected map and realization beneath
the portable provider, independently of Build's runtime raw-provider selection.
