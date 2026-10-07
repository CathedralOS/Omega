`cargo run -p omega -- run tests/omega/pass/arithmetic/runtime_boolean_guarded_bit_value_exit/main.omg`

The saved count guard proves both count validity and the range used to judge
the left-shift result. Counts 3 and 63 produce 8 and 2^63; count 64 takes the
fallback. Native exit: 0.
