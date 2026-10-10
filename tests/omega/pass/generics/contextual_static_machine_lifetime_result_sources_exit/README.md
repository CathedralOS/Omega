# Applied callback lifetime contract

Run `omega run --both tests/omega/pass/generics/contextual_static_machine_lifetime_result_sources_exit/main.omg`.
Both engines must return 0. An explicitly selected callback preserves the first input's returned borrow independently of the second region, including a provider with no runtime specialization operands.
