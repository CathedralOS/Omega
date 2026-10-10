`omega run --both tests/omega/pass/dependent/runtime_applied_default_domain_setter_exit/main.omg`
must return `0` after the generic setter stores `3`. The declaration's `Limit` and the
method's independently named `Bound` are different binders. Its admitted argument
requirement proves the post-store predicate at the prepared `Counter<4>` application;
neither zero qualification nor an unchecked entry requirement proves the write.
