# Address arithmetic

The program adds a `u64` count to an address in both operand orders, with typed
and anonymous counts. It also checks backward offsets, address distance and
an ordinary mutable caller. All four hosted entries are declared; the authored
result is exit 0 in native code and Terminal Psi.

[The overflow control](../../../../../../../_recycling/tests/fixtures/fail/arithmetic/addr_add_overflow/main.omg)
tries to produce the exclusive address bound and must reject. Address-plus-address,
wrong count carriers and policy-qualified arithmetic remain covered by the
source address-bound controls.
