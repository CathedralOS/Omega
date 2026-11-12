# Empty stored extent has no loan demand

Run `omega run --both tests/omega/pass/concurrency/runtime_zero_extent_carry_exit/main.omg`.
Native and Terminal execution must exit 0 after observing the stored marker 42.
The explicit portable carry floor is valid: zero reference elements introduce
no retained loans, so their element type must not impose a backing obligation.
The nonempty stored field still participates in the carry derivation.
The carrier also satisfies an ordinary portable type-parameter bound; an
established structural bound is not an unresolved value-level qualification.
