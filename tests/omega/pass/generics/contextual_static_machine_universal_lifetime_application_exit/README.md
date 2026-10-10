# Applied callback lifetime contract

Run `omega run --both tests/omega/pass/generics/contextual_static_machine_universal_lifetime_application_exit/main.omg`.
Both engines must return 0. The selected provider keeps a universal invocation lifetime; renamed fresh binders refine the callback requirement rather than selecting one captured region.
