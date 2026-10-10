# Applied callback with caller-scoped evidence

Run `omega run --both tests/omega/pass/generics/contextual_static_machine_conformance_application_exit/main.omg`.
Native and Terminal execution must both return 0. The source generic caller
supplies its exact promised evidence to an explicitly applied callback before
the closed caller selects `Positive`. Admission preserves the complete symbolic
tuple; executable specialization normalizes its closed provider to an entry.
