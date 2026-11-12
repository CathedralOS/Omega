Run `omega run tests/omega/pass/arithmetic/runtime_widened_result_predicates_exit/main.omg --both`
with `input.txt` on stdin, after ordinary local package review. Both engines
exit 70.

The runtime byte `~` is ORed with 129 to produce 255 without replacing the
input with a constant. Exact wider-carrier multiplication produces 65535 and
4294967295. All 12 historically bounded widening results establish record
predicates and survive a helper return and state arrival. Other input bytes
may select exit 71; the input remains observable.
