# Borrowed payload state transfer

`omega run --both tests/omega/pass/borrows/case_payload_state_loan_exit/main.omg`
must exit 70 in both engines after the ordinary checkout-local package review.

Two selected payloads lend their original storage across a state edge, once
implicitly and once with an explicit `&`. The next state forwards the loan
again. Reads must observe 31 and 39 respectively; the empty case returns zero.
The source does not copy either payload into owned successor storage.

The original customer is `calls/case_scrutinee_self_dispatch_exit`; this
companion checks the values it computes, not only successful compilation.
