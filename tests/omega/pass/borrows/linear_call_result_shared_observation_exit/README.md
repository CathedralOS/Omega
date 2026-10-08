# Shared observation leaves the result's linear claim owned

`omega run --both tests/omega/pass/borrows/linear_call_result_shared_observation_exit/main.omg`

The helper observes the issued receipt through a shared borrow, not an owned
transfer. The caller discharges the original receipt explicitly before exiting
70. Lending its storage must neither mint a replacement claim nor discharge it.
